//! R36S, que usa a imagem `rg351mp` do dArkOS (RK3326, "GO-Super Gamepad").
//! Fonte: `Emulationstation/es_input.cfg.rg351mp` e `inttools/gamecontrollerdb.txt`.

use super::Profile;
use crate::pad::Btn::*;

pub const PROFILE: Profile = Profile {
    id: "r36s",
    title: "R36S",
    name: "R36S / RG351MP (RK3326)",
    markers: &["RG351MP", "RG351", "R36S"],
    buttons: &[
        A, B, X, Y, Up, Down, Left, Right, L1, R1, L3, R3, Start, Select,
    ],
    sticks: [true, true],
    triggers: true,
    // o mapeamento nao tem `guide`: START + HOME nunca dispararia
    exit: "start+select",
    stick_on_top: false,
    alias: None,
    raw_map: &[],
};
