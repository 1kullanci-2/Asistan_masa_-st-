use crate::ai::{self, Olay};
use crate::ayarlar::{servis_adi, Ayar, McpAyar, SERVISLER};
use crate::mcp::Mcp;
use crate::ses::{Oynatici, GEMINI_SESLER};
use std::sync::{Arc, Mutex};
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
    Ses,
    Baglantilar,
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
    araclar: ai::Araclar,
    oynatici: Arc<Oynatici>,
    arac_durum: String,
    mcp_duzen: Option<(Option<usize>, McpAyar)>,
    sesli_komut: bool,
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
        Self { ayar, sohbetler, aktif, giris: String::new(), yan_acik: false, model_paneli: false, sayfa: Sayfa::Sohbet, rx: None, akan: String::new(), silinecek: None, alt_baslik: alt_baslik_sec(), hata: String::new(), araclar: Arc::new(Mutex::new(vec![])), oynatici: Oynatici::yeni(), arac_durum: String::new(), mcp_duzen: None, sesli_komut: false }
    }

    pub fn baslat(&mut self) {
        self.mcp_yenile();
    }

    fn mcp_yenile(&mut self) {
        let liste: Vec<Mcp> = self.ayar.mcp.iter().map(|m| Mcp::yeni(&m.ad, &m.url, &m.baslik)).collect();
        let n = liste.len();
        *self.araclar.lock().unwrap() = liste;
        let a = self.araclar.clone();
        std::thread::spawn(move || {
            for i in 0..n {
                let mut k = a.lock().unwrap();
                if let Some(m) = k.get_mut(i) { let _ = m.baglan(); }
            }
        });
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
        let araclar = self.araclar.clone();
        let kapali: Vec<Vec<String>> = self.ayar.mcp.iter().map(|m| m.kapali.clone()).collect();
        self.arac_durum.clear();
        std::thread::spawn(move || ai::akis(&servis, &model, &anahtar, gecmis, araclar, kapali, tx, move || c.request_repaint()));
        self.kaydet_aktif();
    }

    fn olaylari_isle(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut bitti = false;
        while let Ok(o) = rx.try_recv() {
            match o {
                Olay::Parca(t) => {
                    self.arac_durum.clear();
                    self.akan.push_str(&t);
                }
                Olay::Arac(a) => self.arac_durum = a,
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
                if !self.ayar.ses_sadece_sesli || self.sesli_komut { self.oynatici.konus(&temiz, &self.ayar); }
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
                    self.sayfa = if self.sayfa == Sayfa::Ayarlar { Sayfa::Sohbet } else { Sayfa::Ayarlar };
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
                    let yazi = if self.arac_durum.is_empty() { "Düşünüyor…".to_string() } else { format!("Araç çalışıyor · {}…", self.arac_durum) };
                    ui.label(RichText::new(yazi).italics().color(KREM.gamma_multiply(a)));
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
            if y.clicked() {
                match k {
                    "Yapay zeka" => self.sayfa = Sayfa::YapayZeka,
                    "Ses" => self.sayfa = Sayfa::Ses,
                    "Bağlantılar" => self.sayfa = Sayfa::Baglantilar,
                    _ => {}
                }
            }
            ui.add_space(6.0);
        }
    }

    fn onizle(&self, motor: &str, ses: Option<&str>) {
        let mut a = self.ayar.clone();
        a.ses_sirasi = vec![motor.to_string()];
        if let Some(s) = ses { a.gemini_ses = s.to_string(); }
        self.oynatici.konus("Merhaba, ben E.D.I.T.H. Sesim böyle.", &a);
    }

    fn ses_sayfasi(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Ses").font(FontId::new(26.0, serif())).color(KREM));
        ui.add_space(8.0);
        ui.label(RichText::new("Gemini sesleri").strong());
        let mut sec: Option<String> = None;
        egui::ScrollArea::horizontal().id_salt("gs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, ad) in GEMINI_SESLER.iter().enumerate() {
                    let (r, y) = ui.allocate_exact_size(Vec2::splat(84.0), Sense::click());
                    let secili = self.ayar.gemini_ses == *ad;
                    let ton = (i as f32 * 37.0) % 360.0;
                    let renk = egui::ecolor::Hsva::new(ton / 360.0, 0.28, 0.62, 1.0);
                    ui.painter().rect(r, Rounding::same(14.0), Color32::from(renk), Stroke::new(if secili { 2.5 } else { 1.0 }, if secili { KREM } else { YUZEY_KENAR }));
                    for b in 0..7 {
                        let h = 8.0 + (((i * 7 + b * 5) % 9) as f32) * 3.0;
                        let x = r.left() + 14.0 + b as f32 * 9.5;
                        ui.painter().line_segment([egui::pos2(x, r.top() + 34.0 - h / 2.0), egui::pos2(x, r.top() + 34.0 + h / 2.0)], Stroke::new(3.0, METIN_KREM));
                    }
                    ui.painter().text(egui::pos2(r.center().x, r.bottom() - 14.0), Align2::CENTER_CENTER, ad, FontId::proportional(12.5), METIN_KOYU);
                    if y.clicked() { sec = Some(ad.to_string()); }
                }
            });
        });
        if let Some(s) = sec {
            self.ayar.gemini_ses = s.clone();
            self.ayar.kaydet();
            self.onizle("gemini", Some(&s));
        }
        ui.add_space(12.0);
        ui.label(RichText::new("ElevenLabs").strong());
        let mut d = false;
        d |= ui.add(egui::TextEdit::singleline(&mut self.ayar.eleven_anahtar).password(true).hint_text("ElevenLabs API anahtarı").desired_width(f32::INFINITY)).changed();
        d |= ui.add(egui::TextEdit::singleline(&mut self.ayar.eleven_ses).hint_text("Voice ID").desired_width(f32::INFINITY)).changed();
        d |= ui.add(egui::TextEdit::singleline(&mut self.ayar.eleven_model).hint_text("Model").desired_width(f32::INFINITY)).changed();
        d |= ui.add(egui::Slider::new(&mut self.ayar.eleven_kararlilik, 0.0..=1.0).text("Kararlılık")).changed();
        d |= ui.add(egui::Slider::new(&mut self.ayar.eleven_benzerlik, 0.0..=1.0).text("Benzerlik")).changed();
        if ui.button("ElevenLabs'i dene").clicked() { self.onizle("eleven", None); }
        ui.add_space(12.0);
        ui.label(RichText::new("Motor sırası").strong());
        let n = self.ayar.ses_sirasi.len();
        let mut tasi: Option<(usize, usize)> = None;
        for i in 0..n {
            ui.horizontal(|ui| {
                let ad = if self.ayar.ses_sirasi[i] == "gemini" { "Gemini" } else { "ElevenLabs" };
                ui.label(format!("{}. {}", i + 1, ad));
                if i > 0 && ui.small_button("Yukarı").clicked() { tasi = Some((i, i - 1)); }
                if i + 1 < n && ui.small_button("Aşağı").clicked() { tasi = Some((i, i + 1)); }
            });
        }
        if let Some((a, b)) = tasi { self.ayar.ses_sirasi.swap(a, b); d = true; }
        d |= ui.checkbox(&mut self.ayar.ses_sadece_sesli, "Sadece sesli komutlarda sesli cevap ver").changed();
        if d { self.ayar.kaydet(); }
    }

    fn baglanti_sayfasi(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Bağlantılar").font(FontId::new(26.0, serif())).color(KREM));
        ui.label(RichText::new("MCP sunucuları: eklediğin araçları asistan kullanabilir.").size(12.5).color(SOLUK_KREM));
        ui.add_space(8.0);
        let mut sil: Option<usize> = None;
        let mut yeniden = false;
        let durumlar: Vec<(bool, usize, String)> = match self.araclar.try_lock() {
            Ok(k) => k.iter().map(|m| (m.bagli, m.araclar.len(), m.hata.clone())).collect(),
            Err(_) => vec![],
        };
        let araclar_adlari: Vec<Vec<String>> = match self.araclar.try_lock() {
            Ok(k) => k.iter().map(|m| m.araclar.iter().map(|a| a.ad.clone()).collect()).collect(),
            Err(_) => vec![],
        };
        for i in 0..self.ayar.mcp.len() {
            egui::Frame::none().fill(YUZEY).stroke(Stroke::new(1.0, YUZEY_KENAR)).rounding(Rounding::same(14.0)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let (bagli, adet, hata) = durumlar.get(i).cloned().unwrap_or((false, 0, String::new()));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.ayar.mcp[i].ad).strong());
                    let (t, c) = if bagli { (format!("bağlı · {adet} araç"), YESIL_ACIK) } else if hata.is_empty() { ("bağlanıyor…".to_string(), SOLUK_KREM) } else { ("bağlanamadı".to_string(), HATA_ACIK) };
                    ui.label(RichText::new(t).size(12.0).color(c));
                });
                ui.horizontal(|ui| {
                    if ui.small_button("Düzenle").clicked() { self.mcp_duzen = Some((Some(i), self.ayar.mcp[i].clone())); }
                    if ui.small_button("Yeniden bağlan").clicked() { yeniden = true; }
                    if ui.small_button("Sil").clicked() { sil = Some(i); }
                });
                if let Some(adlar) = araclar_adlari.get(i) {
                    egui::CollapsingHeader::new("Araçlar").id_salt(("araclar", i)).show(ui, |ui| {
                        for a in adlar {
                            let mut acik = !self.ayar.mcp[i].kapali.contains(a);
                            if ui.checkbox(&mut acik, a).changed() {
                                self.ayar.mcp[i].kapali.retain(|x| x != a);
                                if !acik { self.ayar.mcp[i].kapali.push(a.clone()); }
                                self.ayar.kaydet();
                            }
                        }
                    });
                }
            });
            ui.add_space(6.0);
        }
        if let Some(i) = sil { self.ayar.mcp.remove(i); self.ayar.kaydet(); yeniden = true; }
        if yeniden { self.mcp_yenile(); }
        if self.mcp_duzen.is_none() && ui.button("+ Bağlantı ekle").clicked() { self.mcp_duzen = Some((None, McpAyar::default())); }
        let mut bitir: Option<bool> = None;
        if let Some((_, m)) = &mut self.mcp_duzen {
            ui.add_space(6.0);
            ui.add(egui::TextEdit::singleline(&mut m.ad).hint_text("Ad").desired_width(f32::INFINITY));
            ui.add(egui::TextEdit::singleline(&mut m.url).hint_text("URL").desired_width(f32::INFINITY));
            ui.add(egui::TextEdit::singleline(&mut m.baslik).hint_text("Başlık: Authorization: Bearer …").desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                if ui.button("Kaydet").clicked() { bitir = Some(true); }
                if ui.button("Vazgeç").clicked() { bitir = Some(false); }
            });
        }
        if let Some(kaydet) = bitir {
            if let (true, Some((idx, m))) = (kaydet, self.mcp_duzen.take()) {
                match idx { Some(i) => self.ayar.mcp[i] = m, None => self.ayar.mcp.push(m) }
                self.ayar.kaydet();
                self.mcp_yenile();
            } else { self.mcp_duzen = None; }
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
                Sayfa::Ses => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.ses_sayfasi(ui));
                }
                Sayfa::Baglantilar => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.baglanti_sayfasi(ui));
                }
            }
        });
        self.pencereler(ctx);
    }
}
