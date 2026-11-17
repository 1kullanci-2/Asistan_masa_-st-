use crate::ai::{self, Olay};
use crate::ayarlar::{servis_adi, Ayar, SERVISLER};
use crate::depo::{self, Mesaj, Sohbet};
use crate::tema::*;
use chrono::Timelike;
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Margin, RichText, Rounding, Sense, Stroke, Vec2};
use std::sync::mpsc::{channel, Receiver};

#[derive(PartialEq, Clone, Copy)]
enum Sayfa {
    Sohbet,
    Ayarlar,
    YapayZeka,
}

pub struct Uygulama {
    ayar: Ayar,
    sohbetler: Vec<Sohbet>,
    aktif: Sohbet,
    giris: String,
    yan_acik: bool,
    model_paneli: bool,
    sayfa: Sayfa,
    rx: Option<Receiver<Olay>>,
    akan: String,
    silinecek: Option<String>,
    alt_baslik: &'static str,
    hata: String,
}

fn selam() -> &'static str {
    match chrono::Local::now().hour() {
        5..=11 => "Günaydın",
        12..=17 => "İyi günler",
        18..=22 => "İyi akşamlar",
        _ => "İyi geceler",
    }
}

fn alt_baslik_sec() -> &'static str {
    let l = ["Bugün ne yapıyoruz?", "Nasılsınız?", "E.D.I.T.H.'e hoş geldiniz", "Size nasıl yardımcı olabilirim?"];
    l[(chrono::Local::now().timestamp_subsec_nanos() as usize) % l.len()]
}

fn yuvarlak_dugme(ui: &mut egui::Ui, boy: f32, dolgu: Color32, cizim: impl FnOnce(&egui::Painter, egui::Rect)) -> egui::Response {
    let (r, y) = ui.allocate_exact_size(Vec2::splat(boy), Sense::click());
    let dolgu = if y.hovered() { YUZEY_ACIK } else { dolgu };
    ui.painter().circle(r.center(), boy / 2.0, dolgu, Stroke::new(1.0, YUZEY_KENAR));
    cizim(ui.painter(), r);
    y.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn cizgi_ikon(p: &egui::Painter, r: egui::Rect, satir: usize) {
    let c = r.center();
    let s = Stroke::new(1.8, METIN_KREM);
    let ara = 5.0;
    let ilk = -(satir as f32 - 1.0) * ara / 2.0;
    for i in 0..satir {
        let y = c.y + ilk + i as f32 * ara;
        p.line_segment([egui::pos2(c.x - 7.0, y), egui::pos2(c.x + 7.0, y)], s);
    }
}

fn gonder_oku(p: &egui::Painter, r: egui::Rect) {
    let c = r.center();
    let s = Stroke::new(2.0, METIN_KREM);
    p.line_segment([egui::pos2(c.x, c.y + 6.0), egui::pos2(c.x, c.y - 6.0)], s);
    p.line_segment([egui::pos2(c.x - 5.0, c.y - 1.0), egui::pos2(c.x, c.y - 6.0)], s);
    p.line_segment([egui::pos2(c.x + 5.0, c.y - 1.0), egui::pos2(c.x, c.y - 6.0)], s);
}

fn mikrofon_ikon(p: &egui::Painter, r: egui::Rect) {
    let c = r.center();
    let s = Stroke::new(1.8, METIN_KREM);
    p.rect_stroke(egui::Rect::from_center_size(egui::pos2(c.x, c.y - 2.0), Vec2::new(8.0, 13.0)), Rounding::same(4.0), s);
    p.line_segment([egui::pos2(c.x, c.y + 7.0), egui::pos2(c.x, c.y + 10.0)], s);
}

impl Uygulama {
    pub fn yeni() -> Self {
        let ayar = Ayar::yukle();
        let sohbetler = depo::hepsi();
        let aktif = Sohbet::yeni(ayar.varsayilan_model());
        Self { ayar, sohbetler, aktif, giris: String::new(), yan_acik: false, model_paneli: false, sayfa: Sayfa::Sohbet, rx: None, akan: String::new(), silinecek: None, alt_baslik: alt_baslik_sec(), hata: String::new() }
    }

    fn yeni_sohbet(&mut self) {
        self.aktif = Sohbet::yeni(self.ayar.varsayilan_model());
        self.alt_baslik = alt_baslik_sec();
        self.yan_acik = false;
        self.sayfa = Sayfa::Sohbet;
    }

    fn kaydet_aktif(&mut self) {
        if self.aktif.mesajlar.is_empty() { return; }
        self.aktif.kaydet();
        self.sohbetler = depo::hepsi();
    }

    fn gonder(&mut self, ctx: &egui::Context) {
        let metin = self.giris.trim().to_string();
        if metin.is_empty() || self.rx.is_some() { return; }
        let Some((servis, model)) = self.aktif.model.split_once('|').map(|(a, b)| (a.to_string(), b.to_string())) else {
            self.hata = "Önce Ayarlar > Yapay zeka bölümünden bir anahtar ekle.".into();
            return;
        };
        self.hata.clear();
        self.giris.clear();
        self.aktif.mesajlar.push(Mesaj { k: "k".into(), m: metin });
        depo::ozetle(&mut self.aktif);
        let gecmis = depo::baglam(&self.aktif);
        let anahtar = self.ayar.anahtar(&servis).to_string();
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.akan.clear();
        let c = ctx.clone();
        std::thread::spawn(move || ai::akis(&servis, &model, &anahtar, gecmis, tx, move || c.request_repaint()));
        self.kaydet_aktif();
    }

    fn olaylari_isle(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut bitti = false;
        while let Ok(o) = rx.try_recv() {
            match o {
                Olay::Parca(t) => self.akan.push_str(&t),
                Olay::Bitti => bitti = true,
                Olay::Hata(h) => {
                    self.aktif.mesajlar.push(Mesaj { k: "n".into(), m: h });
                    bitti = true;
                }
            }
        }
        if bitti {
            self.rx = None;
            if !self.akan.is_empty() {
                let (temiz, baslik, _soru) = ai::isaretleri_ayir(&self.akan);
                if let Some(b) = baslik {
                    if self.aktif.baslik == "Yeni sohbet" && !b.is_empty() { self.aktif.baslik = b; }
                }
                self.aktif.mesajlar.push(Mesaj { k: "a".into(), m: temiz });
            }
            self.akan.clear();
            self.kaydet_aktif();
        }
    }

    fn ust_cubuk(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if yuvarlak_dugme(ui, 40.0, VISNE_KOYU, |p, r| cizgi_ikon(p, r, 3)).clicked() {
                self.yan_acik = !self.yan_acik;
            }
            if self.sayfa != Sayfa::Sohbet {
                if ui.button("‹ Geri").clicked() {
                    self.sayfa = if self.sayfa == Sayfa::YapayZeka { Sayfa::Ayarlar } else { Sayfa::Sohbet };
                }
            }
        });
    }

    fn sohbet_gorunumu(&mut self, ui: &mut egui::Ui) {
        if self.aktif.mesajlar.is_empty() && self.akan.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.30);
                ui.label(RichText::new(selam()).font(FontId::new(34.0, serif())).color(KREM));
                ui.add_space(6.0);
                ui.label(RichText::new(self.alt_baslik).font(FontId::new(17.0, serif())).color(SOLUK_KREM));
            });
            return;
        }
        let en = ui.available_width();
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for m in &self.aktif.mesajlar {
                mesaj_ciz(ui, &m.k, &m.m, en);
            }
            if self.rx.is_some() {
                if self.akan.is_empty() {
                    let t = ui.input(|i| i.time);
                    let a = (0.55 + 0.45 * (t * 3.0).sin().abs()) as f32;
                    ui.label(RichText::new("Düşünüyor…").italics().color(KREM.gamma_multiply(a)));
                    ui.ctx().request_repaint();
                } else {
                    let (t, _, _) = ai::isaretleri_ayir(&self.akan);
                    mesaj_ciz(ui, "a", &t, en);
                }
            }
            ui.add_space(8.0);
        });
    }

    fn giris_cubugu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Frame::none().fill(YUZEY).stroke(Stroke::new(1.0, YUZEY_KENAR)).rounding(Rounding::same(22.0)).inner_margin(Margin::symmetric(14.0, 8.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let y = ui.add(
                egui::TextEdit::multiline(&mut self.giris).desired_rows(1).desired_width(f32::INFINITY).frame(false).hint_text("Bir şey yaz…").text_color(METIN_KREM),
            );
            if y.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
                self.gonder(ctx);
                y.request_focus();
            }
            let yazili = !self.giris.trim().is_empty();
            if yazili || y.has_focus() {
                ui.horizontal(|ui| {
                    let etiket = self.aktif.model.split_once('|').map(|(s, m)| format!("{} · {}", servis_adi(s), m)).unwrap_or_else(|| "Model yok".into());
                    if ui.add(egui::Button::new(RichText::new(etiket).size(12.5)).rounding(Rounding::same(14.0))).clicked() {
                        self.model_paneli = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if yuvarlak_dugme(ui, 36.0, VISNE_KOYU, |p, r| if yazili { gonder_oku(p, r) } else { mikrofon_ikon(p, r) }).clicked() && yazili {
                            self.gonder(ctx);
                        }
                    });
                });
            }
        });
        if !self.hata.is_empty() {
            ui.label(RichText::new(&self.hata).size(12.5).color(HATA_ACIK));
        }
    }

    fn yan_panel(&mut self, ctx: &egui::Context) {
        if !self.yan_acik { return; }
        egui::SidePanel::left("yan").exact_width(304.0).frame(egui::Frame::none().fill(VISNE_KOYU).inner_margin(Margin::same(14.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Ayarlar").clicked() {
                    self.sayfa = Sayfa::Ayarlar;
                    self.yan_acik = false;
                }
            });
            ui.add_space(8.0);
            if ui.add_sized([ui.available_width(), 38.0], egui::Button::new(RichText::new("Yeni sohbet").strong())).clicked() {
                self.yeni_sohbet();
            }
            ui.add_space(10.0);
            let mut ac: Option<usize> = None;
            let mut sil: Option<String> = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, s) in self.sohbetler.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let sec = s.id == self.aktif.id;
                        let d = egui::Button::new(RichText::new(&s.baslik).color(if sec { KREM } else { SOLUK_KREM })).fill(if sec { YUZEY } else { Color32::TRANSPARENT }).stroke(Stroke::NONE);
                        if ui.add_sized([ui.available_width() - 34.0, 34.0], d).clicked() { ac = Some(i); }
                        if ui.small_button("Sil").clicked() { sil = Some(s.id.clone()); }
                    });
                }
            });
            if let Some(i) = ac {
                self.aktif = self.sohbetler[i].clone();
                self.yan_acik = false;
                self.sayfa = Sayfa::Sohbet;
            }
            if sil.is_some() { self.silinecek = sil; }
        });
    }

    fn pencereler(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.silinecek.clone() {
            egui::Window::new("Sohbet silinsin mi?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Sil").clicked() {
                        if let Some(s) = self.sohbetler.iter().find(|s| s.id == id) { s.sil(); }
                        if self.aktif.id == id { self.aktif = Sohbet::yeni(self.ayar.varsayilan_model()); }
                        self.sohbetler = depo::hepsi();
                        self.silinecek = None;
                    }
                    if ui.button("Vazgeç").clicked() { self.silinecek = None; }
                });
            });
        }
        if self.model_paneli {
            let mut acik = true;
            egui::Window::new("Model seç").open(&mut acik).collapsible(false).resizable(false).anchor(Align2::CENTER_BOTTOM, [0.0, -70.0]).show(ctx, |ui| {
                let sec = self.ayar.secenekler();
                if sec.is_empty() { ui.label("Henüz anahtar eklenmedi."); }
                for s in sec {
                    let (sv, m) = s.split_once('|').unwrap_or(("", ""));
                    let ad = format!("{} · {}", servis_adi(sv), m);
                    if ui.selectable_label(self.aktif.model == s, ad).clicked() {
                        self.aktif.model = s.clone();
                        self.kaydet_aktif();
                        self.model_paneli = false;
                    }
                }
            });
            if !acik { self.model_paneli = false; }
        }
    }

    fn ayarlar_sayfasi(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Ayarlar").font(FontId::new(26.0, serif())).color(KREM));
        ui.add_space(10.0);
        let kartlar = ["Yapay zeka", "Ses", "Uyandırma", "Mikrofon", "Bağlantılar", "İzinler", "Güncelleme"];
        for k in kartlar {
            let (r, y) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
            ui.painter().rect(r, Rounding::same(14.0), if y.hovered() { YUZEY_ACIK } else { YUZEY }, Stroke::new(1.0, YUZEY_KENAR));
            ui.painter().text(r.left_center() + Vec2::new(16.0, 0.0), Align2::LEFT_CENTER, k, FontId::new(16.0, serif()), METIN_KREM);
            if y.clicked() && k == "Yapay zeka" { self.sayfa = Sayfa::YapayZeka; }
            ui.add_space(6.0);
        }
    }

    fn yapay_zeka_sayfasi(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Yapay zeka").font(FontId::new(26.0, serif())).color(KREM));
        ui.label(RichText::new("Anahtarlar yalnızca bu bilgisayarda saklanır. Her servis için en fazla 2 model (virgülle).").size(12.5).color(SOLUK_KREM));
        ui.add_space(8.0);
        let mut degisti = false;
        for s in SERVISLER {
            ui.label(RichText::new(servis_adi(s)).strong());
            let (anahtar, modeller) = match s {
                "gemini" => (&mut self.ayar.gemini_anahtar, &mut self.ayar.gemini_modeller),
                "groq" => (&mut self.ayar.groq_anahtar, &mut self.ayar.groq_modeller),
                "cerebras" => (&mut self.ayar.cerebras_anahtar, &mut self.ayar.cerebras_modeller),
                _ => (&mut self.ayar.mistral_anahtar, &mut self.ayar.mistral_modeller),
            };
            degisti |= ui.add(egui::TextEdit::singleline(anahtar).password(true).hint_text("API anahtarı").desired_width(f32::INFINITY)).changed();
            degisti |= ui.add(egui::TextEdit::singleline(modeller).hint_text("Modeller").desired_width(f32::INFINITY)).changed();
            ui.add_space(8.0);
        }
        if degisti {
            self.ayar.kaydet();
            if self.aktif.mesajlar.is_empty() && !self.aktif.model.contains('|') || !self.ayar.secenekler().contains(&self.aktif.model) && self.aktif.mesajlar.is_empty() {
                self.aktif.model = self.ayar.varsayilan_model();
            }
        }
    }
}

fn mesaj_ciz(ui: &mut egui::Ui, tur: &str, metin: &str, en: f32) {
    ui.add_space(6.0);
    match tur {
        "k" => {
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                egui::Frame::none().fill(YUZEY_ACIK).stroke(Stroke::new(1.0, YUZEY_KENAR)).rounding(Rounding::same(16.0)).inner_margin(Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
                    ui.set_max_width(en * 0.78);
                    ui.label(RichText::new(metin).size(15.0).color(METIN_KOYU));
                });
            });
        }
        "n" => {
            ui.label(RichText::new(metin).size(12.0).color(SOLUK_KREM));
        }
        _ => {
            ui.label(RichText::new(metin).font(FontId::new(16.5, serif())).color(METIN_KREM));
        }
    }
}

impl eframe::App for Uygulama {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.olaylari_isle();
        self.yan_panel(ctx);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(VISNE).inner_margin(Margin::same(14.0))).show(ctx, |ui| {
            self.ust_cubuk(ui);
            ui.add_space(6.0);
            match self.sayfa {
                Sayfa::Sohbet => {
                    egui::TopBottomPanel::bottom("giris").frame(egui::Frame::none().fill(VISNE)).show_inside(ui, |ui| {
                        ui.add_space(6.0);
                        self.giris_cubugu(ui, ctx);
                    });
                    self.sohbet_gorunumu(ui);
                }
                Sayfa::Ayarlar => self.ayarlar_sayfasi(ui),
                Sayfa::YapayZeka => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.yapay_zeka_sayfasi(ui));
                }
            }
        });
        self.pencereler(ctx);
    }
}
