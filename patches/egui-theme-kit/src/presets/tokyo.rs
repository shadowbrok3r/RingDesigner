//! Tokyo Night palettes.

use eframe::egui::Color32;

use super::palette::Palette;

pub const TOKYO_NIGHT: Palette = Palette {
    name: "Tokyo Night",
    primary: Color32::from_rgb(113, 189, 251),
    secondary: Color32::from_rgb(215, 135, 255),
    base: Color32::from_rgb(34, 35, 39),
    faint: Color32::from_rgb(28, 29, 33),
    sunken: Color32::from_rgb(41, 42, 46),
    widget: Color32::from_rgb(31, 32, 36),
    border: Color32::from_rgb(49, 50, 54),
    text: Color32::from_rgb(196, 200, 213),
    success: Color32::from_rgb(80, 250, 123),
    warn: Color32::from_rgb(255, 215, 64),
    error: Color32::from_rgb(229, 46, 47),
    info: Some(Color32::from_rgb(113, 189, 251)),
};

pub const TOKYO_NIGHT_STORM: Palette = Palette {
    name: "Tokyo Night Storm",
    primary: Color32::from_rgb(138, 171, 244),
    secondary: Color32::from_rgb(97, 175, 239),
    base: Color32::from_rgb(23, 24, 38),
    faint: Color32::from_rgb(31, 31, 51),
    sunken: Color32::from_rgb(33, 35, 53),
    widget: Color32::from_rgb(27, 29, 45),
    border: Color32::from_rgb(42, 42, 68),
    text: Color32::from_rgb(204, 204, 204),
    success: Color32::from_rgb(86, 209, 123),
    warn: Color32::from_rgb(255, 161, 90),
    error: Color32::from_rgb(255, 121, 121),
    info: None,
};
