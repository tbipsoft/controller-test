//! A10 Mini e A10 Mini V4 (RK3326, "GO-Super Gamepad").
//! Fonte: `Emulationstation/es_input.cfg.a10mini`: hotkey no b16, sem nome no SDL.

use super::Profile;
use crate::pad::Btn::{self, *};

pub const PROFILE: Profile = Profile {
    id: "a10mini",
    title: "A10 MINI",
    name: "A10 Mini / A10 Mini V4 (RK3326)",
    markers: &["A10MINI", "A10MINIV4"],
    buttons: &[
        A, B, X, Y, Up, Down, Left, Right, L1, R1, L3, R3, Start, Select, Home,
    ],
    sticks: [true, true],
    triggers: true,
    exit: "start+select",
    stick_on_top: false,
    alias: Some((Btn::Home, "F")),
    raw_map: &[(16, Btn::Home)],
};
