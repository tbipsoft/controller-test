#![allow(clippy::too_many_arguments)]
//! Tela do teste: o aparelho desenhado de frente, na disposicao real do RG52 Mini
//! (HOME e BACK ao lado da tela, analogico acima do direcional a esquerda, ABXY acima do
//! analogico a direita, SELECT/START embaixo, ombros no topo), com o log de eventos no
//! lugar da tela. Desenho por software em um `Pixmap` (design 960x540, escalado e
//! centralizado); o `main` so envia a textura.
//!
//! Custo: a parte fixa fica em cache e o log guarda cada linha ja rasterizada, entao um
//! quadro e so copia + formas simples (nada de brilhos nem de texto longo por quadro).

use std::collections::HashMap;

use crate::gfx::{Color, Face, Fonts, Pixmap};
use crate::pad::{Btn, ExitKey, Kind, Line, State, EXIT_HOLD, SECTORS};

pub const DESIGN_W: f32 = 960.0;
pub const DESIGN_H: f32 = 540.0;

const BG: Color = Color::hex(0x0B0D12);
const TEXT: Color = Color::hex(0xE8ECF4);
const MUTED: Color = Color::hex(0x8E98AD);
const LINE: Color = Color::hex(0x1D2332);
const CARD: Color = Color::hex(0x12161F);
const DARK: Color = Color::hex(0x0B0E14);
const WELL_EDGE: Color = Color::hex(0x222939);
const TRACK: Color = Color::hex(0x242B3B);
const KEY_FILL: Color = Color::hex(0x1A2030);
const KEY_EDGE: Color = Color::hex(0x2A3144);
const TESTED_FILL: Color = Color::hex(0x15291F);
const TESTED_EDGE: Color = Color::hex(0x2E7D4F);
const OK: Color = Color::hex(0x4ADE80);
const BAD: Color = Color::hex(0xFF6B6B);
const DOT: Color = Color::hex(0x3A4256);
const AXIS_C: Color = Color::hex(0x7DD3FC);
const RAW_C: Color = Color::hex(0xFF9F43);
const KEY_C: Color = Color::hex(0xC4B5FD);
pub const ACCENT: Color = Color::hex(0xFFB938);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// A a direita, B embaixo, X em cima, Y a esquerda (e o do RG52 Mini).
    Nintendo,
    /// A embaixo, B a direita, X a esquerda, Y em cima.
    Xbox,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub accent: Color,
    pub layout: Layout,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            accent: ACCENT,
            layout: Layout::Nintendo,
        }
    }
}

pub fn parse_accent(s: &str) -> Result<Color, String> {
    match s.to_ascii_lowercase().as_str() {
        "amber" => Ok(ACCENT),
        "teal" => Ok(Color::hex(0x5EEAD4)),
        "violet" => Ok(Color::hex(0xA78BFA)),
        "coral" => Ok(Color::hex(0xFF7A59)),
        t => {
            let h = t.strip_prefix('#').unwrap_or(t);
            match (h.len(), u32::from_str_radix(h, 16)) {
                (6, Ok(v)) => Ok(Color::hex(v)),
                _ => Err(format!(
                    "cor invalida: {s:?} (use amber, teal, violet, coral ou #RRGGBB)"
                )),
            }
        }
    }
}

pub fn parse_layout(s: &str) -> Result<Layout, String> {
    match s.to_ascii_lowercase().as_str() {
        "nintendo" => Ok(Layout::Nintendo),
        "xbox" => Ok(Layout::Xbox),
        _ => Err(format!("--layout invalido: {s:?} (use nintendo ou xbox)")),
    }
}

#[derive(Clone, Copy)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

const fn r(x: f32, y: f32, w: f32, h: f32) -> R {
    R { x, y, w, h }
}

// --- geometria (coordenadas do design), seguindo a vista frontal do manual -------------
const BODY: R = r(40.0, 54.0, 880.0, 418.0);
const BODY_RAD: f32 = 56.0;
const SCREEN: R = r(268.0, 66.0, 424.0, 394.0);
const LSTICK: (f32, f32) = (140.0, 168.0);
const RSTICK: (f32, f32) = (790.0, 330.0);
const STICK_R: f32 = 44.0;
const TRAVEL: f32 = 30.0;
const TICK_R: f32 = STICK_R + 9.0;
const DPAD: (f32, f32) = (148.0, 330.0);
const ABXY: (f32, f32) = (812.0, 184.0);
const LOG_X: f32 = 284.0;
const LOG_W: f32 = 392.0;
const LOG_Y: f32 = 82.0;
const LOG_PITCH: f32 = 30.0;
const LOG_ROWS: usize = 12;
const COL_NAME: f32 = 64.0;
const COL_ACT: f32 = 160.0;
const COL_SRC: f32 = 256.0;
const EXIT_CARD: R = r(40.0, 484.0, 322.0, 48.0);
const INFO_CARD: R = r(570.0, 484.0, 350.0, 48.0);

fn shoulder(b: Btn) -> R {
    match b {
        Btn::L1 => r(92.0, 84.0, 112.0, 24.0),
        _ => r(756.0, 84.0, 112.0, 24.0),
    }
}

fn trigger(right: bool) -> R {
    if right {
        r(756.0, 64.0, 112.0, 16.0)
    } else {
        r(92.0, 64.0, 112.0, 16.0)
    }
}

fn dpad_arm(b: Btn) -> R {
    let (cx, cy, h, l) = (DPAD.0, DPAD.1, 14.0, 30.0);
    match b {
        Btn::Up => r(cx - h, cy - h - l, 2.0 * h, l),
        Btn::Down => r(cx - h, cy + h, 2.0 * h, l),
        Btn::Left => r(cx - h - l, cy - h, l, 2.0 * h),
        _ => r(cx + h, cy - h, l, 2.0 * h),
    }
}

fn face_pos(b: Btn, layout: Layout) -> (f32, f32) {
    let d = 36.0;
    let (dx, dy) = match (layout, b) {
        (Layout::Nintendo, Btn::X) | (Layout::Xbox, Btn::Y) => (0.0, -d),
        (Layout::Nintendo, Btn::B) | (Layout::Xbox, Btn::A) => (0.0, d),
        (Layout::Nintendo, Btn::Y) | (Layout::Xbox, Btn::X) => (-d, 0.0),
        _ => (d, 0.0),
    };
    (ABXY.0 + dx, ABXY.1 + dy)
}

/// Botoes pequenos em forma de pilula: HOME/BACK ao lado da tela, SELECT/START embaixo.
fn pill(b: Btn) -> R {
    match b {
        Btn::Home => r(212.0, 72.0, 48.0, 24.0),
        Btn::Back => r(700.0, 72.0, 48.0, 24.0),
        Btn::Select => r(108.0, 424.0, 80.0, 24.0),
        _ => r(750.0, 424.0, 80.0, 24.0),
    }
}

/// Logo (RGBA, 264x56 = 2x do tamanho no design) e o tamanho em que e desenhado.
const LOGO: &[u8] = include_bytes!("../assets/logo_264x56.rgba");
const LOGO_SRC: (usize, usize) = (264, 56);
const LOGO_W: f32 = 132.0;
const LOGO_H: f32 = 27.8;

// --- pintor ----------------------------------------------------------------------
struct P<'a> {
    pm: &'a mut Pixmap,
    f: &'a Fonts,
    k: f32,
    ox: f32,
    oy: f32,
}

impl P<'_> {
    fn rrect(&mut self, r: R, rad: f32, c: Color) {
        let k = self.k;
        self.pm.fill_rrect(
            self.ox + r.x * k,
            self.oy + r.y * k,
            r.w * k,
            r.h * k,
            rad * k,
            c,
        );
    }
    fn stroke(&mut self, r: R, rad: f32, bw: f32, c: Color) {
        let k = self.k;
        self.pm.stroke_rrect(
            self.ox + r.x * k,
            self.oy + r.y * k,
            r.w * k,
            r.h * k,
            rad * k,
            bw * k,
            c,
        );
    }
    fn circle(&mut self, cx: f32, cy: f32, rad: f32, c: Color) {
        let k = self.k;
        self.pm
            .fill_circle(self.ox + cx * k, self.oy + cy * k, rad * k, c);
    }
    fn ring(&mut self, cx: f32, cy: f32, rad: f32, bw: f32, c: Color) {
        let k = self.k;
        self.pm
            .stroke_circle(self.ox + cx * k, self.oy + cy * k, rad * k, bw * k, c);
    }
    fn width(&self, face: Face, size: f32, sp: f32, s: &str) -> f32 {
        self.f.width(face, size * self.k, sp * self.k, s) / self.k
    }
    fn text(
        &mut self,
        face: Face,
        size: f32,
        sp: f32,
        c: Color,
        x: f32,
        baseline: f32,
        s: &str,
    ) -> f32 {
        let k = self.k;
        self.pm.text(
            self.f,
            face,
            size * k,
            sp * k,
            c,
            self.ox + x * k,
            self.oy + baseline * k,
            s,
        ) / k
    }
    fn text_mid(
        &mut self,
        face: Face,
        size: f32,
        sp: f32,
        c: Color,
        x: f32,
        cy: f32,
        s: &str,
    ) -> f32 {
        let (a, d) = self.f.metrics(face);
        self.text(face, size, sp, c, x, cy + (a + d) * size / 2.0, s)
    }
    /// Centrado em `cx`.
    fn text_c(&mut self, face: Face, size: f32, sp: f32, c: Color, cx: f32, cy: f32, s: &str) {
        let w = self.width(face, size, sp, s);
        self.text_mid(face, size, sp, c, cx - w / 2.0, cy, s);
    }
    fn text_right(
        &mut self,
        face: Face,
        size: f32,
        c: Color,
        right: f32,
        cy: f32,
        s: &str,
        max_w: f32,
    ) {
        let s = self.f.fit(face, size * self.k, 0.0, s, max_w * self.k);
        let w = self.width(face, size, 0.0, &s);
        self.text_mid(face, size, 0.0, c, right - w, cy, &s);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Vis {
    Idle,
    Tested,
    Pressed,
}

fn vis(st: &State, b: Btn) -> Vis {
    if st.is_pressed(b) {
        Vis::Pressed
    } else if st.is_tested(b) {
        Vis::Tested
    } else {
        Vis::Idle
    }
}

/// (preenchimento, borda, texto) de um controle conforme o estado.
fn colors(v: Vis, accent: Color) -> (Color, Color, Color) {
    match v {
        Vis::Idle => (KEY_FILL, KEY_EDGE, MUTED),
        Vis::Tested => (TESTED_FILL, TESTED_EDGE, OK),
        Vis::Pressed => (accent, accent, DARK),
    }
}

pub struct Renderer {
    fonts: Fonts,
    w: usize,
    h: usize,
    k: f32,
    ox: f32,
    oy: f32,
    statics: Pixmap,
    statics_key: Option<(Color, Vec<String>)>,
    frame: Pixmap,
    strips: HashMap<(u64, u32), Pixmap>,
}

impl Renderer {
    pub fn new(w: usize, h: usize) -> Renderer {
        let mut r = Renderer {
            fonts: Fonts::new(),
            w: 0,
            h: 0,
            k: 1.0,
            ox: 0.0,
            oy: 0.0,
            statics: Pixmap::new(1, 1),
            statics_key: None,
            frame: Pixmap::new(1, 1),
            strips: HashMap::new(),
        };
        r.resize(w, h);
        r
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        let (w, h) = (w.max(1), h.max(1));
        self.w = w;
        self.h = h;
        self.k = (w as f32 / DESIGN_W).min(h as f32 / DESIGN_H);
        self.ox = (w as f32 - DESIGN_W * self.k) / 2.0;
        self.oy = (h as f32 - DESIGN_H * self.k) / 2.0;
        self.statics = Pixmap::new(w, h);
        self.frame = Pixmap::new(w, h);
        self.statics_key = None;
        self.strips.clear();
    }

    pub fn frame(&self) -> &Pixmap {
        &self.frame
    }

    pub fn scale(&self) -> f32 {
        self.k
    }

    fn draw_statics(&mut self, cfg: &Config, combo: &[ExitKey]) {
        self.statics.clear(BG);
        let mut p = P {
            pm: &mut self.statics,
            f: &self.fonts,
            k: self.k,
            ox: self.ox,
            oy: self.oy,
        };
        let acc = cfg.accent;
        // cabecalho compacto
        let cy = 26.0;
        p.ring(30.0, cy, 6.0, 2.0, TEXT);
        p.ring(41.0, cy, 6.0, 2.0, acc);
        let w1 = p.text_mid(Face::Sans800, 20.0, -0.4, TEXT, 58.0, cy, "RG52");
        p.text_mid(
            Face::Sans500,
            20.0,
            -0.4,
            MUTED,
            58.0 + w1 + 5.0,
            cy,
            "CONTROLLER TEST",
        );
        // logo da DiferSoft, centralizado entre o titulo e o contador
        let (lw, lh) = (LOGO_W, LOGO_H);
        let logo = Pixmap::from_rgba_scaled(
            LOGO,
            LOGO_SRC.0,
            LOGO_SRC.1,
            (lw * self.k).round() as usize,
            (lh * self.k).round() as usize,
        );
        p.pm.blit(
            &logo,
            (p.ox + (480.0 - lw / 2.0) * p.k).round() as i32,
            (p.oy + (26.0 - lh / 2.0) * p.k).round() as i32,
            1.0,
        );
        // versao no rodape, entre os dois cartoes
        let ver = concat!("v", env!("CARGO_PKG_VERSION"));
        p.text_c(Face::Mono500, 13.0, 0.0, MUTED, 466.0, 508.0, ver);
        // corpo do aparelho e "tela" (so o log, sem titulo)
        p.rrect(BODY, BODY_RAD, CARD);
        p.stroke(BODY, BODY_RAD, 1.0, LINE);
        p.rrect(SCREEN, 18.0, DARK);
        p.stroke(SCREEN, 18.0, 1.0, WELL_EDGE);
        // trilhos dos gatilhos
        for right in [false, true] {
            p.rrect(trigger(right), 8.0, TRACK);
        }
        // cartoes do rodape
        for c in [EXIT_CARD, INFO_CARD] {
            p.rrect(c, 14.0, CARD);
            p.stroke(c, 14.0, 1.0, LINE);
        }
        // teclas da combinacao de saida
        let mut x = EXIT_CARD.x + 14.0;
        let y = EXIT_CARD.y + 8.0;
        for (i, key) in combo.iter().enumerate() {
            if i > 0 {
                x += 6.0;
                let w = p.text_mid(Face::Sans700, 16.0, 0.0, MUTED, x, y + 13.0, "+");
                x += w + 6.0;
            }
            let label = key.label();
            let kw = (p.width(Face::Mono700, 13.0, 0.0, &label) + 20.0).max(46.0);
            let kr = r(x, y, kw, 26.0);
            p.rrect(kr, 8.0, Color::hex(0x1B2130));
            p.stroke(kr, 8.0, 1.0, Color::hex(0x2A3144));
            p.text_c(
                Face::Mono700,
                13.0,
                0.0,
                TEXT,
                x + kw / 2.0,
                y + 13.0,
                &label,
            );
            x += kw;
        }
        p.text_mid(
            Face::Sans700,
            13.0,
            0.0,
            MUTED,
            x + 12.0,
            y + 13.0,
            &format!("hold {EXIT_HOLD:.0} s to exit"),
        );
        p.text_mid(
            Face::Sans800,
            11.0,
            1.5,
            acc,
            INFO_CARD.x + 14.0,
            INFO_CARD.y + 15.0,
            "FREE MODE",
        );
    }

    /// Redesenha tudo (barato: ver o comentario do modulo) e devolve o quadro.
    pub fn draw(&mut self, st: &State, now: f64, cfg: &Config) -> &Pixmap {
        let key = (
            cfg.accent,
            st.exit_combo.iter().map(|k| k.label()).collect::<Vec<_>>(),
        );
        if self.statics_key.as_ref() != Some(&key) {
            self.draw_statics(cfg, &st.exit_combo);
            self.statics_key = Some(key);
        }
        self.frame.copy_from(&self.statics);
        self.draw_log(st);
        let (k, ox, oy) = (self.k, self.ox, self.oy);
        let mut p = P {
            pm: &mut self.frame,
            f: &self.fonts,
            k,
            ox,
            oy,
        };
        let acc = cfg.accent;

        // --- cabecalho: contador de itens testados (compacto) ---
        let n = st.tested_count();
        let txt = format!("{n}/{}", State::TOTAL);
        let tw = p.width(Face::Mono700, 20.0, 0.0, &txt);
        let col = if n == State::TOTAL { OK } else { TEXT };
        p.text_mid(Face::Mono700, 20.0, 0.0, col, 930.0 - tw, 26.0, &txt);
        let bar = r(930.0 - tw - 12.0 - 90.0, 23.0, 90.0, 6.0);
        p.rrect(bar, 3.0, TRACK);
        let fw = bar.w * n as f32 / State::TOTAL as f32;
        if fw > 0.0 {
            p.rrect(
                R {
                    w: fw.max(6.0),
                    ..bar
                },
                3.0,
                if n == State::TOTAL { OK } else { acc },
            );
        }
        let lw = p.width(Face::Sans800, 12.0, 1.2, "TESTED");
        p.text_mid(
            Face::Sans800,
            12.0,
            1.2,
            MUTED,
            bar.x - lw - 10.0,
            26.0,
            "TESTED",
        );

        // --- ombros (L1/R1) e gatilhos analogicos (L2/R2) ---
        for b in [Btn::L1, Btn::R1] {
            let rc = shoulder(b);
            let (f, e, t) = colors(vis(st, b), acc);
            p.rrect(rc, 9.0, f);
            p.stroke(rc, 9.0, 1.5, e);
            p.text_c(
                Face::Mono700,
                15.0,
                0.0,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                b.label(),
            );
        }
        for (i, name) in ["L2", "R2"].into_iter().enumerate() {
            let rc = trigger(i == 1);
            let v = st.trig[i];
            if v > 0.0 {
                p.rrect(
                    R {
                        w: (rc.w * v).max(6.0),
                        ..rc
                    },
                    7.0,
                    acc,
                );
            }
            if st.trig_tested[i] {
                p.stroke(rc, 7.0, 1.5, TESTED_EDGE);
            }
            let c = if v > 0.5 {
                DARK
            } else if st.trig_tested[i] {
                OK
            } else {
                MUTED
            };
            p.text_mid(
                Face::Mono700,
                12.0,
                0.0,
                c,
                rc.x + 8.0,
                rc.y + rc.h / 2.0,
                name,
            );
            let pct = format!("{:.0}%", v * 100.0);
            let w = p.width(Face::Mono700, 12.0, 0.0, &pct);
            let c2 = if v > 0.92 { DARK } else { MUTED };
            p.text_mid(
                Face::Mono700,
                12.0,
                0.0,
                c2,
                rc.x + rc.w - 8.0 - w,
                rc.y + rc.h / 2.0,
                &pct,
            );
        }

        // --- direcional ---
        let (f, e, _) = colors(Vis::Idle, acc);
        let hub = r(DPAD.0 - 13.0, DPAD.1 - 13.0, 26.0, 26.0);
        p.rrect(hub, 4.0, f);
        p.stroke(hub, 4.0, 1.5, e);
        for (b, g) in [
            (Btn::Up, "^"),
            (Btn::Down, "v"),
            (Btn::Left, "<"),
            (Btn::Right, ">"),
        ] {
            let rc = dpad_arm(b);
            let (f, e, t) = colors(vis(st, b), acc);
            p.rrect(rc, 7.0, f);
            p.stroke(rc, 7.0, 1.5, e);
            p.text_c(
                Face::Mono700,
                16.0,
                0.0,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                g,
            );
        }

        // --- botoes de face ---
        for b in [Btn::A, Btn::B, Btn::X, Btn::Y] {
            let (cx, cy) = face_pos(b, cfg.layout);
            let (f, e, t) = colors(vis(st, b), acc);
            p.circle(cx, cy, 16.0, f);
            p.ring(cx, cy, 15.25, 1.5, e);
            p.text_c(Face::Sans800, 17.0, 0.0, t, cx, cy, b.label());
        }

        // --- analogicos ---
        stick(&mut p, LSTICK, 0, st, Btn::L3, acc);
        stick(&mut p, RSTICK, 1, st, Btn::R3, acc);

        // --- HOME / BACK (ao lado da tela) e SELECT / START (embaixo) ---
        for b in [Btn::Home, Btn::Back, Btn::Select, Btn::Start] {
            let rc = pill(b);
            let (f, e, t) = colors(vis(st, b), acc);
            p.rrect(rc, 10.0, f);
            p.stroke(rc, 10.0, 1.5, e);
            p.text_c(
                Face::Sans800,
                12.0,
                0.6,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                b.label(),
            );
        }

        // --- mensagem quando ainda nao ha eventos ---
        if st.log.is_empty() {
            let cx = SCREEN.x + SCREEN.w / 2.0;
            let cy = SCREEN.y + SCREEN.h / 2.0;
            if st.devices.is_empty() {
                p.text_c(
                    Face::Sans800,
                    22.0,
                    0.0,
                    BAD,
                    cx,
                    cy - 12.0,
                    "No controller detected",
                );
                p.text_c(
                    Face::Sans600,
                    15.0,
                    0.0,
                    MUTED,
                    cx,
                    cy + 12.0,
                    "Connect one or check the SDL controller mapping",
                );
            } else {
                p.text_c(
                    Face::Sans800,
                    22.0,
                    0.0,
                    TEXT,
                    cx,
                    cy - 12.0,
                    "Press any button",
                );
                p.text_c(
                    Face::Sans600,
                    15.0,
                    0.0,
                    MUTED,
                    cx,
                    cy + 12.0,
                    "Move the sticks and squeeze the triggers",
                );
            }
        }

        // --- cartao de saida: so a barra de progresso ---
        let prog = st.exit_progress(now);
        if st.holding_exit() {
            p.stroke(EXIT_CARD, 14.0, 2.0, OK);
        } else {
            p.stroke(EXIT_CARD, 14.0, 1.5, acc);
        }
        let pb = r(
            EXIT_CARD.x + 14.0,
            EXIT_CARD.y + EXIT_CARD.h - 10.0,
            EXIT_CARD.w - 28.0,
            4.0,
        );
        p.rrect(pb, 2.0, TRACK);
        if prog > 0.0 {
            p.rrect(
                R {
                    w: (pb.w * prog).max(4.0),
                    ..pb
                },
                2.0,
                OK,
            );
        }

        // --- cartao de informacoes (compacto: modo + aparelho, depois VOL e dados) ---
        let ix = INFO_CARD.x + 14.0;
        let iw = INFO_CARD.w - 28.0;
        let (name, info) = match st.devices.first() {
            Some((n, i)) => {
                let extra = st.devices.len() - 1;
                (
                    if extra > 0 {
                        format!("{n}  (+{extra})")
                    } else {
                        n.clone()
                    },
                    i.clone(),
                )
            }
            None => ("No device".to_string(), String::new()),
        };
        p.text_right(
            Face::Sans700,
            14.0,
            TEXT,
            ix + iw,
            INFO_CARD.y + 15.0,
            &name,
            iw - 84.0,
        );
        for (i, (label, down, seen)) in [
            ("VOL-", st.vol_down, st.vol_down_seen),
            ("VOL+", st.vol_up, st.vol_up_seen),
        ]
        .into_iter()
        .enumerate()
        {
            let rc = r(ix + i as f32 * 52.0, INFO_CARD.y + 27.0, 46.0, 16.0);
            let v = if down {
                Vis::Pressed
            } else if seen {
                Vis::Tested
            } else {
                Vis::Idle
            };
            let (f, e, t) = colors(v, acc);
            p.rrect(rc, 8.0, f);
            p.stroke(rc, 8.0, 1.5, e);
            p.text_c(
                Face::Sans800,
                11.0,
                0.4,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                label,
            );
        }
        let ev = format!("{} ev", st.events);
        let tail = if info.is_empty() {
            ev
        } else {
            format!("{ev}  {info}")
        };
        p.text_right(
            Face::Mono500,
            12.0,
            MUTED,
            ix + iw,
            INFO_CARD.y + 35.0,
            &tail,
            iw - 112.0,
        );
        &self.frame
    }

    fn make_strip(&self, l: &Line) -> Pixmap {
        let (w, h) = (
            (LOG_W * self.k).ceil() as usize + 1,
            (LOG_PITCH * self.k).ceil() as usize,
        );
        let mut pm = Pixmap::new(w, h);
        pm.clear(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        let mut p = P {
            pm: &mut pm,
            f: &self.fonts,
            k: self.k,
            ox: 0.0,
            oy: 0.0,
        };
        let cy = LOG_PITCH / 2.0;
        let (act_c, name_c) = match l.kind {
            Kind::Down => (ACCENT, TEXT),
            Kind::Up => (MUTED, MUTED),
            Kind::Axis => (AXIS_C, TEXT),
            Kind::Raw => (RAW_C, RAW_C),
            Kind::Key => (KEY_C, TEXT),
            Kind::Sys => (OK, OK),
        };
        let fit = |face, size: f32, s: &str, max: f32| {
            self.fonts.fit(face, size * self.k, 0.0, s, max * self.k)
        };
        p.text_mid(
            Face::Mono500,
            13.0,
            0.0,
            MUTED,
            0.0,
            cy,
            &format!("{:6.1}", l.t),
        );
        let name = fit(Face::Mono700, 16.0, &l.name, COL_ACT - COL_NAME - 6.0);
        p.text_mid(Face::Mono700, 16.0, 0.0, name_c, COL_NAME, cy, &name);
        let act = fit(Face::Mono700, 16.0, &l.act, COL_SRC - COL_ACT - 6.0);
        p.text_mid(Face::Mono700, 16.0, 0.0, act_c, COL_ACT, cy, &act);
        let det = fit(Face::Mono500, 13.0, &l.detail, LOG_W - COL_SRC);
        p.text_mid(Face::Mono500, 13.0, 0.0, MUTED, COL_SRC, cy, &det);
        pm
    }

    fn draw_log(&mut self, st: &State) {
        let rows: Vec<&Line> = st
            .log
            .iter()
            .rev()
            .take(LOG_ROWS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        for l in &rows {
            if !self.strips.contains_key(&(l.id, l.ver)) {
                let s = self.make_strip(l);
                self.strips.insert((l.id, l.ver), s);
            }
        }
        let n = rows.len();
        for (i, l) in rows.iter().enumerate() {
            // a mais nova embaixo; as antigas esmaecem
            let age = (n - 1 - i) as f32;
            let op = (1.0 - age * 0.06).clamp(0.3, 1.0);
            let x = (self.ox + LOG_X * self.k).round() as i32;
            let y =
                (self.oy + (LOG_Y + (LOG_ROWS - n + i) as f32 * LOG_PITCH) * self.k).round() as i32;
            if let Some(s) = self.strips.get(&(l.id, l.ver)) {
                self.frame.blit(s, x, y, op);
            }
        }
        if self.strips.len() > 80 {
            let keep: Vec<(u64, u32)> = rows.iter().map(|l| (l.id, l.ver)).collect();
            self.strips.retain(|k, _| keep.contains(k));
        }
    }
}

fn stick(p: &mut P, c: (f32, f32), idx: usize, st: &State, click: Btn, acc: Color) {
    let (cx, cy) = c;
    p.circle(cx, cy, STICK_R, WELL_EDGE);
    p.circle(cx, cy, STICK_R - 1.0, DARK);
    p.ring(cx, cy, TRAVEL, 1.0, LINE);
    // bordas alcancadas (mostra se o curso e redondo e completo)
    for s in 0..SECTORS {
        let r = st.reach[idx][s];
        if r >= 0.05 {
            let a =
                (s as f32 + 0.5) / SECTORS as f32 * std::f32::consts::TAU - std::f32::consts::PI;
            p.circle(
                cx + a.cos() * r * TRAVEL,
                cy + a.sin() * r * TRAVEL,
                1.5,
                TESTED_EDGE,
            );
        }
    }
    // marcas das 4 direcoes: acendem quando o analogico chegou ao fim naquele lado
    for (d, (dx, dy)) in [(0.0f32, -1.0f32), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)]
        .into_iter()
        .enumerate()
    {
        let on = st.dirs[idx][d];
        p.circle(
            cx + dx * TICK_R,
            cy + dy * TICK_R,
            3.2,
            if on { OK } else { KEY_EDGE },
        );
    }
    // o botao do analogico (clique = L3/R3)
    let (x, y) = st.stick[idx];
    let len = (x * x + y * y).sqrt();
    let s = if len > 1.0 { 1.0 / len } else { 1.0 };
    let (px, py) = (cx + x * s * TRAVEL, cy + y * s * TRAVEL);
    let (f, e, t) = match vis(st, click) {
        Vis::Idle => (DOT, KEY_EDGE, MUTED),
        other => colors(other, acc),
    };
    p.circle(px, py, 15.0, f);
    p.ring(px, py, 14.25, 1.5, e);
    p.text_c(Face::Mono700, 12.0, 0.0, t, px, py, click.label());
    let txt = format!("X {x:+.2}  Y {y:+.2}");
    p.text_c(
        Face::Mono500,
        13.0,
        0.0,
        MUTED,
        cx,
        cy + TICK_R + 16.0,
        &txt,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::{Axis, Input};

    fn near(a: [u8; 4], b: Color, tol: i32) -> bool {
        (a[0] as i32 - b.r as i32).abs() <= tol
            && (a[1] as i32 - b.g as i32).abs() <= tol
            && (a[2] as i32 - b.b as i32).abs() <= tol
    }

    fn st() -> State {
        State::new(vec![ExitKey::Btn(Btn::Start), ExitKey::Btn(Btn::Home)])
    }

    fn shot(s: &State, now: f64, cfg: &Config) -> Renderer {
        let mut r = Renderer::new(960, 540);
        r.draw(s, now, cfg);
        r
    }

    fn at(r: &Renderer, x: f32, y: f32) -> [u8; 4] {
        r.frame()
            .get(x.round() as usize, y.round() as usize)
            .unwrap()
    }

    fn press(s: &mut State, b: Btn, down: bool) {
        s.apply(
            0.0,
            Input::Button {
                btn: b,
                down,
                raw: None,
            },
        );
    }

    #[test]
    fn disposicao_fisica_do_rg52_mini() {
        // esquerda: analogico acima do direcional acima do SELECT
        assert!(LSTICK.1 < DPAD.1 && DPAD.1 < pill(Btn::Select).y);
        // direita: ABXY acima do analogico acima do START
        assert!(ABXY.1 < RSTICK.1 && RSTICK.1 < pill(Btn::Start).y);
        // HOME a esquerda da tela e BACK a direita, no alto
        let (h, b) = (pill(Btn::Home), pill(Btn::Back));
        assert!(h.x + h.w <= SCREEN.x && b.x >= SCREEN.x + SCREEN.w);
        assert!(h.y < LSTICK.1 && b.y < ABXY.1);
        // SELECT/START ficam fora da tela, dos lados
        assert!(pill(Btn::Select).x + pill(Btn::Select).w < SCREEN.x);
        assert!(pill(Btn::Start).x > SCREEN.x + SCREEN.w);
        // L1/L2 a esquerda e R1/R2 a direita; gatilho acima do ombro
        assert!(shoulder(Btn::L1).x < SCREEN.x && shoulder(Btn::R1).x > SCREEN.x + SCREEN.w);
        assert!(trigger(false).y < shoulder(Btn::L1).y);
        // tudo dentro do corpo (exceto os gatilhos, que saem um pouco por cima)
        let inside = |rc: R| {
            rc.x >= BODY.x && rc.x + rc.w <= BODY.x + BODY.w && rc.y + rc.h <= BODY.y + BODY.h
        };
        for b in [
            Btn::Home,
            Btn::Back,
            Btn::Select,
            Btn::Start,
            Btn::L1,
            Btn::R1,
        ] {
            let rc = if matches!(b, Btn::L1 | Btn::R1) {
                shoulder(b)
            } else {
                pill(b)
            };
            assert!(inside(rc), "{b:?}");
        }
        // nada se sobrepoe: HOME x L1, BACK x R1, dpad x analogico
        let ov =
            |a: R, b: R| a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
        assert!(!ov(pill(Btn::Home), shoulder(Btn::L1)));
        assert!(!ov(pill(Btn::Back), shoulder(Btn::R1)));
        assert!(
            DPAD.1 - 40.0 > LSTICK.1 + TICK_R + 24.0,
            "leitura do analogico encosta no direcional"
        );
        assert!(RSTICK.1 + TICK_R + 24.0 < pill(Btn::Start).y);
    }

    #[test]
    fn back_e_home_sao_botoes_separados_do_select_e_start() {
        let cfg = Config::default();
        for lit in [Btn::Home, Btn::Back, Btn::Select, Btn::Start] {
            let mut s = st();
            press(&mut s, lit, true);
            let r = shot(&s, 0.0, &cfg);
            for b in [Btn::Home, Btn::Back, Btn::Select, Btn::Start] {
                let p = pill(b);
                let px = at(&r, p.x + 3.5, p.y + p.h / 2.0);
                let want = if b == lit { ACCENT } else { KEY_FILL };
                assert!(near(px, want, 3), "{lit:?} aceso, {b:?}: {px:?}");
            }
        }
    }

    #[test]
    fn botao_muda_de_cor_ocioso_pressionado_testado() {
        let cfg = Config::default();
        let (ax, ay) = face_pos(Btn::A, cfg.layout);
        let mut s = st();
        let r = shot(&s, 0.0, &cfg);
        assert!(
            near(at(&r, ax, ay + 7.0), KEY_FILL, 3),
            "{:?}",
            at(&r, ax, ay + 7.0)
        );
        press(&mut s, Btn::A, true);
        let r = shot(&s, 0.0, &cfg);
        assert!(near(at(&r, ax, ay + 7.0), ACCENT, 3));
        press(&mut s, Btn::A, false);
        let r = shot(&s, 0.0, &cfg);
        assert!(near(at(&r, ax, ay + 7.0), TESTED_FILL, 3));
        let (bx, by) = face_pos(Btn::B, cfg.layout);
        assert!(near(at(&r, bx, by + 7.0), KEY_FILL, 3));
    }

    #[test]
    fn layout_nintendo_e_xbox_trocam_as_posicoes() {
        let n = face_pos(Btn::A, Layout::Nintendo);
        let x = face_pos(Btn::A, Layout::Xbox);
        assert!(n.0 > ABXY.0 && (n.1 - ABXY.1).abs() < 0.01); // A a direita
        assert!(x.1 > ABXY.1 && (x.0 - ABXY.0).abs() < 0.01); // A embaixo
        assert!(face_pos(Btn::X, Layout::Nintendo).1 < ABXY.1); // X em cima, como no aparelho
        assert!(face_pos(Btn::Y, Layout::Nintendo).0 < ABXY.0); // Y a esquerda
        assert!(face_pos(Btn::Y, Layout::Xbox).1 < ABXY.1);
        assert_eq!(parse_layout("XBOX").unwrap(), Layout::Xbox);
        assert!(parse_layout("sega").is_err());
        for l in [Layout::Nintendo, Layout::Xbox] {
            let mut p: Vec<(i32, i32)> = [Btn::A, Btn::B, Btn::X, Btn::Y]
                .iter()
                .map(|b| {
                    let q = face_pos(*b, l);
                    (q.0 as i32, q.1 as i32)
                })
                .collect();
            p.sort();
            p.dedup();
            assert_eq!(p.len(), 4);
        }
    }

    #[test]
    fn direcional_acende_so_o_lado_pressionado() {
        let cfg = Config::default();
        let mut s = st();
        press(&mut s, Btn::Up, true);
        let r = shot(&s, 0.0, &cfg);
        let up = dpad_arm(Btn::Up);
        let dn = dpad_arm(Btn::Down);
        assert!(near(at(&r, up.x + 4.0, up.y + 5.0), ACCENT, 3));
        assert!(near(at(&r, dn.x + 4.0, dn.y + 5.0), KEY_FILL, 3));
    }

    #[test]
    fn gatilho_enche_proporcionalmente() {
        let cfg = Config::default();
        let mut s = st();
        s.apply(
            0.0,
            Input::Axis {
                axis: Axis::L2,
                value: 16384,
                raw: None,
            },
        );
        let r = shot(&s, 0.0, &cfg);
        let t = trigger(false);
        let y = t.y + t.h - 2.5;
        assert!(
            near(at(&r, t.x + t.w * 0.2, y), ACCENT, 3),
            "{:?}",
            at(&r, t.x + t.w * 0.2, y)
        );
        assert!(
            near(at(&r, t.x + t.w * 0.8, y), TRACK, 3),
            "{:?}",
            at(&r, t.x + t.w * 0.8, y)
        );
        let t2 = trigger(true);
        assert!(near(at(&r, t2.x + t2.w * 0.2, y), TRACK, 3));
    }

    #[test]
    fn analogico_desloca_o_botao_e_marca_as_direcoes() {
        let cfg = Config::default();
        let mut s = st();
        let r0 = shot(&s, 0.0, &cfg);
        assert!(
            near(at(&r0, LSTICK.0, LSTICK.1 + 5.0), DOT, 4),
            "{:?}",
            at(&r0, LSTICK.0, LSTICK.1 + 5.0)
        );
        s.apply(
            0.0,
            Input::Axis {
                axis: Axis::LX,
                value: 32767,
                raw: None,
            },
        );
        let r1 = shot(&s, 0.0, &cfg);
        assert!(near(at(&r1, LSTICK.0 + TRAVEL, LSTICK.1 + 5.0), DOT, 4));
        assert!(near(at(&r1, LSTICK.0 + TICK_R, LSTICK.1), OK, 4));
        assert!(near(at(&r1, LSTICK.0 - TICK_R, LSTICK.1), KEY_EDGE, 4));
        assert!(near(at(&r1, RSTICK.0, RSTICK.1 + 5.0), DOT, 4));
    }

    #[test]
    fn contador_e_barra_de_saida() {
        let cfg = Config::default();
        let mut s = st();
        press(&mut s, Btn::Start, true);
        press(&mut s, Btn::Home, true);
        s.tick(0.0);
        let r = shot(&s, EXIT_HOLD / 2.0, &cfg);
        let y = EXIT_CARD.y + EXIT_CARD.h - 8.0;
        let x0 = EXIT_CARD.x + 14.0;
        let w = EXIT_CARD.w - 28.0;
        assert!(
            near(at(&r, x0 + w * 0.25, y), OK, 3),
            "{:?}",
            at(&r, x0 + w * 0.25, y)
        );
        assert!(near(at(&r, x0 + w * 0.75, y), TRACK, 3));
        let r = shot(&st(), 0.0, &cfg);
        assert!(near(at(&r, x0 + w * 0.25, y), TRACK, 3));
    }

    #[test]
    fn mensagens_de_estado_vazio() {
        let cfg = Config::default();
        let none = shot(&st(), 0.0, &cfg);
        let mut with = st();
        with.devices.push(("Pad".into(), "x".into()));
        let some = shot(&with, 0.0, &cfg);
        let find = |r: &Renderer, c: Color| {
            (300..660).any(|x| (235..275).any(|y| near(r.frame().get(x, y).unwrap(), c, 25)))
        };
        assert!(find(&none, BAD));
        assert!(!find(&some, BAD));
    }

    #[test]
    fn log_aparece_na_tela_e_nao_vaza_da_moldura() {
        let cfg = Config::default();
        let mut s = st();
        s.apply(
            1.0,
            Input::Button {
                btn: Btn::B,
                down: true,
                raw: Some("b0".into()),
            },
        );
        let r = shot(&s, 0.0, &cfg);
        let hit = (LOG_X as usize..(LOG_X + LOG_W) as usize).any(|x| {
            (LOG_Y as usize..(LOG_Y + LOG_ROWS as f32 * LOG_PITCH) as usize)
                .any(|y| near(r.frame().get(x, y).unwrap(), ACCENT, 20))
        });
        assert!(hit, "nenhum texto de log encontrado");
        for i in 0..100 {
            s.apply(
                i as f64,
                Input::Button {
                    btn: Btn::B,
                    down: i % 2 == 0,
                    raw: Some("b0".into()),
                },
            );
        }
        let r = shot(&s, 0.0, &cfg);
        // a faixa entre o fim do log e a borda da tela continua escura
        assert!(near(at(&r, 480.0, SCREEN.y + SCREEN.h - 2.0), DARK, 6));
        assert!(LOG_Y + LOG_ROWS as f32 * LOG_PITCH <= SCREEN.y + SCREEN.h);
        let strength = |row: usize| -> u32 {
            let y0 = (LOG_Y + row as f32 * LOG_PITCH) as usize;
            (LOG_X as usize..(LOG_X + LOG_W) as usize)
                .map(|x| {
                    (y0..y0 + 18)
                        .map(|y| r.frame().get(x, y).unwrap()[0] as u32)
                        .sum::<u32>()
                })
                .sum()
        };
        assert!(strength(LOG_ROWS - 1) > strength(0));
    }

    #[test]
    fn escala_letterbox_e_tamanhos_extremos() {
        let cfg = Config::default();
        let s = State::demo();
        let mut r = Renderer::new(1280, 720);
        r.draw(&s, 0.0, &cfg);
        assert!((r.scale() - 4.0 / 3.0).abs() < 1e-4);
        let mut r = Renderer::new(1024, 768);
        r.draw(&s, 0.0, &cfg);
        assert!(near(r.frame().get(1, 1).unwrap(), BG, 1));
        assert!(near(r.frame().get(1022, 766).unwrap(), BG, 1));
        let mut tiny = Renderer::new(1, 1);
        tiny.draw(&s, 0.0, &cfg);
        r.resize(640, 360);
        r.draw(&s, 0.0, &cfg);
    }

    #[test]
    fn destaque_e_combinacao_customizada_refazem_a_parte_fixa() {
        let mut r = Renderer::new(960, 540);
        let teal = parse_accent("teal").unwrap();
        let mut s = st();
        press(&mut s, Btn::B, true);
        r.draw(
            &s,
            0.0,
            &Config {
                accent: teal,
                layout: Layout::Nintendo,
            },
        );
        let (bx, by) = face_pos(Btn::B, Layout::Nintendo);
        assert!(near(at(&r, bx, by + 7.0), teal, 3));
        let s2 = State::new(crate::pad::parse_exit("select+b12").unwrap());
        let before = r.frame().data().to_vec();
        r.draw(
            &s2,
            0.0,
            &Config {
                accent: teal,
                layout: Layout::Nintendo,
            },
        );
        assert_ne!(before, r.frame().data().to_vec());
        assert!(parse_accent("roxo").is_err());
        assert_eq!(parse_accent("#a78bfa").unwrap(), Color::hex(0xA78BFA));
    }

    #[test]
    fn captura_para_inspecao() {
        if let Ok(p) = std::env::var("RG52_PAD_DUMP") {
            let mut r = Renderer::new(1280, 720);
            r.draw(&State::demo(), 0.0, &Config::default());
            r.frame().write_ppm(std::path::Path::new(&p)).unwrap();
        }
    }
}
