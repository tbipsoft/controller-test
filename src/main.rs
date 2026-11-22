//! rg52-pad ("Controller Test"): mostra em tempo real tudo o que o controle envia.
//!
//! Cada controle e aberto de dois jeitos: como controle com nome (A, B, START, HOME...
//! pelo mapeamento do SDL) e como joystick cru. Eventos nomeados desenham o controle;
//! o que o mapeamento nao cobre (ex.: um botao sem nome) aparece no log como `bN`/`aN`/
//! `hN`, para voce ver que o hardware envia algo mesmo sem mapeamento.
//!
//! Saida: segurar a combinacao (padrao START + HOME) por 1 s. Nao ha outro botao que saia.

use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    process,
    time::{Duration, Instant},
};

use rg52pad::{
    devices::{self, Detected},
    pad::{
        key_as_button, parse_exit, parse_map, parse_mapping, Axis, Binds, Btn, ExitKey, Input,
        State,
    },
    ui::{parse_accent, parse_layout, Config, Layout, Renderer, ACCENT},
};
use sdl2::{
    controller::{Axis as SAxis, Button as SBtn, GameController},
    event::{Event, WindowEvent},
    joystick::{HatState, Joystick},
    pixels::PixelFormatEnum,
};

const HELP: &str = "\
rg52-pad - Controller Test: mostra na tela o que cada botao, gatilho e analogico envia

USO: rg52-pad [opcoes]
  --fullscreen      tela cheia (use no aparelho)
  --device ID       auto (padrao), rg52mini, rg43, r36s, rgb10, a10mini, rk3566 ou generic
  --exit COMBO      combinacao para sair, segurada por 1 s (padrao: a do aparelho).
                    Aceita nomes (start, select, home, l1...) e botoes crus (b12)
  --layout L        posicao dos botoes de face: nintendo (padrao) ou xbox
  --accent COR      amber (padrao), teal, violet, coral ou #RRGGBB
  --map LISTA       atribui botoes crus ao desenho, ex.: back=b13,home=b12
                    (os botoes sem nome no SDL aparecem no log como bN)
  --list            lista os controles detectados (nome, GUID, mapeamento) e sai
  --screenshot PPM  (depuracao) desenha um quadro de exemplo sem janela e sai
  --size LxA        tamanho do --screenshot (padrao 1280x720)
  -h, --help        esta ajuda
";

struct Args {
    fullscreen: bool,
    device: Option<String>,
    exit: Option<Vec<ExitKey>>,
    cfg: Config,
    list: bool,
    map: Vec<(u8, Btn)>,
    screenshot: Option<PathBuf>,
    size: (usize, usize),
}

fn parse_size(s: &str) -> Result<(usize, usize), String> {
    s.split_once('x')
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .filter(|&(w, h): &(usize, usize)| w > 0 && h > 0)
        .ok_or_else(|| format!("--size invalido: {s:?} (use LxA, ex.: 640x480)"))
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        fullscreen: false,
        device: None,
        exit: None,
        size: (1280, 720),
        cfg: Config {
            accent: ACCENT,
            layout: Layout::Nintendo,
        },
        list: false,
        screenshot: None,
        map: Vec::new(),
    };
    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = |n: &str| it.next().ok_or_else(|| format!("{n} precisa de um valor"));
        match arg.as_str() {
            "--fullscreen" => a.fullscreen = true,
            "--device" => a.device = Some(val("--device")?),
            "--size" => a.size = parse_size(&val("--size")?)?,
            "--exit" => a.exit = Some(parse_exit(&val("--exit")?)?),
            "--layout" => a.cfg.layout = parse_layout(&val("--layout")?)?,
            "--accent" => a.cfg.accent = parse_accent(&val("--accent")?)?,
            "--list" => a.list = true,
            "--map" => a.map = parse_map(&val("--map")?)?,
            "--screenshot" => a.screenshot = Some(PathBuf::from(val("--screenshot")?)),
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            o => return Err(format!("opcao desconhecida: {o}\n\n{HELP}")),
        }
    }
    Ok(a)
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };
    if let Err(e) = run(args) {
        eprintln!("erro: {e}");
        process::exit(1);
    }
}

struct Dev {
    name: String,
    _joy: Joystick,
    ctl: Option<GameController>,
    binds: Binds,
}

impl Dev {
    fn bound(&self, name: &str) -> Option<String> {
        self.binds.by_name.get(name).cloned()
    }
}

fn btn_of(b: SBtn) -> Option<(Btn, &'static str)> {
    Some(match b {
        SBtn::A => (Btn::A, "a"),
        SBtn::B => (Btn::B, "b"),
        SBtn::X => (Btn::X, "x"),
        SBtn::Y => (Btn::Y, "y"),
        SBtn::Back => (Btn::Select, "back"),
        SBtn::Guide => (Btn::Home, "guide"),
        SBtn::Start => (Btn::Start, "start"),
        SBtn::LeftStick => (Btn::L3, "leftstick"),
        SBtn::RightStick => (Btn::R3, "rightstick"),
        SBtn::LeftShoulder => (Btn::L1, "leftshoulder"),
        SBtn::RightShoulder => (Btn::R1, "rightshoulder"),
        SBtn::DPadUp => (Btn::Up, "dpup"),
        SBtn::DPadDown => (Btn::Down, "dpdown"),
        SBtn::DPadLeft => (Btn::Left, "dpleft"),
        SBtn::DPadRight => (Btn::Right, "dpright"),
        _ => return None,
    })
}

fn axis_of(a: SAxis) -> (Axis, &'static str) {
    match a {
        SAxis::LeftX => (Axis::LX, "leftx"),
        SAxis::LeftY => (Axis::LY, "lefty"),
        SAxis::RightX => (Axis::RX, "rightx"),
        SAxis::RightY => (Axis::RY, "righty"),
        SAxis::TriggerLeft => (Axis::L2, "lefttrigger"),
        SAxis::TriggerRight => (Axis::R2, "righttrigger"),
    }
}

/// Traduz um evento do SDL no que o estado entende. `None` = ignorar.
fn translate(devs: &HashMap<u32, Dev>, user_map: &[(u8, Btn)], ev: &Event) -> Option<Input> {
    let dev = |which: &u32| devs.get(which);
    match ev {
        Event::ControllerButtonDown { which, button, .. }
        | Event::ControllerButtonUp { which, button, .. } => {
            let down = matches!(ev, Event::ControllerButtonDown { .. });
            match btn_of(*button) {
                Some((btn, name)) => Some(Input::Button {
                    btn,
                    down,
                    raw: dev(which).and_then(|d| d.bound(name)),
                }),
                None => Some(Input::Raw {
                    what: format!("{button:?}"),
                    idx: None,
                    down: Some(down),
                    value: None,
                }),
            }
        }
        Event::ControllerAxisMotion {
            which, axis, value, ..
        } => {
            let (a, name) = axis_of(*axis);
            Some(Input::Axis {
                axis: a,
                value: *value,
                raw: dev(which).and_then(|d| d.bound(name)),
            })
        }
        // joystick cru: so o que o mapeamento nao cobre
        Event::JoyButtonDown {
            which, button_idx, ..
        }
        | Event::JoyButtonUp {
            which, button_idx, ..
        } => {
            let down = matches!(ev, Event::JoyButtonDown { .. });
            if dev(which).is_some_and(|d| d.ctl.is_some() && d.binds.buttons.contains(button_idx)) {
                return None;
            }
            // atribuicao manual (--map back=b13): vira o botao do desenho
            if let Some((_, btn)) = user_map.iter().find(|(i, _)| i == button_idx) {
                return Some(Input::Button {
                    btn: *btn,
                    down,
                    raw: Some(format!("b{button_idx}")),
                });
            }
            Some(Input::Raw {
                what: format!("b{button_idx}"),
                idx: Some(*button_idx),
                down: Some(down),
                value: None,
            })
        }
        Event::JoyAxisMotion {
            which,
            axis_idx,
            value,
            ..
        } => {
            if dev(which).is_some_and(|d| d.ctl.is_some() && d.binds.axes.contains(axis_idx)) {
                return None;
            }
            Some(Input::Raw {
                what: format!("a{axis_idx}"),
                idx: None,
                down: None,
                value: Some(*value),
            })
        }
        Event::JoyHatMotion {
            which,
            hat_idx,
            state,
            ..
        } => {
            if dev(which).is_some_and(|d| d.ctl.is_some() && d.binds.hats.contains(hat_idx)) {
                return None;
            }
            let dir = match state {
                HatState::Centered => "center",
                HatState::Up => "up",
                HatState::Right => "right",
                HatState::Down => "down",
                HatState::Left => "left",
                HatState::RightUp => "up-right",
                HatState::RightDown => "down-right",
                HatState::LeftUp => "up-left",
                HatState::LeftDown => "down-left",
            };
            Some(Input::Raw {
                what: format!("h{hat_idx} {dir}"),
                idx: None,
                down: Some(*state != HatState::Centered),
                value: None,
            })
        }
        Event::KeyDown {
            keycode: Some(k),
            repeat: false,
            ..
        } => {
            let name = k.name();
            Some(match key_as_button(&name) {
                Some(btn) => Input::Button {
                    btn,
                    down: true,
                    raw: Some(format!("key {name}")),
                },
                None => Input::Key { name, down: true },
            })
        }
        Event::KeyUp {
            keycode: Some(k), ..
        } => {
            let name = k.name();
            Some(match key_as_button(&name) {
                Some(btn) => Input::Button {
                    btn,
                    down: false,
                    raw: Some(format!("key {name}")),
                },
                None => Input::Key { name, down: false },
            })
        }
        _ => None,
    }
}

fn info_of(joy: &Joystick, mapped: bool) -> String {
    let h = joy.num_hats();
    format!(
        "{} btn, {} axes, {} hat{}, {}",
        joy.num_buttons(),
        joy.num_axes(),
        h,
        if h == 1 { "" } else { "s" },
        if mapped { "mapped" } else { "NOT mapped" }
    )
}

/// Abre o dispositivo `index` (joystick e, se houver mapeamento, controle). `None` se ja
/// estava aberto ou se falhou.
fn open_dev(
    js: &sdl2::JoystickSubsystem,
    gcs: &sdl2::GameControllerSubsystem,
    devs: &mut HashMap<u32, Dev>,
    index: u32,
) -> Option<(String, String)> {
    let joy = js.open(index).ok()?;
    let id = joy.instance_id();
    if devs.contains_key(&id) {
        return None;
    }
    let ctl = if gcs.is_game_controller(index) {
        gcs.open(index).ok()
    } else {
        None
    };
    let binds = ctl
        .as_ref()
        .map(|c| parse_mapping(&c.mapping()))
        .unwrap_or_default();
    let name = joy.name();
    let info = info_of(&joy, ctl.is_some());
    devs.insert(
        id,
        Dev {
            name: name.clone(),
            _joy: joy,
            ctl,
            binds,
        },
    );
    Some((name, info))
}

fn list(sdl: &sdl2::Sdl, detected: &Detected) -> Result<(), String> {
    println!(
        "aparelho: {} [{}], detectado por {}",
        detected.profile.name,
        detected.profile.id,
        detected.source.label()
    );
    let js = sdl.joystick()?;
    let gcs = sdl.game_controller()?;
    let n = js.num_joysticks()?;
    if n == 0 {
        println!("nenhum controle detectado");
    }
    for i in 0..n {
        let joy = js.open(i).map_err(|e| e.to_string())?;
        println!("#{i}  {}", joy.name());
        println!("    guid     {}", joy.guid().string());
        println!("    {}", info_of(&joy, gcs.is_game_controller(i)));
        match gcs.open(i) {
            Ok(c) => println!("    mapping  {}", c.mapping()),
            Err(_) => println!("    mapping  (sem mapeamento no SDL: so eventos crus)"),
        }
    }
    Ok(())
}

fn run(args: Args) -> Result<(), String> {
    let detected = devices::resolve(args.device.as_deref())?;
    let profile = detected.profile;
    let exit = match &args.exit {
        Some(e) => e.clone(),
        None => parse_exit(profile.exit)?,
    };
    // a atribuicao manual (--map) vem primeiro e tem prioridade sobre a do aparelho
    let raw_map: Vec<(u8, Btn)> = args.map.iter().chain(profile.raw_map).copied().collect();

    if let Some(path) = &args.screenshot {
        let (sw, sh) = args.size;
        let mut demo = State::demo();
        demo.controls = profile.controls();
        demo.exit_combo = exit;
        let mut r = Renderer::with_profile(sw, sh, profile);
        r.draw(&demo, 0.0, &args.cfg)
            .write_ppm(path)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    // recebe os botoes mesmo se a janela nao estiver em foco (ex.: ES ainda ativo)
    sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    let sdl = sdl2::init()?;
    if args.list {
        return list(&sdl, &detected);
    }
    let video = sdl.video()?;
    let js = sdl.joystick()?;
    let gcs = sdl.game_controller()?;

    let mut wb = video.window(&format!("{} Controller Test", profile.title), 960, 540);
    wb.position_centered().resizable();
    if args.fullscreen {
        wb.fullscreen_desktop();
    }
    let window = wb.build().map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    if args.fullscreen {
        sdl.mouse().show_cursor(false);
    }
    // fundo na hora; a interface vem no quadro seguinte
    canvas.set_draw_color(sdl2::pixels::Color::RGB(0x0B, 0x0D, 0x12));
    canvas.clear();
    canvas.present();

    let tc = canvas.texture_creator();
    let mut pump = sdl.event_pump()?;
    let (mut w, mut h) = canvas.output_size()?;
    let mut rend = Renderer::with_profile(w as usize, h as usize, profile);
    let mut tex = tc
        .create_texture_streaming(PixelFormatEnum::ARGB8888, w, h)
        .map_err(|e| e.to_string())?;

    let mut state = State::with_controls(exit, profile.controls());
    let mut devs: HashMap<u32, Dev> = HashMap::new();
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64();
    for i in 0..js.num_joysticks().unwrap_or(0) {
        if let Some((name, info)) = open_dev(&js, &gcs, &mut devs, i) {
            state.apply(
                now(),
                Input::Device {
                    name,
                    info,
                    added: true,
                },
            );
        }
    }

    let mut force = true;
    let mut quit = false;
    while !quit {
        for ev in pump.poll_iter() {
            match &ev {
                Event::Quit { .. } => quit = true,
                Event::JoyDeviceAdded { which, .. } => {
                    if let Some((name, info)) = open_dev(&js, &gcs, &mut devs, *which) {
                        state.apply(
                            now(),
                            Input::Device {
                                name,
                                info,
                                added: true,
                            },
                        );
                    }
                }
                Event::JoyDeviceRemoved { which, .. } => {
                    if let Some(d) = devs.remove(which) {
                        state.apply(
                            now(),
                            Input::Device {
                                name: d.name,
                                info: String::new(),
                                added: false,
                            },
                        );
                    }
                }
                Event::Window {
                    win_event:
                        WindowEvent::SizeChanged(..)
                        | WindowEvent::Exposed
                        | WindowEvent::Restored
                        | WindowEvent::Shown,
                    ..
                } => force = true,
                other => {
                    if let Some(input) = translate(&devs, &raw_map, other) {
                        state.apply(now(), input);
                    }
                }
            }
        }
        let t = now();
        state.tick(t);
        if state.exit_ready(t) {
            break;
        }
        let (nw, nh) = canvas.output_size()?;
        if (nw, nh) != (w, h) {
            (w, h) = (nw, nh);
            rend.resize(w as usize, h as usize);
            tex = tc
                .create_texture_streaming(PixelFormatEnum::ARGB8888, w, h)
                .map_err(|e| e.to_string())?;
            force = true;
        }
        if state.dirty || force {
            state.dirty = false;
            force = false;
            let f = rend.draw(&state, t, &args.cfg);
            tex.update(None, f.data(), f.pitch())
                .map_err(|e| e.to_string())?;
            canvas.copy(&tex, None, None)?;
            canvas.present();
        } else {
            std::thread::sleep(Duration::from_millis(4));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rg52pad::pad::Kind;

    /// Ponta a ponta com um joystick virtual do SDL: mapeamento real, eventos reais.
    #[test]
    fn joystick_virtual_vira_eventos_nomeados_e_crus() {
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().unwrap();
        let js = sdl.joystick().unwrap();
        let gcs = sdl.game_controller().unwrap();
        let mut pump = sdl.event_pump().unwrap();
        // 6 eixos, 14 botoes (b12 = "HOME" sem nome no mapeamento), 1 hat
        let idx = unsafe {
            sdl2::sys::SDL_JoystickAttachVirtual(
                sdl2::sys::SDL_JoystickType::SDL_JOYSTICK_TYPE_GAMECONTROLLER,
                6,
                14,
                1,
            )
        };
        assert!(idx >= 0, "SDL sem joystick virtual");
        let joy = js.open(idx as u32).unwrap();
        let guid = joy.guid().string();
        gcs.add_mapping(&format!(
            "{guid},RG52 Virtual,a:b1,b:b0,x:b3,y:b2,start:b9,back:b8,dpup:h0.1,leftx:a0,lefty:a1,lefttrigger:a4,platform:Linux,"
        ))
        .unwrap();
        let mut devs = HashMap::new();
        let (_name, info) = open_dev(&js, &gcs, &mut devs, idx as u32).unwrap();
        assert!(info.contains("14 btn") && info.contains("mapped"), "{info}");
        // abrir de novo o mesmo dispositivo nao duplica
        assert!(open_dev(&js, &gcs, &mut devs, idx as u32).is_none());

        let mut st = State::new(parse_exit("start+b12").unwrap());
        unsafe {
            let j = sdl2::sys::SDL_JoystickFromInstanceID(joy.instance_id() as i32);
            sdl2::sys::SDL_JoystickSetVirtualButton(j, 1, 1); // b1 = A (mapeado)
            sdl2::sys::SDL_JoystickSetVirtualButton(j, 9, 1); // START
            sdl2::sys::SDL_JoystickSetVirtualButton(j, 12, 1); // sem mapeamento
            sdl2::sys::SDL_JoystickSetVirtualAxis(j, 4, 32767); // gatilho
            sdl2::sys::SDL_JoystickSetVirtualAxis(j, 5, 20000); // eixo sem mapeamento
            sdl2::sys::SDL_JoystickSetVirtualHat(j, 0, 1);
        }
        for _ in 0..3 {
            unsafe { sdl2::sys::SDL_JoystickUpdate() };
        }
        for ev in pump.poll_iter() {
            if let Some(i) = translate(&devs, &[], &ev) {
                st.apply(0.0, i);
            }
        }
        assert!(st.is_pressed(Btn::A), "A (b1) deveria estar apertado");
        assert!(st.is_pressed(Btn::Start));
        assert!(st.trig[0] > 0.99, "gatilho L2: {}", st.trig[0]);
        assert!(st.is_pressed(Btn::Up), "hat mapeado como dpup");
        // b12 e a5 nao estao no mapeamento: aparecem como crus, sem duplicar os mapeados
        let raws: Vec<_> = st
            .log
            .iter()
            .filter(|l| l.kind == Kind::Raw)
            .map(|l| l.name.clone())
            .collect();
        assert!(raws.contains(&"b12".to_string()), "{raws:?}");
        assert!(raws.contains(&"a5".to_string()), "{raws:?}");
        assert!(!raws.contains(&"b1".to_string()));
        // START + b12 (botao cru) segurados = combinacao de saida ativa
        assert!(st.holding_exit());
        // origem mostrada no log
        let a = st.log.iter().find(|l| l.name == "A").unwrap();
        assert_eq!(a.detail, "b1");
        unsafe {
            sdl2::sys::SDL_JoystickDetachVirtual(idx);
        }
    }
}
