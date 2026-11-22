//! Aparelho nao identificado: mostra todos os controles para nada ficar escondido.
//! Sai com START + SELECT porque nem todo aparelho tem HOME.

use super::Profile;
use crate::pad::Btn;

pub const PROFILE: Profile = Profile {
    id: "generic",
    title: "DARKOS",
    name: "Generico (aparelho nao identificado)",
    markers: &[],
    buttons: &Btn::ALL,
    sticks: [true, true],
    triggers: true,
    exit: "start+select",
    stick_on_top: true,
    alias: None,
    raw_map: &[],
};
