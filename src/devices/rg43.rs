//! RG43 Pro (RK3562, 1024x768), mesmo "rk3562-joystick" do RG52 Mini.
//! Fonte: `Emulationstation/es_input.cfg.rg43h` do fork bmdhacks/dArkOS_rg52mini.
//! A imagem do RG43 V Pro tambem grava `RG43H` no `.DEVICE`.

use super::Profile;
use crate::pad::Btn::*;

pub const PROFILE: Profile = Profile {
    id: "rg43",
    title: "RG43",
    name: "RG43 Pro (RK3562)",
    markers: &["RG43H", "RG43V", "RG43"],
    buttons: &[
        A, B, X, Y, Up, Down, Left, Right, L1, R1, L3, R3, Start, Select, Home,
    ],
    sticks: [true, true],
    triggers: true,
    exit: "start+home",
    stick_on_top: false,
    alias: None,
    raw_map: &[],
};
