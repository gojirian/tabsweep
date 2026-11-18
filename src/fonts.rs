//! Fira Sans for the interface and Fira Code for numbers, as in ironterm.
//!
//! Both families are SIL Open Font Licensed; the licence texts are in
//! `assets/`. The bytes are embedded so the machine does not need them
//! installed.

use std::sync::{Arc, LazyLock};

use egui::epaint::text::{FontData, FontDefinitions, FontFamily, FontId};

const FIRA_CODE_RETINA: &[u8] = include_bytes!("../assets/FiraCode-Retina.ttf");
const FIRA_SANS: &[u8] = include_bytes!("../assets/FiraSans-Regular.ttf");
const FIRA_SANS_MEDIUM: &[u8] = include_bytes!("../assets/FiraSans-Medium.ttf");
const FIRA_SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/FiraSans-SemiBold.ttf");

static MEDIUM: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name(Arc::from("FiraSans-Medium")));
static SEMIBOLD: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name(Arc::from("FiraSans-SemiBold")));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiWeight {
    Regular,
    Medium,
    Semibold,
}

/// Install the bundled faces ahead of egui's fallback fonts.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    insert(&mut fonts, "FiraCode-Retina", FIRA_CODE_RETINA);
    insert(&mut fonts, "FiraSans", FIRA_SANS);
    insert(&mut fonts, "FiraSans-Medium", FIRA_SANS_MEDIUM);
    insert(&mut fonts, "FiraSans-SemiBold", FIRA_SANS_SEMIBOLD);

    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.insert(0, "FiraCode-Retina".to_string());
    }
    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        proportional.insert(0, "FiraSans".to_string());
    }
    // Tab titles can hold any script, so the named families fall back to the
    // proportional list rather than to nothing.
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .insert(MEDIUM.clone(), with_first("FiraSans-Medium", &fallback));
    fonts
        .families
        .insert(SEMIBOLD.clone(), with_first("FiraSans-SemiBold", &fallback));
    ctx.set_fonts(fonts);
}

fn with_first(name: &str, rest: &[String]) -> Vec<String> {
    let mut names = vec![name.to_string()];
    names.extend(rest.iter().cloned());
    names
}

fn insert(fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]) {
    fonts
        .font_data
        .insert(name.to_string(), Arc::new(FontData::from_static(bytes)));
}

#[must_use]
pub fn medium_family() -> FontFamily {
    MEDIUM.clone()
}

#[must_use]
pub fn semibold_family() -> FontFamily {
    SEMIBOLD.clone()
}

#[must_use]
pub fn ui_font(size: f32, weight: UiWeight) -> FontId {
    let family = match weight {
        UiWeight::Regular => FontFamily::Proportional,
        UiWeight::Medium => MEDIUM.clone(),
        UiWeight::Semibold => SEMIBOLD.clone(),
    };
    FontId::new(size, family)
}

#[must_use]
pub fn mono_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}
