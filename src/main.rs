#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

struct Uygulama;

impl eframe::App for Uygulama {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("E.D.I.T.H");
        });
    }
}

fn main() -> eframe::Result<()> {
    let secenek = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([420.0, 760.0]).with_title("E.D.I.T.H"),
        ..Default::default()
    };
    eframe::run_native("E.D.I.T.H", secenek, Box::new(|_| Ok(Box::new(Uygulama))))
}
