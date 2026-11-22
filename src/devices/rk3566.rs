//! Familia RK3566 do dArkOS ("retrogame_joypad").
//! Fonte: `Emulationstation/es_input.cfg.*` e `inttools/gamecontrollerdb.txt`.
//! Todas essas imagens compartilham o mesmo mapeamento; HOME = `guide` (b10).

use super::Profile;
use crate::pad::Btn::*;

pub const PROFILE: Profile = Profile {
    id: "rk3566",
    title: "RK3566",
    name: "Familia RK3566",
    markers: &[
        "RK3566",
        "RG353M",
        "RG353V",
        "RG503",
        "RK2023",
        "RGB30",
        "RGB20PRO",
        "MINILOONG",
    ],
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
