mod ai;
mod ayar_ui;
mod ayarlar;
mod depo;
mod dinleme;
mod gomulu;
mod guncelleme;
mod kurulum;
mod mcp;
mod ses;
mod tema;
mod terminal;

use eframe::egui;

#[cfg(windows)]
fn konsolu_birak() {
    extern "system" {
        fn FreeConsole() -> i32;
    }
    unsafe { FreeConsole(); }
}
#[cfg(not(windows))]
fn konsolu_birak() {}

fn main() -> eframe::Result<()> {
    if std::env::args().any(|a| a == "--ayarlar") {
        konsolu_birak();
        let secenek = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 640.0]).with_min_inner_size([420.0, 360.0]).with_title("E.D.I.T.H — Ayarlar"),
            ..Default::default()
        };
        return eframe::run_native(
            "E.D.I.T.H Ayarlar",
            secenek,
            Box::new(|cc| {
                tema::kur(&cc.egui_ctx);
                Ok(Box::new(ayar_ui::AyarUyg::yeni(&cc.egui_ctx)))
            }),
        );
    }
    terminal::calistir();
    Ok(())
}
