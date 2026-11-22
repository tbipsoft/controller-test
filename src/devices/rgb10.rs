//! RGB10 (RK3326, "GO-Advance Gamepad (rev 1.1)"), um unico analogico.
//! Fonte: `Emulationstation/es_input.cfg.rgb10` e `inttools/gamecontrollerdb.txt`.
//!
//! O SDL chama o b16 de `rightstick`, mas o dArkOS o usa como hotkey e nao ha analogico
//! direito; por isso ele aparece como o botao "F".
//! A mesma imagem atende OGA 1.1 (sem analogico) e RGB10s (sem L2/R2), e o `.DEVICE` nao
//! diferencia essas variantes.

use super::Profile;
use crate::pad::Btn::{self, *};

pub const PROFILE: Profile = Profile {
    id: "rgb10",
    title: "RGB10",
    name: "RGB10 (RK3326)",
    markers: &["RGB10"],
    buttons: &[
        A, B, X, Y, Up, Down, Left, Right, L1, R1, L3, R3, Start, Select,
    ],
    sticks: [true, false],
    triggers: true,
    exit: "start+select",
    stick_on_top: false,
    alias: Some((Btn::R3, "F")),
    raw_map: &[],
};
