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
use std::sync::atomic::{AtomicU32, Ordering};
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
    // ANSI Shadow harfleri: E . D . I . T . H
    const E: [&str; 6] = ["███████╗", "██╔════╝", "█████╗  ", "██╔══╝  ", "███████╗", "╚══════╝"];
    const D: [&str; 6] = ["██████╗ ", "██╔══██╗", "██║  ██║", "██║  ██║", "██████╔╝", "╚═════╝ "];
    const N: [&str; 6] = ["   ", "   ", "   ", "   ", "██╗", "╚═╝"];
    const I: [&str; 6] = ["██╗", "██║", "██║", "██║", "██║", "╚═╝"];
    // T: noktalar T'nin boşluklarına yerleştirildi, aralarında boşluk yok
    const T: [&str; 6] = ["████████╗", "╚══██╔══╝", "   ██║   ", "   ██║   ", "██╗██║██╗", "╚═╝╚═╝╚═╝"];
    const B: [&str; 6] = [" ", " ", " ", " ", " ", " "];
    const H: [&str; 6] = ["██╗  ██╗", "██║  ██║", "███████║", "██╔══██║", "██║  ██║", "╚═╝  ╚═╝"];
    let renk = [202, 208, 209, 214, 215, 221];
    println!();
    for r in 0..6 {
        let mut s = String::from("  ");
        for g in [E, N, D, N, I, B, T, B, H] { s.push_str(g[r]); }
        println!("\x1b[38;5;{}m{s}{SIFIRLA}", renk[r]);
    }
    println!("\n  {GRI}sesli asistan  ·  'hey edit' de  ·  komutlar için /yardim  ·  model için /model{SIFIRLA}\n");
}

enum Girdi {
    Kisayol,
    Yeni(u32, String),
    GuncelBitti(Result<(), String>),
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
    kisayol_tid: Arc<AtomicU32>,
    guncel_sor: Option<(u32, String)>,
    guncelliyor: Option<Arc<AtomicU32>>,
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
        self.ekrani_temizle();
        println!("  {GRI}Yeni sohbet · {}{SIFIRLA}\n", self.model_adi());
    }

    fn yardim(&self) {
        yaz(&format!("{SIL}{GRI}  /ayarlar         ayarlar penceresini açar\n  /model           modelleri numaralı listeler\n  /history_chat    eski sohbetleri numaralı listeler\n  /yeni            yeni sohbet\n  /yardim          bu liste\n  /cikis           kapatır\n  sesle: \"hey edit\" / \"edit\" deyip konuş{SIFIRLA}\n"));
    }

    fn ekrani_temizle(&self) {
        yaz("\x1b[2J\x1b[3J\x1b[H");
        banner();
    }

    fn sohbeti_yaz(&self) {
        println!("  {GRI}{} · {}{SIFIRLA}\n", self.aktif.baslik, self.model_adi());
        for m in &self.aktif.mesajlar {
            match m.k.as_str() {
                "k" => println!("{MAVI}› {}{SIFIRLA}\n", m.m),
                "n" => println!("{GRI}{}{SIFIRLA}\n", m.m),
                _ => println!("{TURUNCU}{}{SIFIRLA}\n", m.m),
            }
        }
    }

    fn kisayol_yenile(&mut self) {
        #[cfg(windows)]
        {
            let eski = self.kisayol_tid.swap(0, Ordering::SeqCst);
            if eski != 0 { unsafe { kisayol::PostThreadMessageW(eski, 0x0012, 0, 0); } }
            if self.ayar.kisayol_acik {
                kisayol::baslat(&self.ayar.kisayol, self.gtx.clone(), self.kisayol_tid.clone());
            }
        }
    }

    fn kisayol_basildi(&mut self) {
        let Some(d) = &self.dinleyici else { self.not("Dinleme kapalı, kısayol çalışmaz."); self.istem(); return };
        if self.dinliyor {
            let _ = d.komut.send(DKomut::Bekle);
            self.dinliyor = false;
            yaz(SIL);
            self.istem();
        } else {
            self.oynatici.kes();
            self.sonra_dinle = false;
            let _ = d.komut.send(DKomut::Dinle);
            self.dinliyor = true;
            yaz(&format!("{SIL}{YESIL}Dinleniyor{SIFIRLA}"));
        }
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
        self.kisayol_yenile();
        let gir = crate::ayarlar::klasor().join("gir.txt");
        if let Ok(id) = std::fs::read_to_string(&gir) {
            let _ = std::fs::remove_file(&gir);
            if self.mesgul {
                self.not("Cevap bekleniyor, sohbete girilemedi.");
            } else if let Some(c) = depo::hepsi().into_iter().find(|c| c.id == id.trim()) {
                self.aktif = c;
                self.ekrani_temizle();
                self.sohbeti_yaz();
                self.istem();
                return;
            }
        }
        self.not("Ayarlar kaydedildi.");
        self.istem();
    }

    fn model_listele(&mut self) {
        let sec = self.ayar.secenekler();
        if sec.is_empty() { self.not("Henüz API anahtarı yok. `/ayarlar` > Yapay zeka bölümünden ekle."); return; }
        yaz(SIL);
        let mut onceki = String::new();
        for (i, s) in sec.iter().enumerate() {
            let (sv, m) = s.split_once('|').unwrap_or(("", ""));
            if sv != onceki {
                println!("  {YESIL}{}{SIFIRLA}", servis_adi(sv));
                onceki = sv.to_string();
            }
            let isaret = if *s == self.aktif.model { format!(" {YESIL}←{SIFIRLA}") } else { String::new() };
            println!("    {GRI}{}.{SIFIRLA} {m}{isaret}", i + 1);
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
                let n = format!("Model değişti: {}", self.model_adi());
                if !self.aktif.mesajlar.is_empty() {
                    self.aktif.mesajlar.push(Mesaj { k: "n".into(), m: n.clone() });
                    self.aktif.kaydet();
                }
                self.not(&n);
            }
            Some(Liste::Sohbet(v)) => {
                if let Some(c) = depo::hepsi().into_iter().find(|c| c.id == v[n - 1]) {
                    self.aktif = c;
                    self.ekrani_temizle();
                    self.sohbeti_yaz();
                }
            }
            None => {}
        }
    }

    fn guncelle_basla(&mut self, url: String) {
        let il = Arc::new(AtomicU32::new(0));
        self.guncelliyor = Some(il.clone());
        let gtx = self.gtx.clone();
        std::thread::spawn(move || { let r = guncelleme::kur(&url, il); let _ = gtx.send(Girdi::GuncelBitti(r)); });
    }

    fn sesli_komut(&mut self, t: &str) -> bool {
        let k = t.to_lowercase();
        match k.trim_matches(|c: char| !c.is_alphanumeric()) {
            "model" | "modeller" => self.model_listele(),
            "sohbetler" => self.sohbet_listele(),
            "yeni" => self.yeni(),
            "yardım" | "yardim" => self.yardim(),
            "çıkış" | "cikis" | "kapat" => std::process::exit(0),
            _ => return false,
        }
        self.istem();
        true
    }

    fn satir(&mut self, s: &str) {
        let s = s.trim().to_string();
        if self.guncelliyor.is_some() { return; }
        if let Some((_, url)) = self.guncel_sor.clone() {
            match s.to_lowercase().as_str() {
                "e" | "evet" => { self.guncel_sor = None; self.guncelle_basla(url); return; }
                "h" | "hayır" | "hayir" => { self.guncel_sor = None; self.not("Güncelleme atlandı."); self.istem(); return; }
                _ => self.guncel_sor = None,
            }
        }
        if let Some(l) = &self.liste {
            let adet = match l { Liste::Model(v) => v.len(), Liste::Sohbet(v) => v.len() };
            if let Ok(n) = s.parse::<usize>() {
                if n >= 1 && n <= adet { self.liste_sec(n); self.istem(); return; }
            }
            self.liste = None;
        }
        if self.mesgul { self.kuyruk.push_back(s); return; }
        let k = s.to_lowercase();
        if k.starts_with('/') {
            match k.as_str() {
                "/cikis" | "/çıkış" | "/exit" | "/q" => std::process::exit(0),
                "/model" => self.model_listele(),
                "/history_chat" => self.sohbet_listele(),
                "/yeni" => self.yeni(),
                "/yardim" | "/yardım" | "/help" => self.yardim(),
                "/ayarlar" => self.ayar_ac(),
                _ => self.not("Bilinmeyen komut. Komutlar için /yardim"),
            }
            self.istem();
            return;
        }
        if s.is_empty() { self.istem(); return; }
        self.gonder(&s, false);
    }

    fn gonder(&mut self, metin: &str, sesli: bool) {
        let Some((servis, model)) = self.aktif.model.split_once('|').map(|(a, b)| (a.to_string(), b.to_string())) else {
            self.not("Önce `/ayarlar` > Yapay zeka bölümünden bir API anahtarı ekle.");
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
                else if self.sesli_komut(&t) {}
                else if self.mesgul { self.not("Cevap bekleniyor, komut atlandı."); }
                else { self.gonder(&t, true); }
            }
            DEvent::Zaman => { self.dinliyor = false; yaz(SIL); self.istem(); }
            _ => {}
        }
    }

    fn tik(&mut self) {
        self.kare += 1;
        if let Some(il) = &self.guncelliyor {
            let b = il.load(Ordering::Relaxed);
            let dolu = (b as usize * 30) / 1000;
            yaz(&format!("{SIL}  {YESIL}{}{GRI}{}{SIFIRLA} %{}  güncelleme indiriliyor", "█".repeat(dolu), "░".repeat(30 - dolu), b / 10));
            return;
        }
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

#[cfg(windows)]
mod kisayol {
    use super::Girdi;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::Arc;

    #[repr(C)]
    #[derive(Default)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        x: i32,
        y: i32,
    }
    extern "system" {
        fn RegisterHotKey(h: isize, id: i32, mods: u32, vk: u32) -> i32;
        fn UnregisterHotKey(h: isize, id: i32) -> i32;
        fn GetMessageW(m: *mut Msg, h: isize, a: u32, b: u32) -> i32;
        pub fn PostThreadMessageW(tid: u32, msg: u32, w: usize, l: isize) -> i32;
        fn GetCurrentThreadId() -> u32;
    }

    fn coz(s: &str) -> Option<(u32, u32)> {
        let (mut mods, mut vk) = (0u32, 0u32);
        for p in s.split('+').map(|x| x.trim().to_lowercase()) {
            match p.as_str() {
                "ctrl" => mods |= 2,
                "alt" => mods |= 1,
                "shift" => mods |= 4,
                "win" => mods |= 8,
                "space" => vk = 0x20,
                x if x.len() == 1 && x.as_bytes()[0].is_ascii_alphanumeric() => vk = x.to_uppercase().as_bytes()[0] as u32,
                x if x.starts_with('f') => vk = 0x70 + x[1..].parse::<u32>().ok()?.checked_sub(1)?,
                _ => return None,
            }
        }
        if vk == 0 { None } else { Some((mods, vk)) }
    }

    pub fn baslat(spec: &str, gtx: Sender<Girdi>, tid: Arc<AtomicU32>) {
        let Some((mods, vk)) = coz(spec) else { return };
        std::thread::spawn(move || unsafe {
            tid.store(GetCurrentThreadId(), Ordering::SeqCst);
            if RegisterHotKey(0, 1, mods | 0x4000, vk) == 0 {
                let _ = gtx.send(Girdi::Satir("\u{1}kisayolhata".into()));
                return;
            }
            let mut m = Msg::default();
            while GetMessageW(&mut m, 0, 0, 0) > 0 {
                if m.message == 0x0312 { let _ = gtx.send(Girdi::Kisayol); }
            }
            UnregisterHotKey(0, 1);
        });
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
    mcp::bekci(araclar.clone());
    let (gtx, grx) = channel::<Girdi>();
    let dinleyici = if dinleme::modeller_hazir() {
        let d = dinleme::baslat(Arc::new(|| {}), ayar.dinleme_ayari(), oynatici.bayrak());
        Some(d)
    } else { None };
    let mut t = T {
        aktif: Sohbet::yeni(ayar.varsayilan_model()), ayar, gtx: gtx.clone(), araclar, oynatici, dinleyici, liste: None, mesgul: false, akan: String::new(), arac: String::new(),
        hata: false, sesli: false, sonra_dinle: false, dinliyor: false, kare: 0, kuyruk: VecDeque::new(), cocuk: None, kisayol_tid: Arc::new(AtomicU32::new(0)), guncel_sor: None, guncelliyor: None,
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
        if let Ok(Some((n, url))) = guncelleme::kontrol() {
            let _ = g3.send(Girdi::Yeni(n, url));
        }
    });
    let g4 = gtx.clone();
    std::thread::spawn(move || {
        for l in std::io::stdin().lock().lines() {
            match l { Ok(s) => { if g4.send(Girdi::Satir(s)).is_err() { break; } } Err(_) => break }
        }
        let _ = g4.send(Girdi::Kapat);
    });
    if t.ayar.secenekler().is_empty() { t.not("Başlamak için `/ayarlar` yaz ve Yapay zeka bölümüne bir API anahtarı ekle."); }
    t.kisayol_yenile();
    t.istem();
    loop {
        match grx.recv_timeout(Duration::from_millis(100)) {
            Ok(Girdi::Satir(s)) => {
                if s == "\u{1}kisayolhata" {
                    t.not("Kısayol tuşu başka bir uygulamada kullanılıyor; Ayarlar > Uyandırma'dan değiştir.");
                    t.istem();
                } else { t.satir(&s); }
            }
            Ok(Girdi::Kisayol) => t.kisayol_basildi(),
            Ok(Girdi::Yeni(n, url)) => {
                t.guncel_sor = Some((n, url));
                t.not(&format!("Yeni sürüm var (0.4.{n}). Güncellensin mi? (E/H)"));
                t.istem();
            }
            Ok(Girdi::GuncelBitti(r)) => match r {
                Ok(()) => { yaz(&format!("{SIL}  {GRI}Güncelleme hazır, yeniden başlatılıyor…{SIFIRLA}\n")); std::process::exit(0); }
                Err(e) => { t.guncelliyor = None; t.not(&format!("Güncelleme başarısız: {e}")); t.istem(); }
            },
            Ok(Girdi::Ai(o)) => t.ai_olay(o),
            Ok(Girdi::Din(e)) => t.dinle_olay(e),
            Ok(Girdi::Kapat) => break,
            Err(RecvTimeoutError::Timeout) => t.tik(),
            Err(_) => break,
        }
    }
}
