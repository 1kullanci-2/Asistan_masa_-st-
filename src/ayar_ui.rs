//! Ayarlar penceresi (mobil ayarlarla aynı tasarım, yatay/yeniden boyutlanır).
use crate::ayarlar::{servis_adi, Ayar, McpAyar, SERVISLER};
use crate::dinleme::{self, DEvent, DKomut, Dinleyici};
use crate::kurulum::{self, Ilerleme};
use crate::mcp::Mcp;
use crate::ses::{self, Oynatici, GEMINI_SESLER};
use crate::tema::*;
use crate::{depo, guncelleme};
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Margin, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use std::sync::{Arc, Mutex};

const KATEGORILER: [(&str, &str); 8] = [
    ("Yapay zeka", "Servisler, anahtarlar, modeller"),
    ("Ses", "Sesler, motor sırası, ElevenLabs"),
    ("Uyandırma", "Kelimeler, öğrenme, davranış"),
    ("Mikrofon", "Test, takip süresi, modeller"),
    ("Bağlantılar", "MCP sunucuları ve araçlar"),
    ("Sohbetler", "İsim düzenle, sil"),
    ("İzinler", "Mikrofon ve erişim"),
    ("Güncelleme", "Sürüm ve yükleme"),
];

// ---------- küçük bileşenler ----------
fn kart(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    egui::Frame::none().fill(YUZEY).stroke(Stroke::new(1.0, YUZEY_KENAR)).rounding(Rounding::same(16.0)).inner_margin(Margin::same(14.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        f(ui);
    });
    ui.add_space(10.0);
}

fn sayfa_basligi(ui: &mut Ui, t: &str, alt: &str) {
    ui.label(RichText::new(t).font(FontId::new(28.0, serif())).color(KREM));
    ui.label(RichText::new(alt).size(13.0).color(SOLUK_KREM));
    ui.add_space(12.0);
}

fn baslik(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).font(FontId::new(17.0, serif())).color(KREM));
    ui.add_space(4.0);
}

fn alt_yazi(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).size(12.5).color(SOLUK_KREM));
}

fn anahtar(ui: &mut Ui, v: &mut bool) -> bool {
    let (r, mut y) = ui.allocate_exact_size(Vec2::new(46.0, 26.0), Sense::click());
    if y.clicked() {
        *v = !*v;
        y.mark_changed();
    }
    let t = ui.ctx().animate_bool(y.id, *v);
    ui.painter().rect(r, Rounding::same(13.0), YUZEY_ACIK.lerp_to_gamma(YESIL, t), Stroke::new(1.0, YUZEY_KENAR));
    let x = egui::lerp((r.left() + 13.0)..=(r.right() - 13.0), t);
    ui.painter().circle_filled(egui::pos2(x, r.center().y), 9.5, KREM);
    y.on_hover_cursor(egui::CursorIcon::PointingHand).changed()
}

fn satir_anahtar(ui: &mut Ui, baslik: &str, aciklama: &str, v: &mut bool) -> bool {
    let mut d = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() - 60.0);
            ui.label(RichText::new(baslik).size(15.0));
            alt_yazi(ui, aciklama);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| d = anahtar(ui, v));
    });
    ui.add_space(6.0);
    d
}

fn dugme(ui: &mut Ui, t: &str, vurgulu: bool) -> egui::Response {
    let b = if vurgulu {
        egui::Button::new(RichText::new(t).color(METIN_KOYU).strong()).fill(KREM).stroke(Stroke::NONE)
    } else {
        egui::Button::new(RichText::new(t)).fill(VISNE_KOYU)
    };
    ui.add(b.rounding(Rounding::same(14.0)).min_size(Vec2::new(0.0, 32.0))).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn sekmeler(ui: &mut Ui, secili: &mut usize, adlar: &[&str]) {
    egui::Frame::none().fill(VISNE_KOYU).rounding(Rounding::same(16.0)).inner_margin(Margin::same(4.0)).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, a) in adlar.iter().enumerate() {
                let sec = *secili == i;
                let b = egui::Button::new(RichText::new(*a).color(if sec { METIN_KOYU } else { SOLUK_KREM })).fill(if sec { KREM } else { Color32::TRANSPARENT }).stroke(Stroke::NONE).rounding(Rounding::same(12.0));
                if ui.add(b).clicked() { *secili = i; }
            }
        });
    });
    ui.add_space(8.0);
}

fn metin_alani(ui: &mut Ui, v: &mut String, ipucu: &str, gizli: bool) -> bool {
    ui.add(egui::TextEdit::singleline(v).password(gizli).hint_text(ipucu).desired_width(f32::INFINITY).margin(Margin::symmetric(10.0, 8.0))).changed()
}

fn ses_kartlari(ui: &mut Ui, kimlik: &str, ogeler: &[(String, String)], secili: &str) -> Option<String> {
    let mut sec = None;
    egui::ScrollArea::horizontal().id_salt(kimlik.to_string()).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (id, ad) in ogeler {
                let (r, y) = ui.allocate_exact_size(Vec2::splat(88.0), Sense::click());
                let s = secili == id;
                let ton: u32 = ad.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 360;
                let renk = egui::ecolor::Hsva::new(ton as f32 / 360.0, 0.26, if y.hovered() { 0.68 } else { 0.62 }, 1.0);
                ui.painter().rect(r, Rounding::same(16.0), Color32::from(renk), Stroke::new(if s { 2.5 } else { 1.0 }, if s { KREM } else { YUZEY_KENAR }));
                let tohum: u32 = ad.bytes().fold(3u32, |a, b| a.wrapping_mul(17).wrapping_add(b as u32));
                for b in 0..7u32 {
                    let h = 8.0 + (((tohum >> (b * 2)) & 7) as f32) * 3.0;
                    let x = r.left() + 16.0 + b as f32 * 9.5;
                    ui.painter().line_segment([egui::pos2(x, r.top() + 34.0 - h / 2.0), egui::pos2(x, r.top() + 34.0 + h / 2.0)], Stroke::new(3.0, METIN_KREM));
                }
                let kisa: String = ad.chars().take(11).collect();
                ui.painter().text(egui::pos2(r.center().x, r.bottom() - 14.0), Align2::CENTER_CENTER, kisa, FontId::proportional(12.5), METIN_KOYU);
                if y.clicked() { sec = Some(id.clone()); }
            }
        });
    });
    sec
}

struct Getir<T> {
    kutu: Arc<Mutex<Option<Result<T, String>>>>,
    bekliyor: bool,
    veri: Option<T>,
    hata: String,
}

impl<T: Send + 'static> Getir<T> {
    fn yeni() -> Self {
        Self { kutu: Arc::new(Mutex::new(None)), bekliyor: false, veri: None, hata: String::new() }
    }
    fn baslat(&mut self, ctx: &egui::Context, f: impl FnOnce() -> Result<T, String> + Send + 'static) {
        self.bekliyor = true;
        self.hata.clear();
        let k = self.kutu.clone();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let r = f();
            *k.lock().unwrap() = Some(r);
            c.request_repaint();
        });
    }
    fn yokla(&mut self) {
        if let Some(r) = self.kutu.lock().unwrap().take() {
            self.bekliyor = false;
            match r {
                Ok(v) => self.veri = Some(v),
                Err(e) => self.hata = e,
            }
        }
    }
}

pub struct AyarUyg {
    ayar: Ayar,
    sayfa: usize,
    sayfa_acik: bool,
    gizli: [bool; 5],
    ses_sekme: usize,
    oynatici: Arc<Oynatici>,
    el_sesler: Getir<Vec<(String, String)>>,
    el_modeller: Getir<Vec<(String, String)>>,
    el_kredi: Getir<(u64, u64)>,
    araclar: crate::ai::Araclar,
    mcp_duzen: Option<(Option<usize>, McpAyar)>,
    mcp_test: Getir<String>,
    sohbetler: Vec<depo::Sohbet>,
    ad_duzen: Option<(String, String)>,
    sil_onay: Option<String>,
    dinleyici: Option<Dinleyici>,
    dinleyici_denendi: bool,
    dinle_hata: String,
    dinle_hazir: bool,
    seviye: f32,
    test_acik: bool,
    test_en: String,
    test_tr: String,
    ogren_acik: bool,
    ogren_n: usize,
    kurulum: Option<Arc<Ilerleme>>,
    kontrol: Getir<Option<(u32, String)>>,
    kur: Getir<()>,
    ctx: egui::Context,
}

impl AyarUyg {
    pub fn yeni(ctx: &egui::Context) -> Self {
        ctx.style_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(14.0, 7.0);
            s.spacing.slider_width = 200.0;
            for (_, f) in s.text_styles.iter_mut() { f.size *= 1.08; }
        });
        let ayar = Ayar::yukle();
        let araclar: crate::ai::Araclar = Arc::new(Mutex::new(vec![]));
        crate::mcp::yenile(&araclar, &ayar.mcp);
        Self {
            ayar, sayfa: 0, sayfa_acik: false, gizli: [false; 5], ses_sekme: 0, oynatici: Oynatici::yeni(),
            el_sesler: Getir::yeni(), el_modeller: Getir::yeni(), el_kredi: Getir::yeni(), araclar, mcp_duzen: None, mcp_test: Getir::yeni(),
            sohbetler: depo::hepsi(), ad_duzen: None, sil_onay: None, dinleyici: None, dinleyici_denendi: false, dinle_hata: String::new(), dinle_hazir: false,
            seviye: 0.0, test_acik: false, test_en: String::new(), test_tr: String::new(), ogren_acik: false, ogren_n: 0, kurulum: None,
            kontrol: Getir::yeni(), kur: Getir::yeni(), ctx: ctx.clone(),
        }
    }

    fn kaydet(&self) {
        self.ayar.kaydet();
        if let Some(d) = &self.dinleyici { let _ = d.komut.send(DKomut::Ayar(self.ayar.dinleme_ayari())); }
    }

    fn dinleyici_baslat(&mut self) {
        if self.dinleyici.is_some() || self.dinleyici_denendi || !dinleme::modeller_hazir() { return; }
        self.dinleyici_denendi = true;
        let c = self.ctx.clone();
        self.dinleyici = Some(dinleme::baslat(Arc::new(move || c.request_repaint()), self.ayar.dinleme_ayari(), self.oynatici.bayrak()));
    }

    fn olaylar(&mut self) {
        let mut ev = vec![];
        if let Some(d) = &self.dinleyici { while let Ok(e) = d.olay.try_recv() { ev.push(e); } }
        for e in ev {
            match e {
                DEvent::Hazir => { self.dinle_hazir = true; self.dinle_hata.clear(); }
                DEvent::Hata(h) => self.dinle_hata = h,
                DEvent::Seviye(s) => self.seviye = self.seviye * 0.6 + s * 0.4,
                DEvent::TestMetin(a, b) => { self.test_en = a; self.test_tr = b; }
                DEvent::Ogrenme(n) => self.ogren_n = n,
                DEvent::Ogrenildi(v) => {
                    for x in v { if !self.ayar.uyandirma_ekstra.contains(&x) { self.ayar.uyandirma_ekstra.push(x); } }
                    self.ogren_acik = false;
                    self.kaydet();
                }
                DEvent::Uyandi if !self.test_acik && !self.ogren_acik => {
                    if let Some(d) = &self.dinleyici { let _ = d.komut.send(DKomut::Bekle); }
                }
                _ => {}
            }
        }
    }

    fn komut(&self, k: DKomut) {
        if let Some(d) = &self.dinleyici { let _ = d.komut.send(k); }
    }

    // ---------- sayfalar ----------
    fn yapay_zeka(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Yapay zeka", "Anahtarlar yalnızca bu bilgisayarda saklanır. Her servis için en fazla 2 model, virgülle ayır.");
        let mut d = false;
        for (i, s) in SERVISLER.iter().enumerate() {
            let var = !self.ayar.anahtar(s).trim().is_empty();
            kart(ui, |ui| {
                ui.horizontal(|ui| {
                    baslik(ui, servis_adi(s));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(if var { "anahtar var" } else { "anahtar yok" }).size(12.5).color(if var { YESIL_ACIK } else { SOLUK_KREM }));
                    });
                });
                let (a, m) = match *s {
                    "gemini" => (&mut self.ayar.gemini_anahtar, &mut self.ayar.gemini_modeller),
                    "groq" => (&mut self.ayar.groq_anahtar, &mut self.ayar.groq_modeller),
                    "cerebras" => (&mut self.ayar.cerebras_anahtar, &mut self.ayar.cerebras_modeller),
                    _ => (&mut self.ayar.mistral_anahtar, &mut self.ayar.mistral_modeller),
                };
                ui.horizontal(|ui| {
                    let gizli = !self.gizli[i];
                    ui.scope(|ui| {
                        ui.set_width(ui.available_width() - 76.0);
                        d |= metin_alani(ui, a, "API anahtarı", gizli);
                    });
                    if dugme(ui, if gizli { "Göster" } else { "Gizle" }, false).clicked() { self.gizli[i] = !self.gizli[i]; }
                });
                d |= metin_alani(ui, m, "Modeller (virgülle, en fazla 2)", false);
            });
        }
        if d { self.kaydet(); }
    }

    fn ses(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Ses", "Cevapların seslendirilmesi: ses, motor sırası ve ElevenLabs hesabı.");
        let mut d = false;
        kart(ui, |ui| {
            baslik(ui, "Gemini sesleri");
            alt_yazi(ui, "Karta dokununca ses denenir ve seçilir.");
            let ogeler: Vec<(String, String)> = GEMINI_SESLER.iter().map(|s| (s.to_string(), s.to_string())).collect();
            if let Some(s) = ses_kartlari(ui, "gs", &ogeler, &self.ayar.gemini_ses.clone()) {
                self.ayar.gemini_ses = s.clone();
                d = true;
                self.onizle("gemini");
            }
        });
        kart(ui, |ui| {
            baslik(ui, "ElevenLabs");
            let mut sk = self.ses_sekme;
            sekmeler(ui, &mut sk, &["Hesap", "Sesler", "Modeller", "Karakter"]);
            self.ses_sekme = sk;
            let anahtar_var = !self.ayar.eleven_anahtar.trim().is_empty();
            match self.ses_sekme {
                0 => {
                    d |= metin_alani(ui, &mut self.ayar.eleven_anahtar, "ElevenLabs API anahtarı", true);
                    ui.horizontal(|ui| {
                        if dugme(ui, "Krediyi yenile", false).clicked() && anahtar_var {
                            let k = self.ayar.eleven_anahtar.clone();
                            self.el_kredi.baslat(&self.ctx, move || ses::eleven_kredi(&k));
                        }
                        if dugme(ui, "Dene", false).clicked() { self.onizle("eleven"); }
                    });
                    self.el_kredi.yokla();
                    if let Some((k, l)) = self.el_kredi.veri {
                        let oran = if l > 0 { k as f32 / l as f32 } else { 0.0 };
                        ui.add(egui::ProgressBar::new(oran).fill(YESIL).desired_height(10.0));
                        alt_yazi(ui, &format!("{} / {} karakter kullanıldı · {} kaldı", k, l, l.saturating_sub(k)));
                    }
                    if !self.el_kredi.hata.is_empty() { alt_yazi(ui, &format!("Kredi alınamadı: {}", self.el_kredi.hata)); }
                }
                1 => {
                    if dugme(ui, "Sesleri getir", false).clicked() && anahtar_var {
                        let k = self.ayar.eleven_anahtar.clone();
                        self.el_sesler.baslat(&self.ctx, move || ses::eleven_sesler(&k));
                    }
                    self.el_sesler.yokla();
                    if self.el_sesler.bekliyor { alt_yazi(ui, "Getiriliyor…"); }
                    if !self.el_sesler.hata.is_empty() { alt_yazi(ui, &format!("Alınamadı: {}", self.el_sesler.hata)); }
                    if let Some(v) = &self.el_sesler.veri {
                        let ogeler: Vec<(String, String)> = v.iter().map(|(i, a)| (i.clone(), a.clone())).collect();
                        if let Some(s) = ses_kartlari(ui, "es", &ogeler, &self.ayar.eleven_ses.clone()) {
                            self.ayar.eleven_ses = s;
                            d = true;
                            self.onizle("eleven");
                        }
                    }
                    d |= metin_alani(ui, &mut self.ayar.eleven_ses, "Voice ID (elle girilebilir)", false);
                }
                2 => {
                    if dugme(ui, "Modelleri getir", false).clicked() && anahtar_var {
                        let k = self.ayar.eleven_anahtar.clone();
                        self.el_modeller.baslat(&self.ctx, move || ses::eleven_modeller(&k));
                    }
                    self.el_modeller.yokla();
                    if !self.el_modeller.hata.is_empty() { alt_yazi(ui, &format!("Alınamadı: {}", self.el_modeller.hata)); }
                    if let Some(v) = &self.el_modeller.veri {
                        for (id, ad) in v.clone() {
                            if ui.selectable_label(self.ayar.eleven_model == id, format!("{ad}  ·  {id}")).clicked() { self.ayar.eleven_model = id; d = true; }
                        }
                    }
                    d |= metin_alani(ui, &mut self.ayar.eleven_model, "Model (örn. eleven_flash_v2_5)", false);
                }
                _ => {
                    d |= ui.add(egui::Slider::new(&mut self.ayar.eleven_kararlilik, 0.0..=1.0).text("Kararlılık")).changed();
                    d |= ui.add(egui::Slider::new(&mut self.ayar.eleven_benzerlik, 0.0..=1.0).text("Benzerlik")).changed();
                }
            }
        });
        kart(ui, |ui| {
            baslik(ui, "Motor sırası");
            alt_yazi(ui, "Cevap bu sırayla seslendirilir; bir motor kota hatası verirse 60 sn beklemeye alınır.");
            let n = self.ayar.ses_sirasi.len();
            let mut tasi: Option<(usize, usize)> = None;
            for i in 0..n {
                ui.horizontal(|ui| {
                    let m = self.ayar.ses_sirasi[i].clone();
                    ui.label(RichText::new(format!("{}. {}", i + 1, if m == "gemini" { "Gemini" } else { "ElevenLabs" })).size(15.0));
                    if dugme(ui, "Dene", false).clicked() { self.onizle(&m); }
                    if i > 0 && dugme(ui, "Yukarı", false).clicked() { tasi = Some((i, i - 1)); }
                    if i + 1 < n && dugme(ui, "Aşağı", false).clicked() { tasi = Some((i, i + 1)); }
                });
            }
            if let Some((a, b)) = tasi { self.ayar.ses_sirasi.swap(a, b); d = true; }
            ui.add_space(6.0);
            d |= satir_anahtar(ui, "Sırayla dene", "Bir motor çalışmazsa sıradakine geç.", &mut self.ayar.ses_sirayla);
            d |= satir_anahtar(ui, "Sadece sesli komutlarda sesli cevap ver", "Yazılı mesajlarda ses üretilmez, kota korunur.", &mut self.ayar.ses_sadece_sesli);
        });
        if d { self.kaydet(); }
    }

    fn onizle(&self, motor: &str) {
        let mut a = self.ayar.clone();
        a.ses_sirasi = vec![motor.to_string()];
        self.oynatici.konus("Merhaba, ben E.D.I.T.H. Sesim böyle.", &a);
    }

    fn uyandirma(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Uyandırma", "Asistanı sesle çağırma kelimeleri ve davranışı.");
        self.dinleyici_baslat();
        let mut d = false;
        kart(ui, |ui| {
            baslik(ui, "Kelimeler");
            ui.horizontal(|ui| {
                for k in ["hey edit", "edit"] {
                    egui::Frame::none().fill(VISNE_KOYU).stroke(Stroke::new(1.0, YUZEY_KENAR)).rounding(Rounding::same(20.0)).inner_margin(Margin::symmetric(14.0, 6.0)).show(ui, |ui| {
                        ui.label(RichText::new(k).font(FontId::new(16.0, serif())));
                    });
                }
            });
            alt_yazi(ui, "Telaffuz varyasyonları arka planda tutulur.");
        });
        kart(ui, |ui| {
            baslik(ui, "Öğrenme");
            alt_yazi(ui, "Öğren'e basıp uyandırma kelimesini 8 kez söyle; tanımanın senin telaffuzunu öğrenir.");
            ui.horizontal(|ui| {
                let hazir = self.dinleyici.is_some() && self.dinle_hazir;
                ui.add_enabled_ui(hazir, |ui| {
                    if !self.ogren_acik {
                        if dugme(ui, "Öğren", true).clicked() { self.ogren_acik = true; self.ogren_n = 0; self.komut(DKomut::Ogren(true)); }
                    } else if dugme(ui, "Durdur", false).clicked() { self.ogren_acik = false; self.komut(DKomut::Ogren(false)); }
                });
                if dugme(ui, "Sıfırla", false).clicked() { self.ayar.uyandirma_ekstra.clear(); d = true; }
            });
            if self.ogren_acik { ui.add(egui::ProgressBar::new(self.ogren_n as f32 / 8.0).fill(YESIL).text(format!("{}/8", self.ogren_n))); }
            if !self.ayar.uyandirma_ekstra.is_empty() { alt_yazi(ui, &format!("{} öğrenilmiş varyasyon kayıtlı.", self.ayar.uyandirma_ekstra.len())); }
            if self.dinleyici.is_none() { alt_yazi(ui, "Önce Mikrofon bölümünden konuşma modellerini indir."); }
            if !self.dinle_hata.is_empty() { alt_yazi(ui, &self.dinle_hata); }
        });
        kart(ui, |ui| {
            d |= satir_anahtar(ui, "Cümle başında olsun", "Kelime yalnızca cümlenin başında duyulursa uyan.", &mut self.ayar.cumle_basi);
            d |= satir_anahtar(ui, "Sesle kes", "Asistan konuşurken uyandırma kelimesi onu susturur.", &mut self.ayar.sesle_kes);
            d |= satir_anahtar(ui, "Hoparlör modu", "Asistan konuşurken yalnızca tam algılanan kelimeyle uyan (yankıyı azaltır).", &mut self.ayar.hoparlor_modu);
        });
        if d { self.kaydet(); }
    }

    fn mikrofon(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Mikrofon", "Dinleme testi, takip süreleri ve konuşma modelleri.");
        self.dinleyici_baslat();
        let mut d = false;
        kart(ui, |ui| {
            baslik(ui, "Mikrofon testi");
            let hazir = self.dinleyici.is_some() && self.dinle_hazir;
            ui.add_enabled_ui(hazir, |ui| {
                if !self.test_acik {
                    if dugme(ui, "Testi başlat", true).clicked() { self.test_acik = true; self.komut(DKomut::Test(true)); }
                } else if dugme(ui, "Testi durdur", false).clicked() { self.test_acik = false; self.komut(DKomut::Test(false)); self.seviye = 0.0; }
            });
            ui.add(egui::ProgressBar::new(self.seviye.min(1.0)).fill(YESIL).desired_height(10.0));
            alt_yazi(ui, "İngilizce (uyandırma modeli)");
            ui.label(RichText::new(if self.test_en.is_empty() { "…" } else { &self.test_en }).font(FontId::new(16.0, serif())));
            alt_yazi(ui, "Türkçe (komut modeli)");
            ui.label(RichText::new(if self.test_tr.is_empty() { "…" } else { &self.test_tr }).font(FontId::new(16.0, serif())));
            if !self.dinle_hata.is_empty() { ui.label(RichText::new(&self.dinle_hata).size(12.5).color(HATA_ACIK)); }
        });
        kart(ui, |ui| {
            d |= satir_anahtar(ui, "Soru sorunca mikrofon", "Cevap bir soru içeriyorsa mikrofon kendiliğinden açılır.", &mut self.ayar.soru_sorunca_mikrofon);
            d |= satir_anahtar(ui, "Her cevaptan sonra mikrofon", "Soru olsun olmasın cevaptan sonra dinlemeye geç.", &mut self.ayar.her_cevaptan_sonra);
            d |= satir_anahtar(ui, "Sessizlik kapısı", "Gürültüyü eleyip yalnızca konuşmayı tanımaya verir.", &mut self.ayar.sessizlik_kapisi);
            ui.add_space(4.0);
            ui.label("Takip zaman aşımı");
            d |= ui.add(egui::Slider::new(&mut self.ayar.takip_s, 3.0..=20.0).suffix(" sn")).changed();
            ui.label("Sessizlik süresi");
            d |= ui.add(egui::Slider::new(&mut self.ayar.sessizlik_s, 0.5..=3.0).suffix(" sn")).changed();
        });
        kart(ui, |ui| {
            baslik(ui, "Konuşma modelleri");
            let d1 = dinleme::modeller_dizini();
            let tr = d1.join(dinleme::TR_MODEL).exists();
            let en = d1.join(dinleme::EN_MODEL).exists();
            let dll = std::env::current_exe().ok().and_then(|p| p.parent().map(|x| x.join("libvosk.dll").exists())).unwrap_or(false);
            let isaret = |b: bool| if b { "hazır" } else { "yok" };
            alt_yazi(ui, &format!("Türkçe: {} · İngilizce: {} · Vosk kütüphanesi: {}", isaret(tr), isaret(en), isaret(dll)));
            if !(tr && en) && self.kurulum.is_none() && dugme(ui, "İndir", true).clicked() {
                let c = self.ctx.clone();
                self.kurulum = Some(kurulum::basla(Arc::new(move || c.request_repaint())));
            }
            if let Some(k) = &self.kurulum {
                let b = k.binde.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
                ui.add(egui::ProgressBar::new(b).fill(YESIL).desired_height(10.0).text(format!("%{}", (b * 100.0) as u32)));
                let h = k.hata.lock().unwrap().clone();
                if !h.is_empty() { alt_yazi(ui, &h); }
            }
        });
        if self.kurulum.as_ref().map(|k| k.bitti.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(false) {
            self.kurulum = None;
            self.dinleyici_denendi = false;
            self.dinleyici_baslat();
        }
        if d { self.kaydet(); }
    }

    fn baglantilar(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Bağlantılar", "MCP sunucuları: eklediğin araçları asistan kullanabilir.");
        let durum: Vec<(bool, Vec<String>, String)> = match self.araclar.try_lock() {
            Ok(k) => k.iter().map(|m| (m.bagli, m.araclar.iter().map(|a| a.ad.clone()).collect(), m.hata.clone())).collect(),
            Err(_) => vec![],
        };
        let mut sil: Option<usize> = None;
        let mut yeniden = false;
        for i in 0..self.ayar.mcp.len() {
            kart(ui, |ui| {
                let (bagli, adlar, hata) = durum.get(i).cloned().unwrap_or((false, vec![], String::new()));
                ui.horizontal(|ui| {
                    baslik(ui, &self.ayar.mcp[i].ad.clone());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (t, c) = if bagli { (format!("bağlı · {} araç", adlar.len()), YESIL_ACIK) } else if hata.is_empty() { ("bağlanıyor…".to_string(), SOLUK_KREM) } else { ("bağlanamadı".to_string(), HATA_ACIK) };
                        ui.label(RichText::new(t).size(12.5).color(c));
                    });
                });
                alt_yazi(ui, &self.ayar.mcp[i].url.clone());
                ui.horizontal(|ui| {
                    if dugme(ui, "Düzenle", false).clicked() { self.mcp_duzen = Some((Some(i), self.ayar.mcp[i].clone())); }
                    if dugme(ui, "Yeniden bağlan", false).clicked() { yeniden = true; }
                    if dugme(ui, "Sil", false).clicked() { sil = Some(i); }
                });
                if !adlar.is_empty() {
                    egui::CollapsingHeader::new("Araçlar").id_salt(("araclar", i)).show(ui, |ui| {
                        for a in &adlar {
                            let mut acik = !self.ayar.mcp[i].kapali.contains(a);
                            if satir_anahtar(ui, a, "", &mut acik) {
                                self.ayar.mcp[i].kapali.retain(|x| x != a);
                                if !acik { self.ayar.mcp[i].kapali.push(a.clone()); }
                                self.ayar.kaydet();
                            }
                        }
                    });
                }
            });
        }
        if let Some(i) = sil { self.ayar.mcp.remove(i); self.ayar.kaydet(); yeniden = true; }
        if yeniden { crate::mcp::yenile(&self.araclar, &self.ayar.mcp); }
        if self.mcp_duzen.is_none() {
            if dugme(ui, "+ Bağlantı ekle", true).clicked() { self.mcp_duzen = Some((None, McpAyar::default())); }
        }
        let mut bitir: Option<bool> = None;
        let mut dene = false;
        if let Some((_, m)) = &mut self.mcp_duzen {
            kart(ui, |ui| {
                baslik(ui, "Bağlantı");
                metin_alani(ui, &mut m.ad, "Ad", false);
                metin_alani(ui, &mut m.url, "URL", false);
                metin_alani(ui, &mut m.baslik, "Başlık: Authorization: Bearer …", false);
                ui.horizontal(|ui| {
                    if dugme(ui, "Kaydet", true).clicked() { bitir = Some(true); }
                    if dugme(ui, "Bağlantıyı dene", false).clicked() { dene = true; }
                    if dugme(ui, "Vazgeç", false).clicked() { bitir = Some(false); }
                });
            });
        }
        if dene {
            if let Some((_, m)) = &self.mcp_duzen {
                let (ad, url, b) = (m.ad.clone(), m.url.clone(), m.baslik.clone());
                self.mcp_test.baslat(&self.ctx, move || {
                    let mut c = Mcp::yeni(&ad, &url, &b);
                    c.baglan().map(|_| format!("Bağlandı · {} araç", c.araclar.len()))
                });
            }
        }
        self.mcp_test.yokla();
        if self.mcp_test.bekliyor { alt_yazi(ui, "Deneniyor…"); }
        if let Some(v) = &self.mcp_test.veri { if self.mcp_duzen.is_some() { alt_yazi(ui, v); } }
        if !self.mcp_test.hata.is_empty() && self.mcp_duzen.is_some() { ui.label(RichText::new(&self.mcp_test.hata).size(12.5).color(HATA_ACIK)); }
        if let Some(k) = bitir {
            if let (true, Some((idx, m))) = (k, self.mcp_duzen.take()) {
                match idx { Some(i) => self.ayar.mcp[i] = m, None => self.ayar.mcp.push(m) }
                self.ayar.kaydet();
                crate::mcp::yenile(&self.araclar, &self.ayar.mcp);
            } else { self.mcp_duzen = None; }
            self.mcp_test.veri = None;
            self.mcp_test.hata.clear();
        }
    }

    fn sohbetler(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Sohbetler", "Kayıtlı sohbetlerin adını düzenle veya sil.");
        if dugme(ui, "Yenile", false).clicked() { self.sohbetler = depo::hepsi(); }
        ui.add_space(8.0);
        if self.sohbetler.is_empty() { alt_yazi(ui, "Kayıtlı sohbet yok."); }
        let mut kaydet_ad: Option<(String, String)> = None;
        let mut sil: Option<String> = None;
        for s in self.sohbetler.clone() {
            kart(ui, |ui| {
                let duzen = self.ad_duzen.as_ref().map(|(id, _)| *id == s.id).unwrap_or(false);
                if duzen {
                    if let Some((_, m)) = &mut self.ad_duzen { metin_alani(ui, m, "Sohbet adı", false); }
                    ui.horizontal(|ui| {
                        if dugme(ui, "Kaydet", true).clicked() { kaydet_ad = self.ad_duzen.clone(); }
                        if dugme(ui, "Vazgeç", false).clicked() { self.ad_duzen = None; }
                    });
                } else if self.sil_onay.as_deref() == Some(&s.id) {
                    ui.label(format!("\"{}\" silinsin mi?", s.baslik));
                    ui.horizontal(|ui| {
                        if dugme(ui, "Evet, sil", true).clicked() { sil = Some(s.id.clone()); }
                        if dugme(ui, "Vazgeç", false).clicked() { self.sil_onay = None; }
                    });
                } else {
                    ui.label(RichText::new(&s.baslik).font(FontId::new(17.0, serif())));
                    let tarih = chrono::DateTime::from_timestamp_millis(s.guncelleme).map(|d| d.with_timezone(&chrono::Local).format("%d.%m.%Y %H:%M").to_string()).unwrap_or_default();
                    alt_yazi(ui, &format!("{} · {} mesaj · {}", tarih, s.mesajlar.len(), s.model.replace('|', " · ")));
                    ui.horizontal(|ui| {
                        if dugme(ui, "Adı düzenle", false).clicked() { self.ad_duzen = Some((s.id.clone(), s.baslik.clone())); }
                        if dugme(ui, "Sil", false).clicked() { self.sil_onay = Some(s.id.clone()); }
                    });
                }
            });
        }
        if let Some((id, ad)) = kaydet_ad {
            if let Some(s) = self.sohbetler.iter_mut().find(|s| s.id == id) {
                if !ad.trim().is_empty() { s.baslik = ad.trim().to_string(); }
                s.yaz();
            }
            self.ad_duzen = None;
        }
        if let Some(id) = sil {
            if let Some(s) = self.sohbetler.iter().find(|s| s.id == id) { s.sil(); }
            self.sohbetler = depo::hepsi();
            self.sil_onay = None;
        }
    }

    fn izinler(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "İzinler", "Uygulamanın ihtiyaç duyduğu erişimler.");
        self.dinleyici_baslat();
        kart(ui, |ui| {
            baslik(ui, "Mikrofon");
            let (t, c) = if self.dinle_hazir { ("Erişim var, mikrofon açık.", YESIL_ACIK) } else if !self.dinle_hata.is_empty() { ("Mikrofon açılamadı.", HATA_ACIK) } else { ("Henüz kontrol edilmedi.", SOLUK_KREM) };
            ui.label(RichText::new(t).color(c));
            if !self.dinle_hata.is_empty() { alt_yazi(ui, &self.dinle_hata); }
            if dugme(ui, "Windows mikrofon ayarlarını aç", false).clicked() {
                let _ = std::process::Command::new("cmd").args(["/C", "start", "", "ms-settings:privacy-microphone"]).spawn();
            }
        });
        kart(ui, |ui| {
            baslik(ui, "Ağ");
            alt_yazi(ui, "Yapay zeka, ses ve MCP istekleri için internet gerekir. Konuşma tanıma tamamen bu bilgisayarda çalışır; uyandırma kelimesinden önceki konuşma hiçbir yere gönderilmez.");
        });
    }

    fn guncelleme(&mut self, ui: &mut Ui) {
        sayfa_basligi(ui, "Güncelleme", "Yeni sürümleri GitHub'dan kontrol edip yükler.");
        self.kontrol.yokla();
        self.kur.yokla();
        kart(ui, |ui| {
            baslik(ui, &format!("Sürüm {}", guncelleme::surum_metni()));
            if dugme(ui, "Güncellemeleri kontrol et", true).clicked() { self.kontrol.veri = None; self.kontrol.baslat(&self.ctx, guncelleme::kontrol); }
            if self.kontrol.bekliyor { alt_yazi(ui, "Kontrol ediliyor…"); }
            if !self.kontrol.hata.is_empty() { ui.label(RichText::new(&self.kontrol.hata).size(12.5).color(HATA_ACIK)); }
            match self.kontrol.veri.clone() {
                Some(Some((n, url))) => {
                    ui.label(format!("Yeni sürüm var: 0.4.{n}"));
                    if !self.kur.bekliyor && dugme(ui, "Yükle ve yeniden başlat", true).clicked() {
                        self.kur.baslat(&self.ctx, move || guncelleme::kur(&url));
                    }
                }
                Some(None) => alt_yazi(ui, "Güncelsin."),
                None => {}
            }
            if self.kur.bekliyor { alt_yazi(ui, "İndiriliyor…"); }
            if !self.kur.hata.is_empty() { ui.label(RichText::new(&self.kur.hata).size(12.5).color(HATA_ACIK)); }
        });
        if self.kur.veri.is_some() { std::process::exit(42); }
    }

    fn kategori_karti(&mut self, ui: &mut Ui, i: usize) -> bool {
        let (r, y) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());
        let sec = self.sayfa == i;
        ui.painter().rect(r, Rounding::same(16.0), if sec { YUZEY_ACIK } else if y.hovered() { YUZEY_ACIK.gamma_multiply(0.85) } else { YUZEY }, Stroke::new(if sec { 1.6 } else { 1.0 }, if sec { KREM } else { YUZEY_KENAR }));
        ui.painter().text(r.left_top() + Vec2::new(16.0, 11.0), Align2::LEFT_TOP, KATEGORILER[i].0, FontId::new(16.5, serif()), METIN_KREM);
        ui.painter().text(r.left_top() + Vec2::new(16.0, 34.0), Align2::LEFT_TOP, KATEGORILER[i].1, FontId::proportional(12.0), SOLUK_KREM);
        ui.add_space(6.0);
        y.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }

    fn sayfa_ciz(&mut self, ui: &mut Ui) {
        match self.sayfa {
            0 => self.yapay_zeka(ui),
            1 => self.ses(ui),
            2 => self.uyandirma(ui),
            3 => self.mikrofon(ui),
            4 => self.baglantilar(ui),
            5 => self.sohbetler(ui),
            6 => self.izinler(ui),
            _ => self.guncelleme(ui),
        }
    }
}

impl eframe::App for AyarUyg {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        VISNE.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.olaylar();
        if self.test_acik || self.ogren_acik { ctx.request_repaint_after(std::time::Duration::from_millis(60)); }
        let genis = ctx.screen_rect().width() >= 720.0;
        let cerceve = egui::Frame::none().fill(VISNE).inner_margin(Margin::same(18.0));
        if genis {
            egui::SidePanel::left("kat").exact_width(270.0).resizable(false).frame(egui::Frame::none().fill(VISNE_KOYU).inner_margin(Margin::same(14.0))).show(ctx, |ui| {
                ui.label(RichText::new("Ayarlar").font(FontId::new(24.0, serif())).color(KREM));
                ui.add_space(10.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..KATEGORILER.len() { if self.kategori_karti(ui, i) { self.sayfa = i; } }
                });
            });
            egui::CentralPanel::default().frame(cerceve).show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(760.0);
                    self.sayfa_ciz(ui);
                });
            });
        } else {
            egui::CentralPanel::default().frame(cerceve).show(ctx, |ui| {
                if !self.sayfa_acik {
                    ui.label(RichText::new("Ayarlar").font(FontId::new(28.0, serif())).color(KREM));
                    ui.add_space(10.0);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for i in 0..KATEGORILER.len() { if self.kategori_karti(ui, i) { self.sayfa = i; self.sayfa_acik = true; } }
                    });
                } else {
                    if dugme(ui, "‹ Geri", false).clicked() { self.sayfa_acik = false; }
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.sayfa_ciz(ui));
                }
            });
        }
    }
}
