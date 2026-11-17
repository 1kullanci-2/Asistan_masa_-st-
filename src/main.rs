#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod arayuz;
mod ayarlar;
mod depo;
mod mcp;
mod ses;
mod tema;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let secenek = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([440.0, 780.0]).with_min_inner_size([340.0, 480.0]).with_title("E.D.I.T.H"),
        ..Default::default()
    };
    eframe::run_native(
        "E.D.I.T.H",
        secenek,
        Box::new(|cc| {
            tema::kur(&cc.egui_ctx);
            let mut u = arayuz::Uygulama::yeni();
            u.baslat();
            Ok(Box::new(u))
        }),
    )
}
