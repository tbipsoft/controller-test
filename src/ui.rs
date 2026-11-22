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

use crate::devices::{Profile, GENERIC};
use crate::gfx::{Color, Face, Fonts, Pixmap};
use crate::pad::{Btn, ExitKey, Kind, Line, State, EXIT_HOLD, SECTORS};

pub const DESIGN_W: f32 = 960.0;
pub const DESIGN_H: f32 = 540.0;
/// Largura de design das telas 4:3 e 3:2 (a altura sai da proporcao).
const DESIGN_COMPACT_W: f32 = 640.0;

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

#[cfg(test)]
fn dpad_arm(b: Btn) -> R {
    dpad_arm_at(DPAD, 14.0, 30.0, b)
}

fn dpad_arm_at(c: (f32, f32), h: f32, l: f32, b: Btn) -> R {
    let (cx, cy) = c;
    match b {
        Btn::Up => r(cx - h, cy - h - l, 2.0 * h, l),
        Btn::Down => r(cx - h, cy + h, 2.0 * h, l),
        Btn::Left => r(cx - h - l, cy - h, l, 2.0 * h),
        _ => r(cx + h, cy - h, l, 2.0 * h),
    }
}

#[cfg(test)]
fn face_pos(b: Btn, layout: Layout) -> (f32, f32) {
    face_at(ABXY, 36.0, b, layout)
}

fn face_at(c: (f32, f32), d: f32, b: Btn, layout: Layout) -> (f32, f32) {
    let (dx, dy) = match (layout, b) {
        (Layout::Nintendo, Btn::X) | (Layout::Xbox, Btn::Y) => (0.0, -d),
        (Layout::Nintendo, Btn::B) | (Layout::Xbox, Btn::A) => (0.0, d),
        (Layout::Nintendo, Btn::Y) | (Layout::Xbox, Btn::X) => (-d, 0.0),
        _ => (d, 0.0),
    };
    (c.0 + dx, c.1 + dy)
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

/// Geometria da tela atual, em coordenadas de design. Larga (16:9) ou compacta (4:3, 3:2).
#[derive(Clone, Copy)]
struct Geo {
    w: f32,
    h: f32,
    compact: bool,
    body: R,
    body_rad: f32,
    screen: R,
    lstick: (f32, f32),
    rstick: (f32, f32),
    stick_r: f32,
    travel: f32,
    tick_r: f32,
    knob_r: f32,
    dpad: (f32, f32),
    dpad_h: f32,
    dpad_l: f32,
    abxy: (f32, f32),
    face_d: f32,
    face_r: f32,
    shoulders: [R; 2],
    triggers: [R; 2],
    pills: [Option<(Btn, R)>; 4],
    log_x: f32,
    log_w: f32,
    log_y: f32,
    log_pitch: f32,
    log_rows: usize,
    col_name: f32,
    col_act: f32,
    col_src: f32,
    exit_card: R,
    info_card: R,
    hdr_cy: f32,
    right: f32,
}

impl Geo {
    fn new(w: usize, h: usize, p: &Profile) -> Geo {
        let aspect = w as f32 / h as f32;
        if aspect >= 1.6 {
            Geo::wide(p)
        } else {
            Geo::narrow(DESIGN_COMPACT_W / aspect, p)
        }
    }

    /// Botao que ocupa a vaga do HOME: o proprio HOME ou o alias do aparelho (ex.: F).
    fn home_slot(p: &Profile) -> Option<Btn> {
        p.alias
            .map(|(b, _)| b)
            .or(p.buttons.contains(&Btn::Home).then_some(Btn::Home))
    }

    fn wide(p: &Profile) -> Geo {
        let has = |b| p.buttons.contains(&b);
        let (lstick, dpad) = if p.stick_on_top {
            (LSTICK, DPAD)
        } else {
            ((LSTICK.0, DPAD.1), (DPAD.0, LSTICK.1))
        };
        // sem analogico direito, o ABXY fica centrado na coluna
        let abxy = if p.sticks[1] {
            ABXY
        } else {
            (ABXY.0, (ABXY.1 + RSTICK.1) / 2.0)
        };
        let pills = [
            Geo::home_slot(p).map(|b| (b, pill(Btn::Home))),
            has(Btn::Back).then(|| (Btn::Back, pill(Btn::Back))),
            has(Btn::Select).then(|| (Btn::Select, pill(Btn::Select))),
            has(Btn::Start).then(|| (Btn::Start, pill(Btn::Start))),
        ];
        Geo {
            w: DESIGN_W,
            h: DESIGN_H,
            compact: false,
            body: BODY,
            body_rad: BODY_RAD,
            screen: SCREEN,
            lstick,
            rstick: RSTICK,
            stick_r: STICK_R,
            travel: TRAVEL,
            tick_r: TICK_R,
            knob_r: 15.0,
            dpad,
            dpad_h: 14.0,
            dpad_l: 30.0,
            abxy,
            face_d: 36.0,
            face_r: 16.0,
            shoulders: [shoulder(Btn::L1), shoulder(Btn::R1)],
            triggers: [trigger(false), trigger(true)],
            pills,
            log_x: LOG_X,
            log_w: LOG_W,
            log_y: LOG_Y,
            log_pitch: LOG_PITCH,
            log_rows: LOG_ROWS,
            col_name: COL_NAME,
            col_act: COL_ACT,
            col_src: COL_SRC,
            exit_card: EXIT_CARD,
            info_card: INFO_CARD,
            hdr_cy: 26.0,
            right: 930.0,
        }
    }

    /// Telas 4:3 e 3:2: colunas estreitas e o log ocupando o centro.
    fn narrow(h: f32, p: &Profile) -> Geo {
        let w = DESIGN_COMPACT_W;
        let has = |b| p.buttons.contains(&b);
        let body = r(6.0, 34.0, w - 12.0, h - 76.0);
        let screen = r(176.0, body.y + 6.0, 288.0, body.h - 12.0);
        let (cl, cr) = (92.0, w - 92.0);
        let pill_y = body.y + body.h - 30.0;
        // area livre entre os ombros e as pilulas
        let (y0, y1) = (body.y + 46.0, pill_y - 6.0);
        let upper = y0 + (y1 - y0) * 0.27;
        let lower = y0 + (y1 - y0) * 0.75;
        let (lstick, dpad) = if p.stick_on_top {
            ((cl, upper), (cl, lower))
        } else {
            ((cl, lower), (cl, upper))
        };
        let abxy = if p.sticks[1] {
            (cr, upper)
        } else {
            (cr, (upper + lower) / 2.0)
        };
        // uma ou duas pilulas por lado, centradas na coluna
        let row = |c: f32, a: Option<Btn>, b: Option<Btn>| -> [Option<(Btn, R)>; 2] {
            let (pw, ph, gap) = (70.0, 24.0, 6.0);
            match (a, b) {
                (Some(a), Some(b)) => [
                    Some((a, r(c - pw - gap / 2.0, pill_y, pw, ph))),
                    Some((b, r(c + gap / 2.0, pill_y, pw, ph))),
                ],
                (Some(x), None) | (None, Some(x)) => {
                    [Some((x, r(c - pw / 2.0, pill_y, pw, ph))), None]
                }
                _ => [None, None],
            }
        };
        let left = row(
            cl,
            has(Btn::Select).then_some(Btn::Select),
            Geo::home_slot(p),
        );
        let right = row(
            cr,
            has(Btn::Back).then_some(Btn::Back),
            has(Btn::Start).then_some(Btn::Start),
        );
        let log_pitch = 24.0;
        let log_rows = (((screen.h - 16.0) / log_pitch).floor() as usize).max(1);
        Geo {
            w,
            h,
            compact: true,
            body,
            body_rad: 28.0,
            screen,
            lstick,
            rstick: (cr, lower),
            stick_r: 36.0,
            travel: 24.0,
            tick_r: 43.0,
            knob_r: 12.5,
            dpad,
            dpad_h: 12.0,
            dpad_l: 24.0,
            abxy,
            face_d: 30.0,
            face_r: 14.0,
            shoulders: [
                r(cl - 58.0, body.y + 22.0, 116.0, 20.0),
                r(cr - 58.0, body.y + 22.0, 116.0, 20.0),
            ],
            triggers: [
                r(cl - 58.0, body.y + 6.0, 116.0, 13.0),
                r(cr - 58.0, body.y + 6.0, 116.0, 13.0),
            ],
            pills: [left[0], left[1], right[0], right[1]],
            log_x: screen.x + 10.0,
            log_w: screen.w - 20.0,
            log_y: screen.y + 8.0,
            log_pitch,
            log_rows,
            col_name: 44.0,
            col_act: 130.0,
            col_src: 204.0,
            exit_card: r(6.0, h - 38.0, 300.0, 34.0),
            info_card: r(312.0, h - 38.0, w - 318.0, 34.0),
            hdr_cy: 17.0,
            right: w - 8.0,
        }
    }

    fn arm(&self, b: Btn) -> R {
        dpad_arm_at(self.dpad, self.dpad_h, self.dpad_l, b)
    }

    fn face(&self, b: Btn, layout: Layout) -> (f32, f32) {
        face_at(self.abxy, self.face_d, b, layout)
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
    profile: &'static Profile,
    geo: Geo,
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
    /// Sem aparelho definido: mostra todos os controles.
    pub fn new(w: usize, h: usize) -> Renderer {
        Renderer::with_profile(w, h, &GENERIC)
    }

    pub fn with_profile(w: usize, h: usize, profile: &'static Profile) -> Renderer {
        let mut r = Renderer {
            fonts: Fonts::new(),
            profile,
            geo: Geo::wide(profile),
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
        self.geo = Geo::new(w, h, self.profile);
        self.k = (w as f32 / self.geo.w).min(h as f32 / self.geo.h);
        self.ox = (w as f32 - self.geo.w * self.k) / 2.0;
        self.oy = (h as f32 - self.geo.h * self.k) / 2.0;
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
        let g = self.geo;
        let title = self.profile.title;
        let mut p = P {
            pm: &mut self.statics,
            f: &self.fonts,
            k: self.k,
            ox: self.ox,
            oy: self.oy,
        };
        let acc = cfg.accent;
        crate::header::draw(
            p.pm,
            p.f,
            p.k,
            p.ox,
            p.oy,
            &crate::header::Header {
                left: if g.compact { 10.0 } else { 32.0 },
                center: g.w / 2.0,
                cy: g.hdr_cy,
                compact: g.compact,
                device: title,
                app: "CONTROLLER TEST",
            },
        );
        // corpo do aparelho e "tela" (so o log, sem titulo)
        p.rrect(g.body, g.body_rad, CARD);
        p.stroke(g.body, g.body_rad, 1.0, LINE);
        p.rrect(g.screen, 18.0, DARK);
        p.stroke(g.screen, 18.0, 1.0, WELL_EDGE);
        // trilhos dos gatilhos
        if self.profile.triggers {
            for t in g.triggers {
                p.rrect(t, 8.0, TRACK);
            }
        }
        // cartoes do rodape
        for c in [g.exit_card, g.info_card] {
            p.rrect(c, 14.0, CARD);
            p.stroke(c, 14.0, 1.0, LINE);
        }
        // teclas da combinacao de saida
        let (key_font, key_h, pad_y) = if g.compact {
            (11.0, 18.0, 4.0)
        } else {
            (13.0, 26.0, 8.0)
        };
        let mut x = g.exit_card.x + if g.compact { 10.0 } else { 14.0 };
        let y = g.exit_card.y + pad_y;
        let mid = y + key_h / 2.0;
        for (i, key) in combo.iter().enumerate() {
            if i > 0 {
                x += 6.0;
                let w = p.text_mid(Face::Sans700, key_font + 3.0, 0.0, MUTED, x, mid, "+");
                x += w + 6.0;
            }
            let label = key.label();
            let kw = (p.width(Face::Mono700, key_font, 0.0, &label) + 14.0).max(40.0);
            let kr = r(x, y, kw, key_h);
            p.rrect(kr, 6.0, Color::hex(0x1B2130));
            p.stroke(kr, 6.0, 1.0, Color::hex(0x2A3144));
            p.text_c(
                Face::Mono700,
                key_font,
                0.0,
                TEXT,
                x + kw / 2.0,
                mid,
                &label,
            );
            x += kw;
        }
        p.text_mid(
            Face::Sans700,
            key_font,
            0.0,
            MUTED,
            x + 10.0,
            mid,
            &format!("hold {EXIT_HOLD:.0} s to exit"),
        );
        p.text_mid(
            Face::Sans800,
            if g.compact { 9.5 } else { 11.0 },
            1.5,
            acc,
            g.info_card.x + if g.compact { 10.0 } else { 14.0 },
            g.info_card.y + if g.compact { 10.0 } else { 15.0 },
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
        let g = self.geo;
        let profile = self.profile;
        let c = profile.controls();
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
        let total = st.total();
        let done = total > 0 && n >= total;
        let txt = format!("{n}/{total}");
        let (cnt_size, bar_w, lbl_size) = if g.compact {
            (15.0, 56.0, 10.0)
        } else {
            (20.0, 90.0, 12.0)
        };
        let tw = p.width(Face::Mono700, cnt_size, 0.0, &txt);
        let col = if done { OK } else { TEXT };
        p.text_mid(
            Face::Mono700,
            cnt_size,
            0.0,
            col,
            g.right - tw,
            g.hdr_cy,
            &txt,
        );
        let bar = r(g.right - tw - 10.0 - bar_w, g.hdr_cy - 3.0, bar_w, 6.0);
        p.rrect(bar, 3.0, TRACK);
        let fw = if total > 0 {
            bar.w * n as f32 / total as f32
        } else {
            0.0
        };
        if fw > 0.0 {
            p.rrect(
                R {
                    w: fw.max(6.0),
                    ..bar
                },
                3.0,
                if done { OK } else { acc },
            );
        }
        let lw = p.width(Face::Sans800, lbl_size, 1.2, "TESTED");
        p.text_mid(
            Face::Sans800,
            lbl_size,
            1.2,
            MUTED,
            bar.x - lw - 8.0,
            g.hdr_cy,
            "TESTED",
        );

        // --- ombros (L1/R1) e gatilhos analogicos (L2/R2) ---
        let small = if g.compact { 10.0 } else { 12.0 };
        for (i, b) in [Btn::L1, Btn::R1].into_iter().enumerate() {
            if !c.has(b) {
                continue;
            }
            let rc = g.shoulders[i];
            let (f, e, t) = colors(vis(st, b), acc);
            p.rrect(rc, 9.0, f);
            p.stroke(rc, 9.0, 1.5, e);
            p.text_c(
                Face::Mono700,
                if g.compact { 13.0 } else { 15.0 },
                0.0,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                b.label(),
            );
        }
        if c.triggers {
            for (i, name) in ["L2", "R2"].into_iter().enumerate() {
                let rc = g.triggers[i];
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
                let tc = if v > 0.5 {
                    DARK
                } else if st.trig_tested[i] {
                    OK
                } else {
                    MUTED
                };
                p.text_mid(
                    Face::Mono700,
                    small,
                    0.0,
                    tc,
                    rc.x + 8.0,
                    rc.y + rc.h / 2.0,
                    name,
                );
                let pct = format!("{:.0}%", v * 100.0);
                let w = p.width(Face::Mono700, small, 0.0, &pct);
                let c2 = if v > 0.92 { DARK } else { MUTED };
                p.text_mid(
                    Face::Mono700,
                    small,
                    0.0,
                    c2,
                    rc.x + rc.w - 8.0 - w,
                    rc.y + rc.h / 2.0,
                    &pct,
                );
            }
        }

        // --- direcional ---
        let (f, e, _) = colors(Vis::Idle, acc);
        let hub_half = g.dpad_h - 1.0;
        let hub = r(
            g.dpad.0 - hub_half,
            g.dpad.1 - hub_half,
            hub_half * 2.0,
            hub_half * 2.0,
        );
        p.rrect(hub, 4.0, f);
        p.stroke(hub, 4.0, 1.5, e);
        for (b, glyph) in [
            (Btn::Up, "^"),
            (Btn::Down, "v"),
            (Btn::Left, "<"),
            (Btn::Right, ">"),
        ] {
            let rc = g.arm(b);
            let (f, e, t) = colors(vis(st, b), acc);
            p.rrect(rc, 7.0, f);
            p.stroke(rc, 7.0, 1.5, e);
            p.text_c(
                Face::Mono700,
                if g.compact { 14.0 } else { 16.0 },
                0.0,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                glyph,
            );
        }

        // --- botoes de face ---
        for b in [Btn::A, Btn::B, Btn::X, Btn::Y] {
            let (cx, cy) = g.face(b, cfg.layout);
            let (f, e, t) = colors(vis(st, b), acc);
            p.circle(cx, cy, g.face_r, f);
            p.ring(cx, cy, g.face_r - 0.75, 1.5, e);
            p.text_c(
                Face::Sans800,
                if g.compact { 15.0 } else { 17.0 },
                0.0,
                t,
                cx,
                cy,
                b.label(),
            );
        }

        // --- analogicos ---
        if c.sticks[0] {
            stick(&mut p, &g, g.lstick, 0, st, Btn::L3, acc);
        }
        if c.sticks[1] {
            stick(&mut p, &g, g.rstick, 1, st, Btn::R3, acc);
        }

        // --- botoes pequenos (HOME/F, BACK, SELECT, START) ---
        for (b, rc) in g.pills.into_iter().flatten() {
            let (f, e, t) = colors(vis(st, b), acc);
            p.rrect(rc, 10.0, f);
            p.stroke(rc, 10.0, 1.5, e);
            p.text_c(
                Face::Sans800,
                if g.compact { 10.5 } else { 12.0 },
                0.6,
                t,
                rc.x + rc.w / 2.0,
                rc.y + rc.h / 2.0,
                c.label(b),
            );
        }

        // --- mensagem quando ainda nao ha eventos ---
        if st.log.is_empty() {
            let cx = g.screen.x + g.screen.w / 2.0;
            let cy = g.screen.y + g.screen.h / 2.0;
            let (big, sub) = if g.compact {
                (17.0, 10.5)
            } else {
                (22.0, 15.0)
            };
            if st.devices.is_empty() {
                p.text_c(
                    Face::Sans800,
                    big,
                    0.0,
                    BAD,
                    cx,
                    cy - 12.0,
                    "No controller detected",
                );
                p.text_c(
                    Face::Sans600,
                    sub,
                    0.0,
                    MUTED,
                    cx,
                    cy + 12.0,
                    "Connect one or check the SDL controller mapping",
                );
            } else {
                p.text_c(
                    Face::Sans800,
                    big,
                    0.0,
                    TEXT,
                    cx,
                    cy - 12.0,
                    "Press any button",
                );
                let hint = if c.triggers {
                    "Move the sticks and squeeze the triggers"
                } else {
                    "Move the sticks"
                };
                p.text_c(Face::Sans600, sub, 0.0, MUTED, cx, cy + 12.0, hint);
            }
        }

        // --- cartao de saida: so a barra de progresso ---
        let prog = st.exit_progress(now);
        if st.holding_exit() {
            p.stroke(g.exit_card, 14.0, 2.0, OK);
        } else {
            p.stroke(g.exit_card, 14.0, 1.5, acc);
        }
        let (pb_in, pb_up, pb_h) = if g.compact {
            (10.0, 6.0, 3.0)
        } else {
            (14.0, 10.0, 4.0)
        };
        let pb = r(
            g.exit_card.x + pb_in,
            g.exit_card.y + g.exit_card.h - pb_up,
            g.exit_card.w - 2.0 * pb_in,
            pb_h,
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
        let card = g.info_card;
        let ix = card.x + if g.compact { 10.0 } else { 14.0 };
        let iw = card.w - if g.compact { 20.0 } else { 28.0 };
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
        let (name_y, name_size, vol_y, vol_h, vol_w, vol_step, vol_font, tail_size) = if g.compact {
            (10.0, 11.0, 19.0, 12.0, 38.0, 42.0, 8.5, 9.5)
        } else {
            (15.0, 14.0, 27.0, 16.0, 46.0, 52.0, 11.0, 12.0)
        };
        p.text_right(
            Face::Sans700,
            name_size,
            TEXT,
            ix + iw,
            card.y + name_y,
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
            let rc = r(ix + i as f32 * vol_step, card.y + vol_y, vol_w, vol_h);
            let v = if down {
                Vis::Pressed
            } else if seen {
                Vis::Tested
            } else {
                Vis::Idle
            };
            let (f, e, t) = colors(v, acc);
            p.rrect(rc, 6.0, f);
            p.stroke(rc, 6.0, 1.5, e);
            p.text_c(
                Face::Sans800,
                vol_font,
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
            tail_size,
            MUTED,
            ix + iw,
            card.y + vol_y + vol_h / 2.0,
            &tail,
            iw - 2.0 * vol_step - 8.0,
        );
        &self.frame
    }

    fn make_strip(&self, l: &Line) -> Pixmap {
        let g = &self.geo;
        let (w, h) = (
            (g.log_w * self.k).ceil() as usize + 1,
            (g.log_pitch * self.k).ceil() as usize,
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
        let cy = g.log_pitch / 2.0;
        let (act_c, name_c) = match l.kind {
            Kind::Down => (ACCENT, TEXT),
            Kind::Up => (MUTED, MUTED),
            Kind::Axis => (AXIS_C, TEXT),
            Kind::Raw => (RAW_C, RAW_C),
            Kind::Key => (KEY_C, TEXT),
            Kind::Sys => (OK, OK),
        };
        let (big, small) = if g.compact {
            (13.0, 11.0)
        } else {
            (16.0, 13.0)
        };
        let fit = |face, size: f32, s: &str, max: f32| {
            self.fonts.fit(face, size * self.k, 0.0, s, max * self.k)
        };
        p.text_mid(
            Face::Mono500,
            small,
            0.0,
            MUTED,
            0.0,
            cy,
            &format!("{:6.1}", l.t),
        );
        let name = fit(Face::Mono700, big, &l.name, g.col_act - g.col_name - 6.0);
        p.text_mid(Face::Mono700, big, 0.0, name_c, g.col_name, cy, &name);
        let act = fit(Face::Mono700, big, &l.act, g.col_src - g.col_act - 6.0);
        p.text_mid(Face::Mono700, big, 0.0, act_c, g.col_act, cy, &act);
        let det = fit(Face::Mono500, small, &l.detail, g.log_w - g.col_src);
        p.text_mid(Face::Mono500, small, 0.0, MUTED, g.col_src, cy, &det);
        pm
    }

    fn draw_log(&mut self, st: &State) {
        let g = self.geo;
        let rows: Vec<&Line> = st
            .log
            .iter()
            .rev()
            .take(g.log_rows)
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
            let x = (self.ox + g.log_x * self.k).round() as i32;
            let y = (self.oy + (g.log_y + (g.log_rows - n + i) as f32 * g.log_pitch) * self.k)
                .round() as i32;
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

fn stick(p: &mut P, g: &Geo, c: (f32, f32), idx: usize, st: &State, click: Btn, acc: Color) {
    let (cx, cy) = c;
    let (travel, tick) = (g.travel, g.tick_r);
    p.circle(cx, cy, g.stick_r, WELL_EDGE);
    p.circle(cx, cy, g.stick_r - 1.0, DARK);
    p.ring(cx, cy, travel, 1.0, LINE);
    // bordas alcancadas (mostra se o curso e redondo e completo)
    for s in 0..SECTORS {
        let r = st.reach[idx][s];
        if r >= 0.05 {
            let a =
                (s as f32 + 0.5) / SECTORS as f32 * std::f32::consts::TAU - std::f32::consts::PI;
            p.circle(
                cx + a.cos() * r * travel,
                cy + a.sin() * r * travel,
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
            cx + dx * tick,
            cy + dy * tick,
            3.2,
            if on { OK } else { KEY_EDGE },
        );
    }
    // o botao do analogico (clique = L3/R3)
    let (x, y) = st.stick[idx];
    let len = (x * x + y * y).sqrt();
    let s = if len > 1.0 { 1.0 / len } else { 1.0 };
    let (px, py) = (cx + x * s * travel, cy + y * s * travel);
    let (f, e, t) = match vis(st, click) {
        Vis::Idle => (DOT, KEY_EDGE, MUTED),
        other => colors(other, acc),
    };
    p.circle(px, py, g.knob_r, f);
    p.ring(px, py, g.knob_r - 0.75, 1.5, e);
    let font = if g.compact { 10.5 } else { 12.0 };
    p.text_c(Face::Mono700, font, 0.0, t, px, py, click.label());
    let txt = format!("X {x:+.2}  Y {y:+.2}");
    p.text_c(
        Face::Mono500,
        if g.compact { 11.0 } else { 13.0 },
        0.0,
        MUTED,
        cx,
        cy + tick + if g.compact { 13.0 } else { 16.0 },
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
    fn cada_aparelho_mostra_so_seus_controles_em_cada_tela() {
        use crate::devices::{find, ALL};
        let inside = |o: R, i: R| {
            i.x >= o.x && i.y >= o.y && i.x + i.w <= o.x + o.w && i.y + i.h <= o.y + o.h
        };
        let overlap =
            |a: R, b: R| a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
        for p in ALL {
            for (w, h) in [(640, 480), (480, 320), (720, 720), (960, 544), (1280, 720)] {
                let g = Geo::new(w, h, p);
                let pills: Vec<(Btn, R)> = g.pills.into_iter().flatten().collect();
                for (b, rc) in &pills {
                    assert!(p.buttons.contains(b), "{} {b:?}", p.id);
                    assert!(inside(g.body, *rc), "{} {w}x{h} {b:?}", p.id);
                    assert!(!overlap(*rc, g.screen), "{} {w}x{h} {b:?}", p.id);
                }
                for (i, (_, a)) in pills.iter().enumerate() {
                    for (_, b) in &pills[i + 1..] {
                        assert!(!overlap(*a, *b), "{} {w}x{h}", p.id);
                    }
                }
                // leitura do analogico nao encosta no direcional nem nas pilulas
                let bottom = g.lstick.1 + g.tick_r + 22.0;
                let dpad_top = g.dpad.1 - g.dpad_h - g.dpad_l;
                let dpad_bot = g.dpad.1 + g.dpad_h + g.dpad_l;
                assert!(
                    bottom <= dpad_top || dpad_bot <= g.lstick.1 - g.tick_r,
                    "{} {w}x{h}",
                    p.id
                );
                let select = pills.iter().find(|x| x.0 == Btn::Select).unwrap().1;
                assert!(bottom <= select.y + 8.0, "{} {w}x{h}", p.id);
                assert!(g.log_y + g.log_rows as f32 * g.log_pitch <= g.screen.y + g.screen.h);
                assert!(inside(r(0.0, 0.0, g.w, g.h), g.exit_card));
                assert!(inside(r(0.0, 0.0, g.w, g.h), g.info_card));
                Renderer::with_profile(w, h, p).draw(&State::demo(), 0.0, &Config::default());
            }
        }
        // RGB10: sem analogico direito; o b16 vira a pilula "F" e o HOME nao aparece
        let rgb10 = find("rgb10").unwrap();
        let g = Geo::new(480, 320, rgb10);
        assert!(g.pills.iter().flatten().any(|(b, _)| *b == Btn::R3));
        assert!(!g.pills.iter().flatten().any(|(b, _)| *b == Btn::Home));
        let mut r = Renderer::with_profile(480, 320, rgb10);
        let s = State::with_controls(vec![], rgb10.controls());
        r.draw(&s, 0.0, &Config::default());
        let (x, y) = (
            (g.rstick.0 * r.scale()) as usize,
            (g.rstick.1 * r.scale()) as usize,
        );
        assert!(
            near(r.frame().get(x, y).unwrap(), CARD, 3),
            "analogico direito desenhado"
        );
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
