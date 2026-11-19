use crate::ayarlar::Ayar;
use base64::Engine;
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};
use serde_json::{json, Value};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const GEMINI_SESLER: [&str; 30] = [
    "Zephyr", "Puck", "Charon", "Kore", "Fenrir", "Leda", "Orus", "Aoede", "Callirrhoe", "Autonoe", "Enceladus", "Iapetus", "Umbriel", "Algieba", "Despina",
    "Erinome", "Algenib", "Rasalgethi", "Laomedeia", "Achernar", "Alnilam", "Schedar", "Gacrux", "Pulcherrima", "Achird", "Zubenelgenubi", "Vindemiatrix",
    "Sadachbia", "Sadaltager", "Sulafat",
];

pub struct Oynatici {
    gonder: Sender<(u64, Vec<i16>)>,
    nesil: Arc<AtomicU64>,
    caliyor: Arc<AtomicBool>,
    uretim: Arc<AtomicUsize>,
    bekleme: Mutex<Vec<(String, Instant)>>,
}

impl Oynatici {
    pub fn yeni() -> Arc<Self> {
        let (tx, rx) = channel::<(u64, Vec<i16>)>();
        let nesil = Arc::new(AtomicU64::new(0));
        let caliyor = Arc::new(AtomicBool::new(false));
        let uretim = Arc::new(AtomicUsize::new(0));
        let (n2, c2, u2) = (nesil.clone(), caliyor.clone(), uretim.clone());
        std::thread::spawn(move || {
            let Ok((_ak, el)) = OutputStream::try_default() else { return };
            let mut sink = Sink::try_new(&el).ok();
            let mut son = 0u64;
            loop {
                let simdi = n2.load(Ordering::SeqCst);
                if son != simdi {
                    if let Some(s) = sink.take() { s.stop(); }
                    sink = Sink::try_new(&el).ok();
                    son = simdi;
                }
                match rx.recv_timeout(Duration::from_millis(80)) {
                    Ok((n, v)) => {
                        if n == n2.load(Ordering::SeqCst) {
                            if let Some(s) = &sink { s.append(SamplesBuffer::new(1, 24000, v)); }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(_) => break,
                }
                c2.store(u2.load(Ordering::SeqCst) > 0 || sink.as_ref().map(|s| !s.empty()).unwrap_or(false), Ordering::SeqCst);
            }
        });
        Arc::new(Self { gonder: tx, nesil, caliyor, uretim, bekleme: Mutex::new(vec![]) })
    }

    /// Çalan sesi keser.
    pub fn kes(&self) {
        self.nesil.fetch_add(1, Ordering::SeqCst);
    }

    pub fn caliyor(&self) -> bool {
        self.caliyor.load(Ordering::SeqCst)
    }

    pub fn bayrak(&self) -> Arc<AtomicBool> {
        self.caliyor.clone()
    }

    fn beklemede(&self, motor: &str) -> bool {
        self.bekleme.lock().unwrap().iter().any(|(m, t)| m == motor && t.elapsed() < Duration::from_secs(60))
    }

    pub fn konus(self: &Arc<Self>, metin: &str, ayar: &Ayar) {
        let temiz = konusma_metni(metin);
        if temiz.is_empty() { return; }
        let n = self.nesil.fetch_add(1, Ordering::SeqCst) + 1;
        self.uretim.fetch_add(1, Ordering::SeqCst);
        let me = self.clone();
        let ayar = ayar.clone();
        std::thread::spawn(move || {
            for parca in parcala(&temiz) {
                if me.nesil.load(Ordering::SeqCst) != n { break; }
                if let Some(pcm) = me.uret(&parca, &ayar) {
                    let _ = me.gonder.send((n, pcm));
                }
            }
            me.uretim.fetch_sub(1, Ordering::SeqCst);
        });
    }

    fn uret(&self, metin: &str, a: &Ayar) -> Option<Vec<i16>> {
        for motor in &a.ses_sirasi {
            if self.beklemede(motor) {
                if a.ses_sirayla { continue } else { return None }
            }
            let r = match motor.as_str() {
                "gemini" if !a.gemini_ses_acik => continue,
                "eleven" if !a.eleven_ses_acik => continue,
                "gemini" if !a.gemini_anahtar.is_empty() => gemini_tts(metin, &a.gemini_anahtar, &a.gemini_ses),
                "eleven" if !a.eleven_anahtar.is_empty() && !a.eleven_ses.is_empty() => eleven_tts(metin, a),
                _ => continue,
            };
            match r {
                Ok(v) => return Some(v),
                Err(_) => {
                    self.bekleme.lock().unwrap().push((motor.clone(), Instant::now()));
                    if !a.ses_sirayla { return None; }
                }
            }
        }
        None
    }
}

fn pcm16(b: &[u8]) -> Vec<i16> {
    b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
}

pub fn gemini_tts(metin: &str, anahtar: &str, ses: &str) -> Result<Vec<i16>, String> {
    let g = json!({"contents": [{"parts": [{"text": metin}]}], "generationConfig": {"responseModalities": ["AUDIO"],
        "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": ses}}}}});
    let r: Value = ureq::post("https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-tts-preview:generateContent")
        .set("x-goog-api-key", anahtar)
        .timeout(Duration::from_secs(60))
        .send_json(g)
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let d = r["candidates"][0]["content"]["parts"][0]["inlineData"]["data"].as_str().ok_or("ses yok")?;
    let b = base64::engine::general_purpose::STANDARD.decode(d).map_err(|e| e.to_string())?;
    Ok(pcm16(&b))
}

pub fn eleven_tts(metin: &str, a: &Ayar) -> Result<Vec<i16>, String> {
    let g = json!({"text": metin, "model_id": a.eleven_model, "voice_settings": {"stability": a.eleven_kararlilik, "similarity_boost": a.eleven_benzerlik}});
    let y = ureq::post(&format!("https://api.elevenlabs.io/v1/text-to-speech/{}?output_format=pcm_24000", a.eleven_ses))
        .set("xi-api-key", &a.eleven_anahtar)
        .timeout(Duration::from_secs(60))
        .send_json(g)
        .map_err(|e| e.to_string())?;
    let mut b = vec![];
    y.into_reader().read_to_end(&mut b).map_err(|e| e.to_string())?;
    Ok(pcm16(&b))
}

/// Okunmayacak şeyleri ayıklar: kod blokları, bağlantılar, işaretler.
pub fn konusma_metni(m: &str) -> String {
    let mut cikti = String::new();
    for (i, p) in m.split("```").enumerate() {
        if i % 2 == 0 { cikti.push_str(p); } else { cikti.push_str(" İlgili kod bloğunu ekledim. "); }
    }
    let kelimeler: Vec<&str> = cikti.split_whitespace().filter(|w| !w.starts_with("http://") && !w.starts_with("https://")).collect();
    let t: String = kelimeler.join(" ").chars().filter(|c| !matches!(c, '*' | '#' | '`' | '_' | '~' | '|' | '>')).collect();
    t.chars().take(2500).collect()
}

/// Cümle cümle parçalar: ilk parça ≥25, sonrakiler ≥250 karakter.
pub fn parcala(t: &str) -> Vec<String> {
    let mut v = vec![];
    let mut mevcut = String::new();
    let k: Vec<char> = t.chars().collect();
    for (i, c) in k.iter().enumerate() {
        mevcut.push(*c);
        let son = matches!(c, '.' | '!' | '?' | '…') && k.get(i + 1).map(|x| x.is_whitespace()).unwrap_or(true) || *c == '\n';
        let esik = if v.is_empty() { 25 } else { 250 };
        if son && mevcut.chars().count() >= esik {
            v.push(mevcut.trim().to_string());
            mevcut.clear();
        }
    }
    if !mevcut.trim().is_empty() { v.push(mevcut.trim().to_string()); }
    v
}

fn eleven_get(yol: &str, anahtar: &str) -> Result<Value, String> {
    ureq::get(&format!("https://api.elevenlabs.io{yol}")).set("xi-api-key", anahtar).timeout(Duration::from_secs(20)).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())
}

/// (voice_id, ad)
pub fn eleven_sesler(anahtar: &str) -> Result<Vec<(String, String)>, String> {
    let j = eleven_get("/v1/voices", anahtar)?;
    Ok(j["voices"].as_array().map(|a| a.iter().map(|v| (v["voice_id"].as_str().unwrap_or("").to_string(), v["name"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default())
}

/// (model_id, ad)
pub fn eleven_modeller(anahtar: &str) -> Result<Vec<(String, String)>, String> {
    let j = eleven_get("/v1/models", anahtar)?;
    Ok(j.as_array().map(|a| a.iter().filter(|m| m["can_do_text_to_speech"].as_bool().unwrap_or(true)).map(|v| (v["model_id"].as_str().unwrap_or("").to_string(), v["name"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default())
}

/// (kullanılan, limit)
pub fn eleven_kredi(anahtar: &str) -> Result<(u64, u64), String> {
    let j = eleven_get("/v1/user/subscription", anahtar)?;
    Ok((j["character_count"].as_u64().unwrap_or(0), j["character_limit"].as_u64().unwrap_or(0)))
}
