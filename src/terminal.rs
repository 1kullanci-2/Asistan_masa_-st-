//! Terminal arayüzü: banner, sohbet, komutlar, sesli dinleme.
use crate::ai::{self, Araclar, Olay};
use crate::ayarlar::{servis_adi, Ayar};
use crate::depo::{self, Mesaj, Sohbet};
use crate::dinleme::{self, DEvent, DKomut, Dinleyici};
use crate::ses::Oynatici;
use crate::{guncelleme, kurulum, mcp};
use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TURUNCU: &str = "\x1b[38;5;209m";
const MAVI: &str = "\x1b[38;5;110m";
const KIRMIZI: &str = "\x1b[38;5;203m";
const YESIL: &str = "\x1b[38;5;78m";
const GRI: &str = "\x1b[90m";
const SIFIRLA: &str = "\x1b[0m";
const SIL: &str = "\r\x1b[K";

#[cfg(windows)]
pub fn vt_ac() {
    extern "system" {
        fn GetStdHandle(n: i32) -> isize;
        fn GetConsoleMode(h: isize, m: *mut u32) -> i32;
        fn SetConsoleMode(h: isize, m: u32) -> i32;
        fn SetConsoleOutputCP(cp: u32) -> i32;
        fn SetConsoleCP(cp: u32) -> i32;
    }
    unsafe {
        SetConsoleOutputCP(65001);
        SetConsoleCP(65001);
        let h = GetStdHandle(-11);
        let mut m = 0u32;
        if GetConsoleMode(h, &mut m) != 0 { SetConsoleMode(h, m | 4); }
    }
}
#[cfg(not(windows))]
pub fn vt_ac() {}

fn yaz(s: &str) {
    print!("{s}");
    let _ = std::io::stdout().flush();
}

fn banner() {
    const E: [&str; 5] = ["█████", "█    ", "████ ", "█    ", "█████"];
    const D: [&str; 5] = ["████ ", "█   █", "█   █", "█   █", "████ "];
    const I: [&str; 5] = ["███", " █ ", " █ ", " █ ", "███"];
    const T: [&str; 5] = ["█████", "  █  ", "  █  ", "  █  ", "  █  "];
    const H: [&str; 5] = ["█   █", "█   █", "█████", "█   █", "█   █"];
    const N: [&str; 5] = [" ", " ", " ", " ", "█"];
    let renk = [215, 214, 209, 208, 203];
    println!();
    for r in 0..5 {
        let mut s = String::from("  ");
        for (k, g) in [E, N, D, N, I, N, T, N, H].iter().enumerate() {
            s.push_str(g[r]);
            if k < 8 { s.push(' '); }
        }
        println!("\x1b[38;5;{}m{s}{SIFIRLA}", renk[r]);
    }
    println!("\n  {GRI}ayarlar · /model · /history_chat · /yeni · /yardim · çıkış{SIFIRLA}\n");
}

enum Girdi {
    Satir(String),
    Ai(Olay),
    Din(DEvent),
    Kapat,
}

enum Liste {
    Model(Vec<String>),
    Sohbet(Vec<String>),
}

struct T {
    ayar: Ayar,
    aktif: Sohbet,
    gtx: Sender<Girdi>,
    araclar: Araclar,
    oynatici: Arc<Oynatici>,
    dinleyici: Option<Dinleyici>,
    liste: Option<Liste>,
    mesgul: bool,
    akan: String,
    arac: String,
    hata: bool,
    sesli: bool,
    sonra_dinle: bool,
    dinliyor: bool,
    kare: usize,
    kuyruk: VecDeque<String>,
    cocuk: Option<std::process::Child>,
}

fn ayar_komutu_mu(m: &str) -> bool {
    let k: Vec<&str> = m.split_whitespace().collect();
    !k.is_empty() && k.len() <= 3 && k[0].to_lowercase().starts_with("ayar")
}

fn tarih(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(&chrono::Local).format("%d.%m %H:%M").to_string()).unwrap_or_default()
}

impl T {
    fn istem(&self) {
        if !self.mesgul && !self.dinliyor { yaz(&format!("{GRI}› {SIFIRLA}")); }
    }

    fn not(&self, m: &str) {
        yaz(&format!("{SIL}{GRI}{m}{SIFIRLA}\n"));
    }

    fn model_adi(&self) -> String {
        self.aktif.model.split_once('|').map(|(s, m)| format!("{} · {}", servis_adi(s), m)).unwrap_or_else(|| "model yok".into())
    }

    fn yeni(&mut self) {
        self.aktif = Sohbet::yeni(self.ayar.varsayilan_model());
        self.not(&format!("Yeni sohbet · {}", self.model_adi()));
    }

    fn yardim(&self) {
        yaz(&format!("{SIL}{GRI}  ayarlar          ayarlar penceresini açar\n  /model           modelleri numaralı listeler\n  /history_chat    eski sohbetleri numaralı listeler\n  /yeni            yeni sohbet\n  çıkış            kapatır\n  sesle: \"hey edit\" / \"edit\" deyip konuş{SIFIRLA}\n"));
    }

    fn ayar_ac(&mut self) {
        if let Some(c) = &mut self.cocuk {
            if c.try_wait().ok().flatten().is_none() { self.not("Ayarlar zaten açık."); return; }
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let mut k = std::process::Command::new(exe);
        k.arg("--ayarlar");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            k.creation_flags(0x00000008);
        }
        match k.spawn() {
            Ok(c) => { self.cocuk = Some(c); self.not("Ayarlar açılıyor…"); }
            Err(e) => self.not(&format!("Ayarlar açılamadı: {e}")),
        }
    }

    fn ayar_kapandi(&mut self) {
        self.ayar = Ayar::yukle();
        if let Some(d) = &self.dinleyici { let _ = d.komut.send(DKomut::Ayar(self.ayar.dinleme_ayari())); }
        mcp::yenile(&self.araclar, &self.ayar.mcp);
        let yol = crate::ayarlar::klasor().join("sohbetler").join(format!("{}.json", self.aktif.id));
        if !self.aktif.mesajlar.is_empty() {
            match std::fs::read_to_string(&yol).ok().and_then(|s| serde_json::from_str::<Sohbet>(&s).ok()) {
                Some(d) => { self.aktif.baslik = d.baslik; }
                None => { self.aktif = Sohbet::yeni(self.ayar.varsayilan_model()); }
            }
        }
        if !self.ayar.secenekler().contains(&self.aktif.model) && self.aktif.mesajlar.is_empty() { self.aktif.model = self.ayar.varsayilan_model(); }
        self.not("Ayarlar kaydedildi.");
        self.istem();
    }

    fn model_listele(&mut self) {
        let sec = self.ayar.secenekler();
        if sec.is_empty() { self.not("Henüz API anahtarı yok. `ayarlar` > Yapay zeka bölümünden ekle."); return; }
        yaz(&format!("{SIL}"));
        for (i, s) in sec.iter().enumerate() {
            let (sv, m) = s.split_once('|').unwrap_or(("", ""));
            let isaret = if *s == self.aktif.model { format!(" {YESIL}←{SIFIRLA}") } else { String::new() };
            println!("  {GRI}{}.{SIFIRLA} {} · {m}{isaret}", i + 1, servis_adi(sv));
        }
        println!("  {GRI}Numara yaz (vazgeçmek için başka bir şey yaz){SIFIRLA}");
        self.liste = Some(Liste::Model(sec));
    }

    fn sohbet_listele(&mut self) {
        let hepsi = depo::hepsi();
        if hepsi.is_empty() { self.not("Kayıtlı sohbet yok."); return; }
        yaz(SIL);
        let mut ids = vec![];
        for (i, s) in hepsi.iter().enumerate() {
            let isaret = if s.id == self.aktif.id { format!(" {YESIL}←{SIFIRLA}") } else { String::new() };
            println!("  {GRI}{}.{SIFIRLA} {} {GRI}{}{SIFIRLA}{isaret}", i + 1, s.baslik, tarih(s.guncelleme));
            ids.push(s.id.clone());
        }
        println!("  {GRI}Numara yaz (vazgeçmek için başka bir şey yaz){SIFIRLA}");
        self.liste = Some(Liste::Sohbet(ids));
    }

    fn liste_sec(&mut self, n: usize) {
        match self.liste.take() {
            Some(Liste::Model(v)) => {
                self.aktif.model = v[n - 1].clone();
                if !self.aktif.mesajlar.is_empty() { self.aktif.kaydet(); }
                self.not(&format!("Bu sohbetin modeli: {}", self.model_adi()));
            }
            Some(Liste::Sohbet(v)) => {
                if let Some(s) = depo::hepsi().into_iter().find(|s| s.id == v[n - 1]) {
                    self.aktif = s;
                    yaz(SIL);
                    println!("  {GRI}{} · {}{SIFIRLA}\n", self.aktif.baslik, self.model_adi());
                    let bas = self.aktif.mesajlar.len().saturating_sub(6);
                    for m in &self.aktif.mesajlar[bas..] {
                        match m.k.as_str() {
                            "k" => println!("{MAVI}› {}{SIFIRLA}\n", m.m),
                            "n" => println!("{GRI}{}{SIFIRLA}\n", m.m),
                            _ => println!("{TURUNCU}{}{SIFIRLA}\n", m.m),
                        }
                    }
                }
            }
            None => {}
        }
    }

    fn satir(&mut self, s: &str) {
        let s = s.trim().to_string();
        if let Some(l) = &self.liste {
            let adet = match l { Liste::Model(v) => v.len(), Liste::Sohbet(v) => v.len() };
            if let Ok(n) = s.parse::<usize>() {
                if n >= 1 && n <= adet { self.liste_sec(n); self.istem(); return; }
            }
            self.liste = None;
        }
        if self.mesgul { self.kuyruk.push_back(s); return; }
        let k = s.to_lowercase();
        match k.as_str() {
            "" => {}
            "çıkış" | "cikis" | "exit" | "/exit" | "/cikis" => { std::process::exit(0); }
            "/model" => self.model_listele(),
            "/history_chat" => self.sohbet_listele(),
            "/yeni" => self.yeni(),
            "/yardim" | "/yardım" | "/help" => self.yardim(),
            _ if k == "/ayarlar" || ayar_komutu_mu(&k) => self.ayar_ac(),
            _ => { self.gonder(&s, false); return; }
        }
        self.istem();
    }

    fn gonder(&mut self, metin: &str, sesli: bool) {
        let Some((servis, model)) = self.aktif.model.split_once('|').map(|(a, b)| (a.to_string(), b.to_string())) else {
            self.not("Önce `ayarlar` > Yapay zeka bölümünden bir API anahtarı ekle.");
            self.istem();
            return;
        };
        yaz(&format!("{}{MAVI}› {metin}{SIFIRLA}\n", if sesli { SIL } else { "\x1b[F\x1b[K" }));
        self.aktif.mesajlar.push(Mesaj { k: "k".into(), m: metin.to_string() });
        depo::ozetle(&mut self.aktif);
        let gecmis = depo::baglam(&self.aktif);
        self.aktif.kaydet();
        let anahtar = self.ayar.anahtar(&servis).to_string();
        let (tx, rx) = channel();
        let gtx = self.gtx.clone();
        std::thread::spawn(move || {
            for o in rx { if gtx.send(Girdi::Ai(o)).is_err() { break; } }
        });
        let araclar = self.araclar.clone();
        let kapali: Vec<Vec<String>> = self.ayar.mcp.iter().map(|m| m.kapali.clone()).collect();
        std::thread::spawn(move || ai::akis(&servis, &model, &anahtar, gecmis, araclar, kapali, tx, || {}));
        self.mesgul = true;
        self.sesli = sesli;
        self.akan.clear();
        self.arac.clear();
        self.hata = false;
    }

    fn ai_olay(&mut self, o: Olay) {
        match o {
            Olay::Parca(t) => { self.arac.clear(); self.akan.push_str(&t); }
            Olay::Arac(a) => { yaz(&format!("{SIL}{GRI}  araç: {a}{SIFIRLA}\n")); self.arac = a; }
            Olay::Hata(h) => {
                self.aktif.mesajlar.push(Mesaj { k: "n".into(), m: h.clone() });
                yaz(&format!("{SIL}{KIRMIZI}{h}{SIFIRLA}\n\n"));
                self.hata = true;
                self.bitir();
            }
            Olay::Bitti => self.bitir(),
        }
    }

    fn bitir(&mut self) {
        if !self.mesgul { return; }
        self.mesgul = false;
        yaz(SIL);
        let mut soru = false;
        if !self.akan.is_empty() {
            let (temiz, baslik, s) = ai::isaretleri_ayir(&self.akan);
            soru = s;
            if let Some(b) = baslik {
                if self.aktif.baslik == "Yeni sohbet" && !b.is_empty() { self.aktif.baslik = b; }
            }
            println!("{TURUNCU}{temiz}{SIFIRLA}\n");
            if !self.ayar.ses_sadece_sesli || self.sesli { self.oynatici.konus(&temiz, &self.ayar); }
            self.aktif.mesajlar.push(Mesaj { k: "a".into(), m: temiz });
        }
        self.aktif.kaydet();
        self.akan.clear();
        if self.sesli && !self.hata && self.dinleyici.is_some() && ((soru && self.ayar.soru_sorunca_mikrofon) || self.ayar.her_cevaptan_sonra) {
            self.sonra_dinle = true;
        }
        self.sesli = false;
        self.istem();
        if let Some(s) = self.kuyruk.pop_front() { self.satir(&s); }
    }

    fn dinle_olay(&mut self, e: DEvent) {
        match e {
            DEvent::Hazir => self.not("Dinleme hazır · \"hey edit\" de"),
            DEvent::Hata(h) => self.not(&format!("Sesli dinleme kapalı: {h}")),
            DEvent::Uyandi => {
                self.oynatici.kes();
                self.sonra_dinle = false;
                self.dinliyor = true;
                yaz(&format!("{SIL}{YESIL}Dinleniyor{SIFIRLA}"));
            }
            DEvent::Kismi(t) => { if self.dinliyor { yaz(&format!("{SIL}{YESIL}Dinleniyor{SIFIRLA} {GRI}{t}{SIFIRLA}")); } }
            DEvent::Son(t) => {
                self.dinliyor = false;
                if ayar_komutu_mu(&t) { yaz(SIL); self.ayar_ac(); self.istem(); }
                else if self.mesgul { self.not("Cevap bekleniyor, komut atlandı."); }
                else { self.gonder(&t, true); }
            }
            DEvent::Zaman => { self.dinliyor = false; yaz(SIL); self.istem(); }
            _ => {}
        }
    }

    fn tik(&mut self) {
        self.kare += 1;
        if self.mesgul && self.akan.is_empty() {
            let n = (self.kare / 3) % 4;
            let yazi = if self.arac.is_empty() { "Düşünüyor".to_string() } else { format!("Araç çalışıyor · {}", self.arac) };
            yaz(&format!("{SIL}{GRI}{yazi}{}{SIFIRLA}", ".".repeat(n)));
        }
        if self.sonra_dinle && !self.oynatici.caliyor() && !self.mesgul {
            self.sonra_dinle = false;
            if let Some(d) = &self.dinleyici {
                let _ = d.komut.send(DKomut::Dinle);
                self.dinliyor = true;
                yaz(&format!("{SIL}{YESIL}Dinleniyor{SIFIRLA}"));
            }
        }
        let durum = match &mut self.cocuk { Some(c) => c.try_wait().ok().flatten(), None => None };
        if let Some(d) = &durum { if d.code() == Some(42) { std::process::exit(0); } }
        let bitti = durum.is_some();
        if bitti { self.cocuk = None; self.ayar_kapandi(); }
    }
}

fn kurulum_yap() {
    if dinleme::modeller_hazir() { return; }
    println!("  {GRI}İlk kurulum: konuşma modelleri indiriliyor (~75 MB){SIFIRLA}");
    let il = kurulum::basla(Arc::new(|| {}));
    loop {
        let b = il.binde.load(std::sync::atomic::Ordering::Relaxed);
        let dolu = (b as usize * 30) / 1000;
        yaz(&format!("{SIL}  {YESIL}{}{GRI}{}{SIFIRLA} %{}", "█".repeat(dolu), "░".repeat(30 - dolu), b / 10));
        if il.bitti.load(std::sync::atomic::Ordering::Relaxed) { println!("\n"); break; }
        let h = il.hata.lock().unwrap().clone();
        if !h.is_empty() { println!("\n  {KIRMIZI}{h}{SIFIRLA}\n  {GRI}Sesli dinleme olmadan devam ediliyor.{SIFIRLA}\n"); break; }
        std::thread::sleep(Duration::from_millis(150));
    }
}

pub fn calistir() {
    vt_ac();
    yaz("\x1b]0;E.D.I.T.H\x07");
    banner();
    let ayar = Ayar::yukle();
    kurulum_yap();
    let oynatici = Oynatici::yeni();
    let araclar: Araclar = Arc::new(Mutex::new(vec![]));
    mcp::yenile(&araclar, &ayar.mcp);
    let (gtx, grx) = channel::<Girdi>();
    let dinleyici = if dinleme::modeller_hazir() {
        let d = dinleme::baslat(Arc::new(|| {}), ayar.dinleme_ayari(), oynatici.bayrak());
        Some(d)
    } else { None };
    let mut t = T {
        aktif: Sohbet::yeni(ayar.varsayilan_model()), ayar, gtx: gtx.clone(), araclar, oynatici, dinleyici, liste: None, mesgul: false, akan: String::new(), arac: String::new(),
        hata: false, sesli: false, sonra_dinle: false, dinliyor: false, kare: 0, kuyruk: VecDeque::new(), cocuk: None,
    };
    // Dinleyici olaylarını ana döngüye ilet.
    if let Some(d) = t.dinleyici.take() {
        let (komut, olay) = (d.komut, d.olay);
        let g2 = gtx.clone();
        std::thread::spawn(move || {
            for e in olay { if g2.send(Girdi::Din(e)).is_err() { break; } }
        });
        t.dinleyici = Some(Dinleyici { komut, olay: channel().1 });
    }
    // Güncelleme kontrolü (sessiz).
    let g3 = gtx.clone();
    std::thread::spawn(move || {
        if let Ok(Some((n, _))) = guncelleme::kontrol() {
            let _ = g3.send(Girdi::Satir(format!("\u{1}yeni:{n}")));
        }
    });
    let g4 = gtx.clone();
    std::thread::spawn(move || {
        for l in std::io::stdin().lock().lines() {
            match l { Ok(s) => { if g4.send(Girdi::Satir(s)).is_err() { break; } } Err(_) => break }
        }
        let _ = g4.send(Girdi::Kapat);
    });
    if t.ayar.secenekler().is_empty() { t.not("Başlamak için `ayarlar` yaz ve Yapay zeka bölümüne bir API anahtarı ekle."); }
    t.istem();
    loop {
        match grx.recv_timeout(Duration::from_millis(100)) {
            Ok(Girdi::Satir(s)) => {
                if let Some(n) = s.strip_prefix("\u{1}yeni:") {
                    t.not(&format!("Yeni sürüm var (0.4.{n}). `ayarlar` > Güncelleme bölümünden yükleyebilirsin."));
                    t.istem();
                } else { t.satir(&s); }
            }
            Ok(Girdi::Ai(o)) => t.ai_olay(o),
            Ok(Girdi::Din(e)) => t.dinle_olay(e),
            Ok(Girdi::Kapat) => break,
            Err(RecvTimeoutError::Timeout) => t.tik(),
            Err(_) => break,
        }
    }
}
