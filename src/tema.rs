use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily};

pub const VISNE: Color32 = Color32::from_rgb(0xA7, 0x6C, 0x80);
pub const YUZEY: Color32 = Color32::from_rgb(0xB5, 0x83, 0x94);
pub const YUZEY_ACIK: Color32 = Color32::from_rgb(0xC1, 0x94, 0xA3);
pub const YUZEY_KENAR: Color32 = Color32::from_rgb(0xC7, 0x9C, 0xAB);
pub const VISNE_KOYU: Color32 = Color32::from_rgb(0x86, 0x50, 0x6A);
pub const KREM: Color32 = Color32::from_rgb(0xF7, 0xEC, 0xF0);
pub const KREM_KOYU: Color32 = Color32::from_rgb(0xEB, 0xD9, 0xE0);
pub const METIN_KOYU: Color32 = Color32::from_rgb(0x3B, 0x22, 0x30);
pub const METIN_KREM: Color32 = Color32::from_rgb(0xFF, 0xF8, 0xFA);
pub const SOLUK_KREM: Color32 = Color32::from_rgb(0xF3, 0xDC, 0xE5);
pub const VURGU: Color32 = Color32::from_rgb(0x8C, 0x4A, 0x63);
pub const YESIL_ACIK: Color32 = Color32::from_rgb(0xCF, 0xDD, 0xC0);
pub const YESIL: Color32 = Color32::from_rgb(0x3F, 0x8F, 0x5F);
pub const HATA: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
pub const HATA_ACIK: Color32 = Color32::from_rgb(0xFF, 0xE3, 0xDC);

pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}

pub fn kur(ctx: &egui::Context) {
    let mut f = FontDefinitions::default();
    let adaylar = ["C:\\Windows\\Fonts\\georgia.ttf", "C:\\Windows\\Fonts\\times.ttf"];
    for yol in adaylar {
        if let Ok(b) = std::fs::read(yol) {
            f.font_data.insert("serif".into(), FontData::from_owned(b));
            f.families.insert(serif(), vec!["serif".into()]);
            break;
        }
    }
    let orantili = f.families[&FontFamily::Proportional].clone();
    f.families.entry(serif()).or_insert(orantili);
    if let Ok(b) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
        f.font_data.insert("segoe".into(), FontData::from_owned(b));
        f.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "segoe".into());
    }
    ctx.set_fonts(f);

    let mut v = egui::Visuals::light();
    v.panel_fill = VISNE;
    v.window_fill = VISNE_KOYU;
    v.extreme_bg_color = YUZEY;
    v.override_text_color = Some(METIN_KREM);
    v.window_stroke = egui::Stroke::new(1.0, YUZEY_KENAR);
    v.selection.bg_fill = KREM_KOYU;
    v.selection.stroke = egui::Stroke::new(1.0, METIN_KOYU);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive] {
        w.rounding = egui::Rounding::same(12.0);
        w.bg_stroke = egui::Stroke::new(1.0, YUZEY_KENAR);
    }
    v.widgets.inactive.weak_bg_fill = YUZEY;
    v.widgets.hovered.weak_bg_fill = YUZEY_ACIK;
    v.widgets.active.weak_bg_fill = YUZEY_ACIK;
    v.widgets.inactive.bg_fill = YUZEY;
    ctx.set_visuals(v);
}
