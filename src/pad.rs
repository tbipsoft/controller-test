//! Estado do teste, sem dependencia de SDL: o `main` traduz os eventos do SDL em
//! `Input` e a tela le o `State`. Assim toda a logica (quais botoes ja foram testados,
//! alcance dos analogicos, combinacao de saida, log) e testada em x86 sem controle.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Botoes digitais com nome (os do mapeamento padrao do SDL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Btn {
    A,
    B,
    X,
    Y,
    Up,
    Down,
    Left,
    Right,
    L1,
    R1,
    L3,
    R3,
    Start,
    Select,
    Home,
    Back,
}

/// Quantidade de botoes digitais com nome.
pub const NB: usize = 16;

impl Btn {
    pub const ALL: [Btn; NB] = [
        Btn::A,
        Btn::B,
        Btn::X,
        Btn::Y,
        Btn::Up,
        Btn::Down,
        Btn::Left,
        Btn::Right,
        Btn::L1,
        Btn::R1,
        Btn::L3,
        Btn::R3,
        Btn::Start,
        Btn::Select,
        Btn::Home,
        Btn::Back,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Btn::A => "A",
            Btn::B => "B",
            Btn::X => "X",
            Btn::Y => "Y",
            Btn::Up => "UP",
            Btn::Down => "DOWN",
            Btn::Left => "LEFT",
            Btn::Right => "RIGHT",
            Btn::L1 => "L1",
            Btn::R1 => "R1",
            Btn::L3 => "L3",
            Btn::R3 => "R3",
            Btn::Start => "START",
            Btn::Select => "SELECT",
            Btn::Home => "HOME",
            Btn::Back => "BACK",
        }
    }

    pub fn idx(self) -> usize {
        Btn::ALL.iter().position(|b| *b == self).unwrap_or(0)
    }
}

/// Quais controles existem no aparelho: so eles aparecem na tela e contam no progresso.
/// Um evento de controle ausente continua indo para o log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controls {
    pub buttons: [bool; NB],
    pub sticks: [bool; 2],
    pub triggers: bool,
    /// Botao mostrado com outro nome (ex.: `R3` do RGB10 = "F").
    pub alias: Option<(Btn, &'static str)>,
}

impl Controls {
    pub const ALL: Controls = Controls {
        buttons: [true; NB],
        sticks: [true; 2],
        triggers: true,
        alias: None,
    };

    pub fn has(&self, b: Btn) -> bool {
        self.buttons[b.idx()]
    }

    pub fn label(&self, b: Btn) -> &'static str {
        match self.alias {
            Some((btn, name)) if btn == b => name,
            _ => b.label(),
        }
    }

    pub fn total(&self) -> usize {
        self.buttons.iter().filter(|b| **b).count()
            + self.sticks.iter().filter(|s| **s).count() * DIRS
            + if self.triggers { 2 } else { 0 }
    }
}

/// Eixos: dois analogicos (X/Y) e os dois gatilhos analogicos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    LX,
    LY,
    RX,
    RY,
    L2,
    R2,
}

impl Axis {
    pub fn label(self) -> &'static str {
        match self {
            Axis::LX => "LX",
            Axis::LY => "LY",
            Axis::RX => "RX",
            Axis::RY => "RY",
            Axis::L2 => "L2",
            Axis::R2 => "R2",
        }
    }
}

/// Uma tecla da combinacao de saida: botao nomeado ou botao cru `bN` (para o caso de o
/// HOME nao estar no mapeamento do SDL).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKey {
    Btn(Btn),
    Raw(u8),
}

impl ExitKey {
    pub fn label(&self) -> String {
        match self {
            ExitKey::Btn(b) => b.label().to_string(),
            ExitKey::Raw(n) => format!("B{n}"),
        }
    }
}

/// `start+home`, `start+select`, `start+b12`... (sem diferenciar maiusculas).
pub fn parse_exit(s: &str) -> Result<Vec<ExitKey>, String> {
    let keys: Result<Vec<ExitKey>, String> = s
        .split('+')
        .map(|t| {
            let t = t.trim().to_ascii_lowercase();
            if let Some(n) = t.strip_prefix('b').and_then(|n| n.parse::<u8>().ok()) {
                return Ok(ExitKey::Raw(n));
            }
            Btn::ALL
                .iter()
                .find(|b| {
                    b.label().to_ascii_lowercase() == t || (t == "back" && **b == Btn::Select)
                })
                .map(|b| ExitKey::Btn(*b))
                .ok_or_else(|| format!("botao desconhecido na combinacao: {t:?}"))
        })
        .collect();
    let keys = keys?;
    if keys.len() < 2 {
        return Err(
            "a combinacao de saida precisa de pelo menos 2 botoes (ex.: start+home)".into(),
        );
    }
    Ok(keys)
}

/// Mapeamento SDL de um controle (`SDL_GameControllerMapping`): nome -> origem fisica.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Binds {
    /// `a` -> `b1`, `leftx` -> `a0`, `dpup` -> `h0.1`...
    pub by_name: HashMap<String, String>,
    pub buttons: HashSet<u8>,
    pub axes: HashSet<u8>,
    pub hats: HashSet<u8>,
}

/// Le uma linha `guid,nome,a:b0,b:b1,dpup:h0.1,lefttrigger:a4,platform:Linux,`.
pub fn parse_mapping(m: &str) -> Binds {
    let mut out = Binds::default();
    for entry in m.split(',').skip(2) {
        let Some((name, val)) = entry.split_once(':') else {
            continue;
        };
        if name == "platform" || name == "crc" || name == "hint" || val.is_empty() {
            continue;
        }
        let v = val.trim_start_matches(['+', '-']).trim_end_matches('~');
        let num = |s: &str| s.split('.').next().and_then(|n| n.parse::<u8>().ok());
        let ok = match v.chars().next() {
            Some('b') => v[1..].parse::<u8>().map(|n| out.buttons.insert(n)).is_ok(),
            Some('a') => v[1..].parse::<u8>().map(|n| out.axes.insert(n)).is_ok(),
            Some('h') => num(&v[1..]).map(|n| out.hats.insert(n)).is_some(),
            _ => false,
        };
        if ok {
            out.by_name.insert(name.to_string(), v.to_string());
        }
    }
    out
}

/// `back=b13,home=b12`: atribui botoes crus (sem nome no mapeamento do SDL) a controles
/// do desenho. Devolve pares (indice cru, controle).
pub fn parse_map(s: &str) -> Result<Vec<(u8, Btn)>, String> {
    let mut out = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (name, raw) = part
            .split_once('=')
            .ok_or_else(|| format!("--map: use nome=bN (ex.: back=b13), recebi {part:?}"))?;
        let name = name.trim().to_ascii_lowercase();
        let btn = Btn::ALL
            .iter()
            .find(|b| b.label().to_ascii_lowercase() == name)
            .ok_or_else(|| format!("--map: controle desconhecido: {name:?}"))?;
        let n = raw
            .trim()
            .to_ascii_lowercase()
            .strip_prefix('b')
            .and_then(|n| n.parse::<u8>().ok())
            .ok_or_else(|| format!("--map: botao cru invalido: {raw:?} (use bN)"))?;
        out.push((n, *btn));
    }
    Ok(out)
}

/// Teclas que o sistema entrega como teclado e equivalem a um botao do desenho
/// (o BACK/HOME do aparelho podem chegar assim, como "AC Back" / "AC Home").
pub fn key_as_button(name: &str) -> Option<Btn> {
    match name {
        "AC Back" => Some(Btn::Back),
        "AC Home" => Some(Btn::Home),
        _ => None,
    }
}

/// O que o `main` entrega ao estado.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Button {
        btn: Btn,
        down: bool,
        /// de onde veio no hardware (`b3`, `a2`, `h0.1`), se o SDL souber dizer
        raw: Option<String>,
    },
    /// `value` na faixa do SDL: analogicos -32768..=32767, gatilhos 0..=32767
    Axis {
        axis: Axis,
        value: i16,
        raw: Option<String>,
    },
    /// Controle fisico sem nome no mapeamento (botao `idx` ou eixo/hat).
    Raw {
        what: String,
        idx: Option<u8>,
        down: Option<bool>,
        value: Option<i16>,
    },
    Key {
        name: String,
        down: bool,
    },
    Device {
        name: String,
        info: String,
        added: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Down,
    Up,
    Axis,
    Raw,
    Key,
    Sys,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub id: u64,
    /// sobe a cada atualizacao no lugar (eixos), para a tela refazer o texto
    pub ver: u32,
    pub t: f64,
    pub kind: Kind,
    pub name: String,
    pub act: String,
    pub detail: String,
}

pub const SECTORS: usize = 24;
pub const LOG_MAX: usize = 60;
/// Segundos segurando a combinacao de saida.
pub const EXIT_HOLD: f64 = 1.0;
/// Acima disto (90 %) o gatilho/direcao do analogico conta como testado.
pub const FULL: f32 = 0.9;
/// Ruido do potenciometro: variacoes menores que isto nao entram no log.
const AXIS_LOG_STEP: f32 = 0.03;

/// 0 = cima, 1 = baixo, 2 = esquerda, 3 = direita.
pub const DIRS: usize = 4;

#[derive(Clone, Debug)]
pub struct State {
    pub pressed: [bool; NB],
    pub tested: [bool; NB],
    /// (x, y) de cada analogico em -1..=1; y negativo = para cima
    pub stick: [(f32, f32); 2],
    /// maior raio atingido por setor (0..~1,1): mostra se o curso e redondo
    pub reach: [[f32; SECTORS]; 2],
    pub dirs: [[bool; DIRS]; 2],
    pub trig: [f32; 2],
    pub trig_tested: [bool; 2],
    pub vol_down: bool,
    pub vol_up: bool,
    pub vol_down_seen: bool,
    pub vol_up_seen: bool,
    pub log: VecDeque<Line>,
    pub devices: Vec<(String, String)>,
    pub events: u64,
    pub exit_combo: Vec<ExitKey>,
    pub controls: Controls,
    raw_down: BTreeSet<u8>,
    esc_down: bool,
    hold_since: Option<f64>,
    next_id: u64,
    last_logged: [f32; 6],
    raw_last: HashMap<String, i16>,
    pub dirty: bool,
}

impl State {
    pub fn new(exit_combo: Vec<ExitKey>) -> State {
        State::with_controls(exit_combo, Controls::ALL)
    }

    pub fn with_controls(exit_combo: Vec<ExitKey>, controls: Controls) -> State {
        State {
            controls,
            pressed: [false; NB],
            tested: [false; NB],
            stick: [(0.0, 0.0); 2],
            reach: [[0.0; SECTORS]; 2],
            dirs: [[false; DIRS]; 2],
            trig: [0.0; 2],
            trig_tested: [false; 2],
            vol_down: false,
            vol_up: false,
            vol_down_seen: false,
            vol_up_seen: false,
            log: VecDeque::new(),
            devices: Vec::new(),
            events: 0,
            exit_combo,
            raw_down: BTreeSet::new(),
            esc_down: false,
            hold_since: None,
            next_id: 0,
            last_logged: [0.0; 6],
            raw_last: HashMap::new(),
            dirty: true,
        }
    }

    /// Maximo de itens: 16 botoes + 4 direcoes x 2 analogicos + 2 gatilhos.
    pub const TOTAL: usize = NB + 2 * DIRS + 2;

    /// Itens a testar neste aparelho.
    pub fn total(&self) -> usize {
        self.controls.total()
    }

    pub fn tested_count(&self) -> usize {
        let c = &self.controls;
        let buttons = (0..NB).filter(|&i| c.buttons[i] && self.tested[i]).count();
        let dirs = (0..2)
            .filter(|&s| c.sticks[s])
            .map(|s| self.dirs[s].iter().filter(|t| **t).count())
            .sum::<usize>();
        let trig = if c.triggers {
            self.trig_tested.iter().filter(|t| **t).count()
        } else {
            0
        };
        buttons + dirs + trig
    }

    pub fn is_pressed(&self, b: Btn) -> bool {
        self.pressed[b.idx()]
    }

    pub fn is_tested(&self, b: Btn) -> bool {
        self.tested[b.idx()]
    }

    fn push(&mut self, t: f64, kind: Kind, name: &str, act: &str, detail: &str) {
        self.next_id += 1;
        self.log.push_back(Line {
            id: self.next_id,
            ver: 0,
            t,
            kind,
            name: name.to_string(),
            act: act.to_string(),
            detail: detail.to_string(),
        });
        while self.log.len() > LOG_MAX {
            self.log.pop_front();
        }
    }

    /// Eixos: se a ultima linha e do mesmo eixo e recente, atualiza no lugar (em vez de
    /// inundar o log com dezenas de linhas por segundo).
    fn push_axis(&mut self, t: f64, kind: Kind, name: &str, act: &str, detail: &str) {
        if let Some(l) = self.log.back_mut() {
            if l.kind == kind && l.name == name && t - l.t < 0.6 {
                l.t = t;
                l.ver += 1;
                l.act = act.to_string();
                l.detail = detail.to_string();
                return;
            }
        }
        self.push(t, kind, name, act, detail);
    }

    fn combo_active(&self) -> bool {
        if self.esc_down {
            return true;
        }
        !self.exit_combo.is_empty()
            && self.exit_combo.iter().all(|k| match k {
                ExitKey::Btn(b) => self.pressed[b.idx()],
                ExitKey::Raw(n) => self.raw_down.contains(n),
            })
    }

    fn refresh_hold(&mut self, now: f64) {
        match (self.combo_active(), self.hold_since) {
            (true, None) => self.hold_since = Some(now),
            (false, Some(_)) => self.hold_since = None,
            _ => {}
        }
    }

    /// 0..=1: quanto da retencao de saida ja passou.
    pub fn exit_progress(&self, now: f64) -> f32 {
        self.hold_since
            .map_or(0.0, |t| ((now - t) / EXIT_HOLD).clamp(0.0, 1.0) as f32)
    }

    pub fn holding_exit(&self) -> bool {
        self.hold_since.is_some()
    }

    pub fn exit_ready(&self, now: f64) -> bool {
        self.hold_since.is_some_and(|t| now - t >= EXIT_HOLD)
    }

    pub fn apply(&mut self, now: f64, input: Input) {
        self.events += 1;
        self.dirty = true;
        match input {
            Input::Button { btn, down, raw } => {
                self.pressed[btn.idx()] = down;
                if down {
                    self.tested[btn.idx()] = true;
                }
                let act = if down { "DOWN" } else { "UP" };
                let kind = if down { Kind::Down } else { Kind::Up };
                let label = self.controls.label(btn);
                self.push(now, kind, label, act, raw.as_deref().unwrap_or(""));
            }
            Input::Axis { axis, value, raw } => {
                let detail = raw.unwrap_or_default();
                match axis {
                    Axis::L2 | Axis::R2 => {
                        let i = (axis == Axis::R2) as usize;
                        let v = (value.max(0) as f32 / 32767.0).clamp(0.0, 1.0);
                        self.trig[i] = v;
                        if v >= FULL {
                            self.trig_tested[i] = true;
                        }
                        self.log_axis(now, axis, v, &format!("{:>3.0}%", v * 100.0), &detail);
                    }
                    _ => {
                        let v = (value as f32 / 32768.0).clamp(-1.0, 1.0);
                        let s = matches!(axis, Axis::RX | Axis::RY) as usize;
                        let vertical = matches!(axis, Axis::LY | Axis::RY);
                        if vertical {
                            self.stick[s].1 = v;
                        } else {
                            self.stick[s].0 = v;
                        }
                        // direcoes: cima/baixo = y (SDL: cima e negativo), esquerda/direita = x
                        let (lo, hi) = if vertical { (0, 1) } else { (2, 3) };
                        if v <= -FULL {
                            self.dirs[s][lo] = true;
                        }
                        if v >= FULL {
                            self.dirs[s][hi] = true;
                        }
                        let (x, y) = self.stick[s];
                        let r = (x * x + y * y).sqrt().min(1.2);
                        let a = y.atan2(x) + std::f32::consts::PI;
                        let sec = ((a / (2.0 * std::f32::consts::PI)) * SECTORS as f32) as usize
                            % SECTORS;
                        if r > self.reach[s][sec] {
                            self.reach[s][sec] = r;
                        }
                        self.log_axis(now, axis, v, &format!("{v:+.2}"), &detail);
                    }
                }
            }
            Input::Raw {
                what,
                idx,
                down,
                value,
            } => {
                if let (Some(i), Some(d)) = (idx, down) {
                    if d {
                        self.raw_down.insert(i);
                    } else {
                        self.raw_down.remove(&i);
                    }
                }
                let act = match (down, value) {
                    (Some(true), _) => "DOWN".to_string(),
                    (Some(false), _) => "UP".to_string(),
                    (None, Some(v)) => format!("{:+.2}", v as f32 / 32768.0),
                    _ => String::new(),
                };
                match (down, value) {
                    // eixo cru: coalesce e ignora ruido, como nos eixos nomeados
                    (None, Some(v)) => {
                        let last = self.raw_last.get(&what).copied().unwrap_or(0);
                        if (v as i32 - last as i32).abs() >= 1000 || (v == 0 && last != 0) {
                            self.raw_last.insert(what.clone(), v);
                            self.push_axis(now, Kind::Raw, &what, &act, "not mapped");
                        }
                    }
                    _ => self.push(now, Kind::Raw, &what, &act, "not mapped"),
                }
            }
            Input::Key { name, down } => {
                match name.as_str() {
                    "VolumeDown" => {
                        self.vol_down = down;
                        self.vol_down_seen |= down;
                    }
                    "VolumeUp" => {
                        self.vol_up = down;
                        self.vol_up_seen |= down;
                    }
                    "Escape" => self.esc_down = down,
                    _ => {}
                }
                self.push(
                    now,
                    Kind::Key,
                    &name,
                    if down { "DOWN" } else { "UP" },
                    "keyboard",
                );
            }
            Input::Device { name, info, added } => {
                if added {
                    self.devices.push((name.clone(), info));
                    self.push(now, Kind::Sys, &name, "CONNECTED", "");
                } else {
                    self.devices.retain(|d| d.0 != name);
                    self.push(now, Kind::Sys, &name, "REMOVED", "");
                }
            }
        }
        self.refresh_hold(now);
    }

    fn log_axis(&mut self, now: f64, axis: Axis, v: f32, act: &str, detail: &str) {
        let i = axis as usize;
        if (v - self.last_logged[i]).abs() >= AXIS_LOG_STEP
            || (v == 0.0 && self.last_logged[i] != 0.0)
        {
            self.last_logged[i] = v;
            self.push_axis(now, Kind::Axis, axis.label(), act, detail);
        }
    }

    /// Chamado a cada volta do laco: mantem a barra de saida animada.
    pub fn tick(&mut self, now: f64) {
        if self.hold_since.is_some() {
            let _ = now;
            self.dirty = true;
        }
    }

    /// Estado de exemplo (capturas de tela e testes).
    pub fn demo() -> State {
        let mut s = State::new(vec![ExitKey::Btn(Btn::Start), ExitKey::Btn(Btn::Home)]);
        s.devices
            .push(("RG52 Gamepad".into(), "17 btn, 6 axes, mapped".into()));
        let b = |btn, down, raw: &str| Input::Button {
            btn,
            down,
            raw: Some(raw.into()),
        };
        let mut t = 0.0;
        for (btn, raw) in [
            (Btn::B, "b0"),
            (Btn::A, "b1"),
            (Btn::Y, "b2"),
            (Btn::Up, "h0.1"),
            (Btn::L1, "b4"),
            (Btn::Select, "b8"),
            (Btn::Back, "key AC Back"),
            (Btn::L3, "b10"),
        ] {
            t += 0.9;
            s.apply(t, b(btn, true, raw));
            t += 0.15;
            s.apply(t, b(btn, false, raw));
        }
        for v in [8000i16, 16000, 24000, 32767] {
            t += 0.05;
            s.apply(
                t,
                Input::Axis {
                    axis: Axis::R2,
                    value: v,
                    raw: Some("a5".into()),
                },
            );
        }
        for (i, v) in [
            (0.0f32, -1.0f32),
            (0.7, -0.7),
            (1.0, 0.0),
            (0.7, 0.7),
            (0.0, 1.0),
            (-1.0, 0.0),
            (-0.5, -0.4),
        ]
        .iter()
        .enumerate()
        {
            t += 0.05 + i as f64 * 0.001;
            s.apply(
                t,
                Input::Axis {
                    axis: Axis::LX,
                    value: (v.0 * 32767.0) as i16,
                    raw: Some("a0".into()),
                },
            );
            s.apply(
                t,
                Input::Axis {
                    axis: Axis::LY,
                    value: (v.1 * 32767.0) as i16,
                    raw: Some("a1".into()),
                },
            );
        }
        t += 0.9;
        s.apply(t, b(Btn::X, true, "b3"));
        s.apply(
            t,
            Input::Axis {
                axis: Axis::RX,
                value: 12000,
                raw: Some("a2".into()),
            },
        );
        s.apply(
            t,
            Input::Axis {
                axis: Axis::RY,
                value: -9000,
                raw: Some("a3".into()),
            },
        );
        s.apply(
            t,
            Input::Axis {
                axis: Axis::L2,
                value: 20000,
                raw: Some("a4".into()),
            },
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> State {
        State::new(vec![ExitKey::Btn(Btn::Start), ExitKey::Btn(Btn::Home)])
    }

    fn press(s: &mut State, t: f64, b: Btn, down: bool) {
        s.apply(
            t,
            Input::Button {
                btn: b,
                down,
                raw: None,
            },
        );
    }

    fn axis(s: &mut State, t: f64, a: Axis, v: i16) {
        s.apply(
            t,
            Input::Axis {
                axis: a,
                value: v,
                raw: None,
            },
        );
    }

    #[test]
    fn botao_pressionado_vira_testado_e_continua_testado() {
        let mut s = st();
        assert_eq!(s.tested_count(), 0);
        press(&mut s, 0.0, Btn::A, true);
        assert!(s.is_pressed(Btn::A) && s.is_tested(Btn::A));
        press(&mut s, 0.1, Btn::A, false);
        assert!(!s.is_pressed(Btn::A) && s.is_tested(Btn::A));
        assert_eq!(s.tested_count(), 1);
        // pressionar de novo nao conta em dobro
        press(&mut s, 0.2, Btn::A, true);
        assert_eq!(s.tested_count(), 1);
        assert_eq!(State::TOTAL, 26);
    }

    #[test]
    fn log_registra_descida_e_subida_com_origem() {
        let mut s = st();
        s.apply(
            1.0,
            Input::Button {
                btn: Btn::Start,
                down: true,
                raw: Some("b9".into()),
            },
        );
        press(&mut s, 1.1, Btn::Start, false);
        let l: Vec<_> = s.log.iter().collect();
        assert_eq!(
            (l[0].name.as_str(), l[0].act.as_str(), l[0].detail.as_str()),
            ("START", "DOWN", "b9")
        );
        assert_eq!(l[1].act, "UP");
        assert_eq!(l[0].kind, Kind::Down);
        assert_eq!(l[1].kind, Kind::Up);
    }

    #[test]
    fn analogico_registra_direcoes_e_alcance() {
        let mut s = st();
        axis(&mut s, 0.0, Axis::LX, -32768);
        assert!(s.dirs[0][2] && !s.dirs[0][3]); // esquerda
        axis(&mut s, 0.1, Axis::LX, 32767);
        assert!(s.dirs[0][3]);
        axis(&mut s, 0.2, Axis::LY, -32768); // cima (SDL: negativo)
        assert!(s.dirs[0][0] && !s.dirs[0][1]);
        axis(&mut s, 0.3, Axis::LY, 32767);
        assert!(s.dirs[0][1]);
        assert!(s.dirs[0].iter().all(|d| *d));
        assert!(s.dirs[1].iter().all(|d| !*d)); // o outro analogico nao mexeu
        assert_eq!(s.tested_count(), 4);
        // alcance: algum setor chegou perto do raio 1
        assert!(s.reach[0].iter().cloned().fold(0.0, f32::max) > 0.95);
        assert_eq!(s.reach[1].iter().cloned().fold(0.0, f32::max), 0.0);
        // meia amplitude nao conta como direcao testada
        let mut s = st();
        axis(&mut s, 0.0, Axis::RX, 16000);
        assert!(!s.dirs[1][3]);
        assert!((s.stick[1].0 - 0.488).abs() < 0.01);
    }

    #[test]
    fn gatilho_so_conta_perto_do_fim_do_curso() {
        let mut s = st();
        axis(&mut s, 0.0, Axis::L2, 20000);
        assert!(!s.trig_tested[0]);
        assert!((s.trig[0] - 0.61).abs() < 0.01);
        axis(&mut s, 0.1, Axis::L2, 32767);
        assert!(s.trig_tested[0] && !s.trig_tested[1]);
        axis(&mut s, 0.2, Axis::L2, 0);
        assert!(s.trig_tested[0], "continua testado ao soltar");
        assert_eq!(s.trig[0], 0.0);
        // valor negativo (ruido) nao quebra
        axis(&mut s, 0.3, Axis::R2, -5);
        assert_eq!(s.trig[1], 0.0);
    }

    #[test]
    fn log_de_eixo_atualiza_no_lugar_e_ignora_ruido() {
        let mut s = st();
        for (i, v) in [4000i16, 8000, 12000, 16000].iter().enumerate() {
            axis(&mut s, i as f64 * 0.05, Axis::LX, *v);
        }
        assert_eq!(s.log.len(), 1, "rajada do mesmo eixo vira uma linha so");
        let (ver, act) = {
            let l = s.log.back().unwrap();
            (l.ver, l.act.clone())
        };
        assert!(ver >= 2 && act == "+0.49", "{ver} {act}");
        // ruido minimo (<3 %) nao entra
        let n = s.log.len();
        axis(&mut s, 0.3, Axis::LX, 16100);
        assert_eq!(s.log.len(), n);
        assert_eq!(s.log.back().unwrap().ver, ver);
        // depois de um tempo, vira linha nova
        axis(&mut s, 5.0, Axis::LX, 30000);
        assert_eq!(s.log.len(), n + 1);
        // outro eixo tambem abre linha nova
        axis(&mut s, 5.01, Axis::LY, 30000);
        assert_eq!(s.log.len(), n + 2);
        // voltar ao centro e registrado mesmo sendo variacao pequena a partir de ~0
        axis(&mut s, 9.0, Axis::LX, 0);
        assert_eq!(s.log.back().unwrap().act, "+0.00");
    }

    #[test]
    fn log_tem_limite() {
        let mut s = st();
        for i in 0..200 {
            press(&mut s, i as f64, Btn::B, i % 2 == 0);
        }
        assert_eq!(s.log.len(), LOG_MAX);
        assert!(s.log.front().unwrap().id > 100);
    }

    #[test]
    fn saida_exige_segurar_a_combinacao() {
        let mut s = st();
        press(&mut s, 0.0, Btn::Start, true);
        assert!(!s.holding_exit(), "so START nao basta");
        press(&mut s, 0.5, Btn::Home, true);
        assert!(s.holding_exit());
        assert!(!s.exit_ready(0.9));
        assert!((s.exit_progress(1.0) - 0.5).abs() < 1e-3);
        assert!(s.exit_ready(1.5));
        assert_eq!(s.exit_progress(9.0), 1.0);
        // soltar um dos dois cancela
        press(&mut s, 0.9, Btn::Home, false);
        assert!(!s.holding_exit() && !s.exit_ready(5.0));
        assert_eq!(s.exit_progress(5.0), 0.0);
        // apertar em sequencia rapida (teste normal dos dois botoes) nao sai
        let mut s = st();
        press(&mut s, 0.0, Btn::Start, true);
        press(&mut s, 0.1, Btn::Start, false);
        press(&mut s, 0.2, Btn::Home, true);
        press(&mut s, 0.3, Btn::Home, false);
        assert!(!s.exit_ready(10.0));
    }

    #[test]
    fn saida_por_botao_cru_e_pelo_esc() {
        let mut s = State::new(parse_exit("start+b12").unwrap());
        press(&mut s, 0.0, Btn::Start, true);
        s.apply(
            0.1,
            Input::Raw {
                what: "b12".into(),
                idx: Some(12),
                down: Some(true),
                value: None,
            },
        );
        assert!(s.holding_exit());
        s.apply(
            0.2,
            Input::Raw {
                what: "b12".into(),
                idx: Some(12),
                down: Some(false),
                value: None,
            },
        );
        assert!(!s.holding_exit());
        // Esc do teclado (PC) segura sozinho
        let mut s = st();
        s.apply(
            0.0,
            Input::Key {
                name: "Escape".into(),
                down: true,
            },
        );
        assert!(s.holding_exit());
        s.apply(
            0.4,
            Input::Key {
                name: "Escape".into(),
                down: false,
            },
        );
        assert!(!s.holding_exit());
    }

    #[test]
    fn parse_da_combinacao() {
        assert_eq!(
            parse_exit("START+Home").unwrap(),
            vec![ExitKey::Btn(Btn::Start), ExitKey::Btn(Btn::Home)]
        );
        assert_eq!(parse_exit("select + b7").unwrap()[1], ExitKey::Raw(7));
        assert_eq!(
            parse_exit("start+back").unwrap()[1],
            ExitKey::Btn(Btn::Select)
        );
        assert!(parse_exit("start").is_err());
        assert!(parse_exit("start+voar").is_err());
        assert_eq!(ExitKey::Raw(12).label(), "B12");
    }

    #[test]
    fn teclas_de_volume_e_dispositivos() {
        let mut s = st();
        s.apply(
            0.0,
            Input::Key {
                name: "VolumeUp".into(),
                down: true,
            },
        );
        assert!(s.vol_up && s.vol_up_seen && !s.vol_down_seen);
        s.apply(
            0.1,
            Input::Key {
                name: "VolumeUp".into(),
                down: false,
            },
        );
        assert!(!s.vol_up && s.vol_up_seen);
        s.apply(
            0.2,
            Input::Device {
                name: "Pad".into(),
                info: "17 buttons".into(),
                added: true,
            },
        );
        assert_eq!(s.devices.len(), 1);
        s.apply(
            0.3,
            Input::Device {
                name: "Pad".into(),
                info: String::new(),
                added: false,
            },
        );
        assert!(s.devices.is_empty());
        assert_eq!(s.log.back().unwrap().act, "REMOVED");
    }

    #[test]
    fn controle_sem_mapeamento_vai_para_o_log_cru() {
        let mut s = st();
        s.apply(
            0.0,
            Input::Raw {
                what: "b12".into(),
                idx: Some(12),
                down: Some(true),
                value: None,
            },
        );
        let l = s.log.back().unwrap();
        assert_eq!(
            (l.kind, l.act.as_str(), l.detail.as_str()),
            (Kind::Raw, "DOWN", "not mapped")
        );
        assert_eq!(s.tested_count(), 0);
    }

    #[test]
    fn map_manual_e_teclas_equivalentes() {
        assert_eq!(
            parse_map("back=b13, HOME=b12").unwrap(),
            vec![(13, Btn::Back), (12, Btn::Home)]
        );
        assert!(parse_map("").unwrap().is_empty());
        assert!(parse_map("back").is_err());
        assert!(parse_map("voar=b1").is_err());
        assert!(parse_map("back=13").is_err());
        assert_eq!(key_as_button("AC Back"), Some(Btn::Back));
        assert_eq!(key_as_button("A"), None);
    }

    #[test]
    fn mapeamento_sdl_e_lido() {
        let m = "0300,RG52 Gamepad,a:b1,b:b0,x:b3,y:b2,back:b8,guide:b12,start:b9,leftstick:b10,\
dpup:h0.1,dpleft:h0.8,leftx:a0,lefty:a1~,lefttrigger:+a4,righttrigger:a5,platform:Linux,";
        let b = parse_mapping(m);
        assert_eq!(b.by_name["a"], "b1");
        assert_eq!(b.by_name["guide"], "b12");
        assert_eq!(b.by_name["dpup"], "h0.1");
        assert_eq!(b.by_name["lefty"], "a1");
        assert_eq!(b.by_name["lefttrigger"], "a4");
        assert!(b.buttons.contains(&12) && b.buttons.contains(&0) && !b.buttons.contains(&13));
        assert!(b.axes.contains(&1) && b.axes.contains(&5));
        assert!(b.hats.contains(&0));
        assert!(!b.by_name.contains_key("platform"));
        assert_eq!(parse_mapping(""), Binds::default());
        assert_eq!(parse_mapping("so,duas").by_name.len(), 0);
    }

    #[test]
    fn eixo_cru_coalesce_e_ignora_ruido() {
        let mut s = st();
        let raw = |v| Input::Raw {
            what: "a6".into(),
            idx: None,
            down: None,
            value: Some(v),
        };
        for (i, v) in [3000i16, 6000, 9000].iter().enumerate() {
            s.apply(i as f64 * 0.05, raw(*v));
        }
        assert_eq!(s.log.len(), 1);
        s.apply(0.3, raw(9300)); // ruido
        assert_eq!(s.log.len(), 1);
        s.apply(0.4, raw(0));
        assert_eq!(s.log.back().unwrap().act, "+0.00");
    }

    #[test]
    fn progresso_conta_so_o_que_o_aparelho_tem() {
        let mut buttons = [false; NB];
        buttons[Btn::A.idx()] = true;
        buttons[Btn::R3.idx()] = true;
        let c = Controls {
            buttons,
            sticks: [true, false],
            triggers: false,
            alias: Some((Btn::R3, "F")),
        };
        let mut s = State::with_controls(vec![], c);
        assert_eq!(s.total(), 6);
        press(&mut s, 0.0, Btn::A, true);
        press(&mut s, 0.1, Btn::B, true); // nao existe: vai ao log, mas nao conta
        axis(&mut s, 0.2, Axis::RX, 32767); // analogico ausente
        axis(&mut s, 0.3, Axis::L2, 32767); // gatilho ausente
        axis(&mut s, 0.4, Axis::LX, 32767);
        assert_eq!(s.tested_count(), 2);
        assert!(s.log.iter().any(|l| l.name == "B"));
        press(&mut s, 0.5, Btn::R3, true);
        assert_eq!(s.log.back().unwrap().name, "F");
    }

    #[test]
    fn demo_e_consistente() {
        let s = State::demo();
        assert!(s.tested_count() >= 10 && s.tested_count() < State::TOTAL);
        assert!(s.is_pressed(Btn::X));
        assert!(!s.log.is_empty());
    }
}
