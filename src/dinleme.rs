//! Mikrofon (cpal) + Vosk (libvosk.dll çalışma anında yüklenir) + uyandırma.
use crate::ayarlar::klasor;
use libloading::Library;
use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type Bildir = Arc<dyn Fn() + Send + Sync>;

pub const BLOK: usize = 2000; // 125 ms @16 kHz

#[derive(Clone)]
pub struct DAyar {
    pub ekstra: Vec<String>,
    pub cumle_basi: bool,
    pub sesle_kes: bool,
    pub hoparlor: bool,
    pub takip_s: f32,
    pub sessizlik_s: f32,
    pub kapi: bool,
}

pub enum DKomut {
    Dinle,
    Bekle,
    Test(bool),
    Ogren(bool),
    Ayar(DAyar),
}

pub enum DEvent {
    Hazir,
    Hata(String),
    Seviye(f32),
    Uyandi,
    Kismi(String),
    Son(String),
    Zaman,
    TestMetin(String, String),
    Ogrenme(usize),
    Ogrenildi(Vec<String>),
}

pub struct Dinleyici {
    pub komut: Sender<DKomut>,
    pub olay: Receiver<DEvent>,
}

pub fn modeller_dizini() -> std::path::PathBuf {
    let d = klasor().join("modeller");
    let _ = std::fs::create_dir_all(&d);
    d
}
pub const TR_MODEL: &str = "vosk-model-small-tr-0.3";
pub const EN_MODEL: &str = "vosk-model-small-en-us-0.15";

pub fn modeller_hazir() -> bool {
    modeller_dizini().join(TR_MODEL).exists() && modeller_dizini().join(EN_MODEL).exists()
}

// ---------- Vosk FFI ----------
type Ptr = *mut c_void;
struct Vosk {
    _lib: Library,
    model_new: unsafe extern "C" fn(*const c_char) -> Ptr,
    rec_new: unsafe extern "C" fn(Ptr, f32) -> Ptr,
    accept: unsafe extern "C" fn(Ptr, *const i16, i32) -> i32,
    result: unsafe extern "C" fn(Ptr) -> *const c_char,
    partial: unsafe extern "C" fn(Ptr) -> *const c_char,
    final_: unsafe extern "C" fn(Ptr) -> *const c_char,
    reset: unsafe extern "C" fn(Ptr),
    log: unsafe extern "C" fn(i32),
}

impl Vosk {
    fn yukle() -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let yol = exe.parent().map(|p| p.join("libvosk.dll")).ok_or("yol yok")?;
        unsafe {
            let lib = Library::new(&yol).map_err(|e| format!("libvosk.dll yüklenemedi: {e}"))?;
            macro_rules! al {
                ($ad:literal) => {
                    *lib.get(concat!($ad, "\0").as_bytes()).map_err(|e| format!("{}: {e}", $ad))?
                };
            }
            let v = Vosk {
                model_new: al!("vosk_model_new"),
                rec_new: al!("vosk_recognizer_new"),
                accept: al!("vosk_recognizer_accept_waveform_s"),
                result: al!("vosk_recognizer_result"),
                partial: al!("vosk_recognizer_partial_result"),
                final_: al!("vosk_recognizer_final_result"),
                reset: al!("vosk_recognizer_reset"),
                log: al!("vosk_set_log_level"),
                _lib: lib,
            };
            (v.log)(-1);
            Ok(v)
        }
    }
    fn model(&self, yol: &std::path::Path) -> Result<Ptr, String> {
        let c = CString::new(yol.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let m = unsafe { (self.model_new)(c.as_ptr()) };
        if m.is_null() { Err(format!("Model açılamadı: {}", yol.display())) } else { Ok(m) }
    }
    fn tani(&self, m: Ptr) -> Ptr {
        unsafe { (self.rec_new)(m, 16000.0) }
    }
    fn kabul(&self, r: Ptr, p: &[i16]) -> bool {
        unsafe { (self.accept)(r, p.as_ptr(), p.len() as i32) == 1 }
    }
    fn oku(&self, p: *const c_char, alan: &str) -> String {
        if p.is_null() { return String::new(); }
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().to_string();
        serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|j| j[alan].as_str().map(|x| x.trim().to_string())).unwrap_or_default()
    }
    fn sonuc(&self, r: Ptr) -> String { self.oku(unsafe { (self.result)(r) }, "text") }
    fn kismi(&self, r: Ptr) -> String { self.oku(unsafe { (self.partial)(r) }, "partial") }
    fn bitir(&self, r: Ptr) -> String { self.oku(unsafe { (self.final_)(r) }, "text") }
    fn sifirla(&self, r: Ptr) { unsafe { (self.reset)(r) } }
}

// ---------- Uyandırma eşleştirme ----------
const ONEKLER: [&str; 9] = ["hey", "a", "the", "uh", "oh", "and", "hi", "um", "hay"];
const TEMEL: [&str; 6] = ["edit", "edith", "edits", "edid", "edict", "ed"];

fn uyandirma_var(metin: &str, ekstra: &[String], cumle_basi: bool) -> bool {
    let kucuk = metin.to_lowercase();
    let tok: Vec<&str> = kucuk.split_whitespace().collect();
    for (i, t) in tok.iter().enumerate() {
        let eslesti = (TEMEL.contains(t) && *t != "ed" && *t != "edict") || ekstra.iter().any(|e| e == t);
        if eslesti && (!cumle_basi || tok[..i].iter().all(|x| ONEKLER.contains(x))) { return true; }
    }
    ekstra.iter().any(|e| e.contains(' ') && kucuk.contains(e.as_str()))
}

/// Türkçe sonuçtan baştaki uyandırma artıklarını atar.
fn artik_temizle(m: &str) -> String {
    const ATILACAK: [&str; 10] = ["hey", "hay", "ey", "edit", "edith", "edici", "edit'i", "ediz", "edid", "heyedit"];
    let mut t: Vec<&str> = m.split_whitespace().collect();
    while t.first().map(|x| ATILACAK.contains(&x.to_lowercase().as_str())).unwrap_or(false) { t.remove(0); }
    t.join(" ")
}

fn rms(b: &[i16]) -> f32 {
    (b.iter().map(|&x| (x as f32).powi(2)).sum::<f32>() / b.len().max(1) as f32).sqrt()
}

#[derive(PartialEq, Clone, Copy)]
enum Mod {
    Bekle,
    Dinle,
    Test,
}

struct Is {
    v: Vosk,
    en: Ptr,
    tr: Ptr,
    mod_: Mod,
    ring: VecDeque<i16>,
    ayar: DAyar,
    otx: Sender<DEvent>,
    bildir: Bildir,
    konusuyor: Arc<AtomicBool>,
    ardisik: u32,
    taban: f32,
    acik: u32,
    onbellek: VecDeque<Vec<i16>>,
    sessiz: u32,
    konustu: bool,
    parcalar: Vec<String>,
    son_kismi: String,
    baslangic: Instant,
    ogren: Option<Vec<String>>,
    test_en: String,
    test_tr: String,
}

unsafe impl Send for Is {}

impl Is {
    fn g(&self, e: DEvent) {
        let _ = self.otx.send(e);
        (self.bildir.as_ref())();
    }

    fn dinlemeye_gec(&mut self, geri_al: bool) {
        self.v.sifirla(self.tr);
        self.parcalar.clear();
        self.son_kismi.clear();
        self.sessiz = 0;
        self.konustu = false;
        self.acik = 0;
        self.onbellek.clear();
        self.baslangic = Instant::now();
        self.mod_ = Mod::Dinle;
        if geri_al {
            let v: Vec<i16> = self.ring.iter().copied().collect();
            for p in v.chunks(4000) {
                if self.v.kabul(self.tr, p) { let s = self.v.sonuc(self.tr); if !s.is_empty() { self.parcalar.push(s); } }
            }
            let t = self.metin();
            if !t.is_empty() { self.konustu = true; self.g(DEvent::Kismi(t)); }
        }
    }

    fn metin(&self) -> String {
        let mut t = self.parcalar.join(" ");
        if !self.son_kismi.is_empty() { t.push(' '); t.push_str(&self.son_kismi); }
        artik_temizle(t.trim())
    }

    fn blok(&mut self, b: &[i16]) {
        let r = rms(b);
        self.g(DEvent::Seviye((r / 4000.0).min(1.0)));
        if r < self.taban * 1.5 || self.taban == 0.0 { self.taban = self.taban * 0.98 + r * 0.02; }
        let esik = (self.taban * 2.4).max(170.0);
        self.ring.extend(b.iter().copied());
        while self.ring.len() > 80000 { self.ring.pop_front(); }
        match self.mod_ {
            Mod::Bekle => self.bekle(b),
            Mod::Dinle => self.dinle(b, r > esik),
            Mod::Test => {
                if self.v.kabul(self.en, b) { self.test_en = self.v.sonuc(self.en); } else { let k = self.v.kismi(self.en); if !k.is_empty() { self.test_en = k; } }
                if self.v.kabul(self.tr, b) { self.test_tr = self.v.sonuc(self.tr); } else { let k = self.v.kismi(self.tr); if !k.is_empty() { self.test_tr = k; } }
                self.g(DEvent::TestMetin(self.test_en.clone(), self.test_tr.clone()));
            }
        }
    }

    fn bekle(&mut self, b: &[i16]) {
        let (metin, son) = if self.v.kabul(self.en, b) { (self.v.sonuc(self.en), true) } else { (self.v.kismi(self.en), false) };
        if metin.is_empty() { self.ardisik = 0; return; }
        if let Some(ornek) = &mut self.ogren {
            if son {
                ornek.push(metin);
                let n = ornek.len();
                self.g(DEvent::Ogrenme(n));
                if n >= 8 { self.ogrenmeyi_bitir(); }
            }
            return;
        }
        if !uyandirma_var(&metin, &self.ayar.ekstra, self.ayar.cumle_basi) { self.ardisik = 0; return; }
        self.ardisik += 1;
        let caliyor = self.konusuyor.load(Ordering::Relaxed);
        if caliyor && (!self.ayar.sesle_kes || (self.ayar.hoparlor && !son)) { return; }
        if son || self.ardisik >= 3 {
            self.ardisik = 0;
            self.v.sifirla(self.en);
            self.g(DEvent::Uyandi);
            self.dinlemeye_gec(true);
        }
    }

    fn ogrenmeyi_bitir(&mut self) {
        let Some(ornek) = self.ogren.take() else { return };
        let mut say: HashMap<String, u32> = HashMap::new();
        for o in &ornek {
            let mut gorulen: Vec<String> = vec![];
            for t in o.to_lowercase().split_whitespace() {
                if t.chars().count() >= 3 && !gorulen.contains(&t.to_string()) { gorulen.push(t.to_string()); }
            }
            for t in gorulen { *say.entry(t).or_default() += 1; }
        }
        let mut v: Vec<String> = say.into_iter().filter(|(t, n)| *n >= 2 && !ONEKLER.contains(&t.as_str()) && !TEMEL.contains(&t.as_str())).map(|(t, _)| t).collect();
        v.sort();
        self.g(DEvent::Ogrenildi(v));
    }

    fn dinle(&mut self, b: &[i16], konusma: bool) {
        if konusma { self.sessiz = 0; self.konustu = true; } else { self.sessiz += 1; }
        let besle = if !self.ayar.kapi { true } else if konusma {
            let eski: Vec<Vec<i16>> = self.onbellek.drain(..).collect();
            for e in eski { self.besle(&e); }
            self.acik = 8;
            true
        } else if self.acik > 0 {
            self.acik -= 1;
            true
        } else {
            self.onbellek.push_back(b.to_vec());
            while self.onbellek.len() > 3 { self.onbellek.pop_front(); }
            false
        };
        if besle { self.besle(b); }
        let sure = self.baslangic.elapsed();
        if self.konustu && (self.sessiz as f32 * 0.125 >= self.ayar.sessizlik_s || sure > Duration::from_secs(60)) {
            let s = self.v.bitir(self.tr);
            if !s.is_empty() { self.parcalar.push(s); }
            self.son_kismi.clear();
            let t = self.metin();
            self.mod_ = Mod::Bekle;
            self.v.sifirla(self.en);
            if t.is_empty() { self.g(DEvent::Zaman) } else { self.g(DEvent::Son(t)) }
        } else if !self.konustu && sure.as_secs_f32() > self.ayar.takip_s {
            self.mod_ = Mod::Bekle;
            self.v.sifirla(self.en);
            self.g(DEvent::Zaman);
        }
    }

    fn besle(&mut self, b: &[i16]) {
        if self.v.kabul(self.tr, b) {
            let s = self.v.sonuc(self.tr);
            if !s.is_empty() { self.parcalar.push(s); }
            self.son_kismi.clear();
        } else {
            self.son_kismi = self.v.kismi(self.tr);
        }
        let t = self.metin();
        if !t.is_empty() { self.konustu = true; self.g(DEvent::Kismi(t)); }
    }
}

fn mikrofon_kur<T>(cihaz: &cpal::Device, yap: &cpal::StreamConfig, tx: Sender<Vec<i16>>) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;
    use cpal::Sample;
    let kanal = yap.channels as usize;
    let adim = yap.sample_rate.0 as f64 / 16000.0;
    let (mut toplam, mut n, mut faz) = (0f32, 0f32, 0f64);
    cihaz
        .build_input_stream(
            yap,
            move |veri: &[T], _| {
                let mut cikis = Vec::with_capacity(veri.len() / kanal / 2 + 1);
                for kare in veri.chunks(kanal) {
                    let m: f32 = kare.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / kanal as f32;
                    toplam += m;
                    n += 1.0;
                    faz += 1.0;
                    if faz >= adim {
                        cikis.push(((toplam / n) * 32767.0).clamp(-32768.0, 32767.0) as i16);
                        toplam = 0.0;
                        n = 0.0;
                        faz -= adim;
                    }
                }
                if !cikis.is_empty() { let _ = tx.send(cikis); }
            },
            |e| eprintln!("mikrofon: {e}"),
            None,
        )
        .map_err(|e| format!("Mikrofon açılamadı: {e}"))
}

pub fn baslat(bildir: Bildir, ayar: DAyar, konusuyor: Arc<AtomicBool>) -> Dinleyici {
    let (ktx, krx) = channel::<DKomut>();
    let (otx, orx) = channel::<DEvent>();
    std::thread::spawn(move || {
        let hata = |m: String| {
            let _ = otx.send(DEvent::Hata(m));
            (bildir.as_ref())();
        };
        let v = match Vosk::yukle() { Ok(v) => v, Err(e) => return hata(e) };
        let d = modeller_dizini();
        let (mtr, men) = match (v.model(&d.join(TR_MODEL)), v.model(&d.join(EN_MODEL))) { (Ok(a), Ok(b)) => (a, b), (Err(e), _) | (_, Err(e)) => return hata(e) };
        let (en, tr) = (v.tani(men), v.tani(mtr));

        use cpal::traits::{HostTrait, StreamTrait, DeviceTrait};
        let Some(cihaz) = cpal::default_host().default_input_device() else { return hata("Mikrofon bulunamadı.".into()) };
        let Ok(yap) = cihaz.default_input_config() else { return hata("Mikrofon ayarı okunamadı.".into()) };
        let (mtx, mrx) = channel::<Vec<i16>>();
        let sf = yap.sample_format();
        let yapi: cpal::StreamConfig = yap.into();
        let akis = match sf {
            cpal::SampleFormat::F32 => mikrofon_kur::<f32>(&cihaz, &yapi, mtx),
            cpal::SampleFormat::I16 => mikrofon_kur::<i16>(&cihaz, &yapi, mtx),
            cpal::SampleFormat::U16 => mikrofon_kur::<u16>(&cihaz, &yapi, mtx),
            _ => Err("Mikrofon biçimi desteklenmiyor.".to_string()),
        };
        let akis = match akis { Ok(a) => a, Err(e) => return hata(e) };
        if let Err(e) = akis.play() { return hata(format!("Mikrofon başlatılamadı: {e}")); }

        let mut is = Is {
            v, en, tr, mod_: Mod::Bekle, ring: VecDeque::new(), ayar, otx: otx.clone(), bildir: bildir.clone(), konusuyor, ardisik: 0, taban: 0.0, acik: 0,
            onbellek: VecDeque::new(), sessiz: 0, konustu: false, parcalar: vec![], son_kismi: String::new(), baslangic: Instant::now(), ogren: None,
            test_en: String::new(), test_tr: String::new(),
        };
        is.g(DEvent::Hazir);
        let mut tampon: Vec<i16> = vec![];
        loop {
            while let Ok(k) = krx.try_recv() {
                match k {
                    DKomut::Dinle => is.dinlemeye_gec(false),
                    DKomut::Bekle => { is.mod_ = Mod::Bekle; is.v.sifirla(is.en); }
                    DKomut::Test(a) => {
                        if a { is.v.sifirla(is.en); is.v.sifirla(is.tr); is.test_en.clear(); is.test_tr.clear(); is.mod_ = Mod::Test; } else { is.mod_ = Mod::Bekle; }
                    }
                    DKomut::Ogren(a) => { is.ogren = if a { Some(vec![]) } else { None }; is.mod_ = Mod::Bekle; }
                    DKomut::Ayar(a) => is.ayar = a,
                }
            }
            match mrx.recv_timeout(Duration::from_millis(100)) {
                Ok(v) => tampon.extend(v),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
            while tampon.len() >= BLOK {
                let b: Vec<i16> = tampon.drain(..BLOK).collect();
                is.blok(&b);
            }
        }
    });
    Dinleyici { komut: ktx, olay: orx }
}
