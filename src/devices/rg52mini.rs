//! RG52 Mini (RK3562, "rk3562-joystick"), do fork bmdhacks/dArkOS_rg52mini.
//! Fonte: `Emulationstation/es_input.cfg.rg52mini` e `inttools/gamecontrollerdb.txt`.
//! HOME = `guide` (b10). BACK e desenhado conforme o manual do aparelho.

use super::Profile;
use crate::pad::Btn::*;

pub const PROFILE: Profile = Profile {
    id: "rg52mini",
    title: "RG52",
    name: "RG52 Mini (RK3562)",
    markers: &["RG52MINI", "RG52"],
    buttons: &[
        A, B, X, Y, Up, Down, Left, Right, L1, R1, L3, R3, Start, Select, Home, Back,
    ],
    sticks: [true, true],
    triggers: true,
    exit: "start+home",
    stick_on_top: true,
    alias: None,
    raw_map: &[],
};
