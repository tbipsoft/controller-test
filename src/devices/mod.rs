//! Especificacoes de cada aparelho. Para adicionar um modelo: crie um arquivo com um
//! `PROFILE`, declare o modulo aqui e inclua-o em `ALL`. O resto do app so le `Profile`.
//!
//! A resolucao nao fica aqui de proposito: a tela usa o tamanho real informado pelo SDL.

mod a10mini;
mod detect;
mod generic;
mod r36s;
mod rg43;
mod rg52mini;
mod rgb10;
mod rk3566;

pub use detect::{identify, resolve, Detected, Source};
pub use generic::PROFILE as GENERIC;

use crate::pad::{Btn, Controls, NB};

#[derive(Debug, PartialEq)]
pub struct Profile {
    /// Identificador aceito em `--device`.
    pub id: &'static str,
    /// Nome curto do cabecalho.
    pub title: &'static str,
    /// Nome completo mostrado em `--list`.
    pub name: &'static str,
    /// Valores de `/home/ark/.config/.DEVICE` (ou hostname) que indicam este perfil.
    pub markers: &'static [&'static str],
    pub buttons: &'static [Btn],
    /// Analogicos presentes: [esquerdo, direito].
    pub sticks: [bool; 2],
    pub triggers: bool,
    /// Combinacao de saida padrao; precisa usar botoes que o aparelho tem.
    pub exit: &'static str,
    /// `true`: analogico acima do direcional. `false`: direcional acima do analogico.
    pub stick_on_top: bool,
    /// Botao desenhado como pilula com outro nome (ex.: b16 do RGB10 = "F").
    pub alias: Option<(Btn, &'static str)>,
    /// Botoes crus sem nome no mapeamento do SDL que este aparelho usa.
    pub raw_map: &'static [(u8, Btn)],
}

impl Profile {
    pub fn controls(&self) -> Controls {
        let mut buttons = [false; NB];
        for b in self.buttons {
            buttons[b.idx()] = true;
        }
        Controls {
            buttons,
            sticks: self.sticks,
            triggers: self.triggers,
            alias: self.alias,
        }
    }
}

pub const ALL: [&Profile; 7] = [
    &rg52mini::PROFILE,
    &rg43::PROFILE,
    &r36s::PROFILE,
    &rgb10::PROFILE,
    &a10mini::PROFILE,
    &rk3566::PROFILE,
    &generic::PROFILE,
];

/// Maiusculas, sem espacos, `-`, `_` nem NUL: `rg-351 mp\n` e `RG351MP` sao iguais.
pub(crate) fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Procura um perfil pelo `id` ou por um dos marcadores.
pub fn find(name: &str) -> Option<&'static Profile> {
    let n = normalize(name);
    if n.is_empty() {
        return None;
    }
    ALL.into_iter()
        .find(|p| normalize(p.id) == n || p.markers.iter().any(|m| normalize(m) == n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::{parse_exit, ExitKey};

    #[test]
    fn saida_de_cada_perfil_usa_botoes_que_existem() {
        for p in ALL {
            for key in parse_exit(p.exit).unwrap() {
                match key {
                    ExitKey::Btn(b) => assert!(p.controls().has(b), "{}: {b:?}", p.id),
                    ExitKey::Raw(n) => panic!("{}: saida nao deve depender de b{n}", p.id),
                }
            }
        }
    }

    #[test]
    fn perfis_nao_repetem_marcadores_nem_botoes() {
        let mut seen = std::collections::HashSet::new();
        for p in ALL {
            assert!(seen.insert(normalize(p.id)), "id repetido: {}", p.id);
            for m in p.markers {
                assert!(seen.insert(normalize(m)) || normalize(m) == normalize(p.id));
            }
            let mut b = p.buttons.to_vec();
            b.sort();
            b.dedup();
            assert_eq!(b.len(), p.buttons.len(), "{}", p.id);
            if let Some((btn, _)) = p.alias {
                assert!(p.buttons.contains(&btn), "{}", p.id);
            }
            for (_, btn) in p.raw_map {
                assert!(p.buttons.contains(btn), "{}", p.id);
            }
        }
    }

    #[test]
    fn total_de_itens_por_aparelho() {
        let total = |id| find(id).unwrap().controls().total();
        assert_eq!(total("r36s"), 24);
        assert_eq!(total("rgb10"), 20);
        assert_eq!(total("a10mini"), 25);
        assert_eq!(total("rg52mini"), 26);
        assert_eq!(total("rg43"), 25);
        assert_eq!(total("rk3566"), 25);
        assert_eq!(total("generic"), 26);
    }
}
