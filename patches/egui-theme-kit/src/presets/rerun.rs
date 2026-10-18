//! Rerun-inspired palettes: neutral greys under a pink accent.

use eframe::egui::Color32;

use super::palette::Palette;

const SUCCESS: Color32 = Color32::from_rgb(72, 199, 142);
const WARN: Color32 = Color32::from_rgb(255, 138, 76);
const ERROR: Color32 = Color32::from_rgb(255, 73, 99);
const INFO: Color32 = Color32::from_rgb(125, 195, 255);
const PRIMARY: Color32 = Color32::from_rgb(255, 56, 130);
const SECONDARY: Color32 = Color32::from_rgb(220, 38, 102);

pub const RERUN: Palette = Palette {
    name: "Rerun",
    primary: PRIMARY,
    secondary: SECONDARY,
    base: Color32::from_rgb(13, 13, 13),
    faint: Color32::from_rgb(20, 20, 20),
    sunken: Color32::from_rgb(28, 28, 28),
    widget: Color32::from_rgb(24, 24, 24),
    border: Color32::from_rgb(47, 47, 47),
    text: Color32::from_rgb(220, 220, 220),
    success: SUCCESS,
    warn: WARN,
    error: ERROR,
    info: Some(INFO),
};

pub const RERUN_OLED: Palette = Palette {
    name: "Rerun OLED",
    primary: PRIMARY,
    secondary: SECONDARY,
    base: Color32::from_rgb(0, 0, 0),
    faint: Color32::from_rgb(3, 3, 3),
    sunken: Color32::from_rgb(10, 10, 10),
    widget: Color32::from_rgb(6, 6, 6),
    border: Color32::from_rgb(36, 36, 36),
    text: Color32::from_rgb(232, 232, 232),
    success: SUCCESS,
    warn: WARN,
    error: ERROR,
    info: Some(INFO),
};
