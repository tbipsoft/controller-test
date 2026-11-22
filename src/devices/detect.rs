//! Descobre em qual aparelho o app esta rodando.
//!
//! Ordem: `--device`, `/home/ark/.config/.DEVICE` (gravado pelo build do dArkOS), hostname
//! e familia do chip no device tree. Sem nenhuma pista, usa o perfil generico.

use std::fs;

use super::{find, normalize, Profile, GENERIC};

const MARKER_FILE: &str = "/home/ark/.config/.DEVICE";
const HOSTNAME_FILE: &str = "/etc/hostname";
const COMPATIBLE_FILE: &str = "/proc/device-tree/compatible";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Manual,
    MarkerFile,
    Hostname,
    DeviceTree,
    Fallback,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Manual => "--device",
            Source::MarkerFile => MARKER_FILE,
            Source::Hostname => HOSTNAME_FILE,
            Source::DeviceTree => COMPATIBLE_FILE,
            Source::Fallback => "nenhuma pista (perfil generico)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detected {
    pub profile: &'static Profile,
    pub source: Source,
}

/// Decisao pura, testavel sem o aparelho.
pub fn identify(
    marker: Option<&str>,
    hostname: Option<&str>,
    compatible: Option<&str>,
) -> Detected {
    let hit = |profile, source| Detected { profile, source };
    if let Some(p) = marker.and_then(find) {
        return hit(p, Source::MarkerFile);
    }
    if let Some(p) = hostname.and_then(find) {
        return hit(p, Source::Hostname);
    }
    // rk3326 nao decide: R36S, RGB10, A10 Mini e G350 usam o mesmo chip.
    let chip = compatible.map(normalize).unwrap_or_default();
    for (name, id) in [("RK3562", "rg52mini"), ("RK3566", "rk3566")] {
        if chip.contains(name) {
            if let Some(p) = find(id) {
                return hit(p, Source::DeviceTree);
            }
        }
    }
    hit(&GENERIC, Source::Fallback)
}

fn detect() -> Detected {
    let read = |path| fs::read_to_string(path).ok();
    // o device tree tem NULs e pode nao ser UTF-8 valido
    let compatible = fs::read(COMPATIBLE_FILE)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).replace('\0', " "));
    identify(
        read(MARKER_FILE).as_deref(),
        read(HOSTNAME_FILE).as_deref(),
        compatible.as_deref(),
    )
}

/// `None` ou `auto` detectam; qualquer outro valor escolhe o perfil manualmente.
pub fn resolve(requested: Option<&str>) -> Result<Detected, String> {
    match requested {
        None => Ok(detect()),
        Some(r) if r.eq_ignore_ascii_case("auto") => Ok(detect()),
        Some(r) => find(r)
            .map(|profile| Detected {
                profile,
                source: Source::Manual,
            })
            .ok_or_else(|| {
                let ids: Vec<_> = super::ALL.iter().map(|p| p.id).collect();
                format!("--device invalido: {r:?} (use auto, {})", ids.join(", "))
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(m: Option<&str>, h: Option<&str>, c: Option<&str>) -> (&'static str, Source) {
        let d = identify(m, h, c);
        (d.profile.id, d.source)
    }

    #[test]
    fn marcadores_gravados_pelo_darkos() {
        for (marker, want) in [
            ("RG52MINI\n", "rg52mini"),
            ("RG43H\n", "rg43"),
            ("RG351MP\n", "r36s"),
            ("RGB10\n", "rgb10"),
            ("A10MINI\n", "a10mini"),
            ("A10MINIV4\n", "a10mini"),
            ("RG353M\n", "rk3566"),
            ("RG503\n", "rk3566"),
            ("RK2023\n", "rk3566"),
            ("MINILOONG\n", "rk3566"),
        ] {
            assert_eq!(
                id(Some(marker), None, None),
                (want, Source::MarkerFile),
                "{marker:?}"
            );
        }
    }

    #[test]
    fn ordem_de_prioridade() {
        // o marcador vence o hostname e o chip
        assert_eq!(
            id(Some("RGB10"), Some("a10mini"), Some("rockchip,rk3566")).0,
            "rgb10"
        );
        // marcador desconhecido cai para o hostname
        assert_eq!(
            id(Some("XYZ"), Some("rg351mp\n"), None),
            ("r36s", Source::Hostname)
        );
        assert_eq!(
            id(
                None,
                Some("debian"),
                Some("rockchip,rk3566-evb\0rockchip,rk3566\0")
            ),
            ("rk3566", Source::DeviceTree)
        );
    }

    #[test]
    fn chip_rk3326_sozinho_nao_adivinha_o_modelo() {
        assert_eq!(
            id(None, Some("debian"), Some("rockchip,rk3326\0")),
            ("generic", Source::Fallback)
        );
        assert_eq!(id(None, None, None), ("generic", Source::Fallback));
        assert_eq!(id(Some(""), Some(""), Some("")).0, "generic");
    }

    #[test]
    fn escolha_manual() {
        let d = resolve(Some("R36S")).unwrap();
        assert_eq!((d.profile.id, d.source), ("r36s", Source::Manual));
        assert_eq!(resolve(Some("rg-351mp")).unwrap().profile.id, "r36s");
        assert_eq!(resolve(Some("rg52 mini")).unwrap().profile.id, "rg52mini");
        assert!(resolve(Some("gameboy")).unwrap_err().contains("r36s"));
    }
}
