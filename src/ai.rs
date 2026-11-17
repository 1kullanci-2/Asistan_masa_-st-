use crate::mcp::{sema_temizle, Mcp};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub enum Olay {
    Parca(String),
    Arac(String),
    Bitti,
    Hata(String),
}

pub type Araclar = Arc<Mutex<Vec<Mcp>>>;

pub const SISTEM: &str = "Adın E.D.I.T.H. Rol yapma; kendini başka bir kimlik, şirket veya aksesuarla ilişkilendirme. \
Kısa, doğal ve net cevap ver; kullanıcının dilinde konuş. Kod yazarsan ``` bloğu içinde ver. \
Kullanıcıdan cevap bekliyorsan cevabın sonuna [SORU] ekle. Sohbetin ilk mesajında cevabın sonuna [BASLIK: en fazla 4 kelimelik başlık] ekle.";

fn istek(url: &str, govde: &Value, anahtar: &str, gemini: bool) -> Result<ureq::Response, String> {
    let ajan = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(90)).build();
    let mut deneme = 0u64;
    loop {
        let i = ajan.post(url).set("Content-Type", "application/json");
        let i = if gemini { i.set("x-goog-api-key", anahtar) } else { i.set("Authorization", &format!("Bearer {anahtar}")) };
        match i.send_json(govde.clone()) {
            Ok(r) => return Ok(r),
            Err(ureq::Error::Status(429, _)) => return Err("Bu modelin kotası doldu, biraz sonra tekrar dene ya da başka model seç.".into()),
            Err(ureq::Error::Status(k, _)) if k >= 500 && deneme < 3 => {
                deneme += 1;
                std::thread::sleep(Duration::from_millis(1200 * deneme));
            }
            Err(ureq::Error::Status(k, r)) => {
                let g = r.into_string().unwrap_or_default();
                return Err(format!("Hata {k}: {}", g.chars().take(200).collect::<String>()));
            }
            Err(_) if deneme < 3 => {
                deneme += 1;
                std::thread::sleep(Duration::from_millis(1200 * deneme));
            }
            Err(e) => return Err(format!("Bağlantı kurulamadı: {e}")),
        }
    }
}

/// Gemini için araç tanımları ve ad eşlemesi: m{i}_{ad}
fn arac_tanimlari(araclar: &Araclar, kapali: &[Vec<String>]) -> Vec<Value> {
    let k = araclar.lock().unwrap();
    let mut v = vec![];
    for (i, m) in k.iter().enumerate() {
        if !m.bagli { continue; }
        for a in &m.araclar {
            if kapali.get(i).map(|x| x.contains(&a.ad)).unwrap_or(false) { continue; }
            let mut tanim = json!({"name": format!("m{i}_{}", a.ad), "description": a.aciklama});
            let sema = sema_temizle(&a.sema);
            if sema["properties"].as_object().map(|p| !p.is_empty()).unwrap_or(false) { tanim["parameters"] = sema; }
            v.push(tanim);
        }
    }
    v
}

fn arac_calistir(araclar: &Araclar, ad: &str, args: Value) -> String {
    let Some(rest) = ad.strip_prefix('m') else { return "Bilinmeyen araç".into() };
    let Some((i, gercek)) = rest.split_once('_') else { return "Bilinmeyen araç".into() };
    let Ok(i) = i.parse::<usize>() else { return "Bilinmeyen araç".into() };
    let mut k = araclar.lock().unwrap();
    match k.get_mut(i) {
        Some(m) => m.cagir(gercek, args).unwrap_or_else(|e| format!("Araç hatası: {e}")),
        None => "Bilinmeyen araç".into(),
    }
}

pub fn akis(servis: &str, model: &str, anahtar: &str, gecmis: Vec<(String, String)>, araclar: Araclar, kapali: Vec<Vec<String>>, tx: Sender<Olay>, yenile: impl Fn()) {
    let gonder = |o: Olay| {
        let _ = tx.send(o);
        yenile();
    };
    if servis != "gemini" {
        let taban = match servis {
            "groq" => "https://api.groq.com/openai/v1/chat/completions",
            "cerebras" => "https://api.cerebras.ai/v1/chat/completions",
            _ => "https://api.mistral.ai/v1/chat/completions",
        };
        let mut msgs = vec![json!({"role": "system", "content": SISTEM})];
        msgs.extend(gecmis.iter().map(|(r, m)| json!({"role": r, "content": m})));
        match istek(taban, &json!({"model": model, "messages": msgs, "stream": true}), anahtar, false) {
            Ok(y) => {
                for l in BufReader::new(y.into_reader()).lines().flatten() {
                    let Some(v) = l.strip_prefix("data:") else { continue };
                    let v = v.trim();
                    if v == "[DONE]" { break; }
                    if let Ok(j) = serde_json::from_str::<Value>(v) {
                        if let Some(t) = j["choices"][0]["delta"]["content"].as_str() { gonder(Olay::Parca(t.into())); }
                    }
                }
                gonder(Olay::Bitti);
            }
            Err(e) => gonder(Olay::Hata(e)),
        }
        return;
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse");
    let mut icerik: Vec<Value> = gecmis.iter().map(|(r, m)| json!({"role": if r == "user" {"user"} else {"model"}, "parts": [{"text": m}]})).collect();
    let tanimlar = arac_tanimlari(&araclar, &kapali);
    for _tur in 0..8 {
        let mut govde = json!({"systemInstruction": {"parts": [{"text": SISTEM}]}, "contents": icerik});
        if !tanimlar.is_empty() { govde["tools"] = json!([{"functionDeclarations": tanimlar}]); }
        let y = match istek(&url, &govde, anahtar, true) {
            Ok(y) => y,
            Err(e) => return gonder(Olay::Hata(e)),
        };
        let mut parcalar: Vec<Value> = vec![];
        let mut cagrilar: Vec<(String, Value)> = vec![];
        for l in BufReader::new(y.into_reader()).lines().flatten() {
            let Some(v) = l.strip_prefix("data:") else { continue };
            let Ok(j) = serde_json::from_str::<Value>(v.trim()) else { continue };
            let Some(ps) = j["candidates"][0]["content"]["parts"].as_array() else { continue };
            for p in ps {
                if p["thought"].as_bool() == Some(true) { continue; }
                if let Some(t) = p["text"].as_str() {
                    if !t.is_empty() { gonder(Olay::Parca(t.into())); }
                }
                if let Some(fc) = p.get("functionCall") {
                    cagrilar.push((fc["name"].as_str().unwrap_or("").into(), fc["args"].clone()));
                    parcalar.push(p.clone());
                }
            }
        }
        if cagrilar.is_empty() { return gonder(Olay::Bitti); }
        icerik.push(json!({"role": "model", "parts": parcalar}));
        let mut yanitlar = vec![];
        for (ad, args) in cagrilar {
            gonder(Olay::Arac(ad.split_once('_').map(|x| x.1).unwrap_or(&ad).to_string()));
            let sonuc = arac_calistir(&araclar, &ad, if args.is_null() { json!({}) } else { args });
            yanitlar.push(json!({"functionResponse": {"name": ad, "response": {"result": sonuc}}}));
        }
        icerik.push(json!({"role": "user", "parts": yanitlar}));
    }
    gonder(Olay::Hata("Araç çağrısı çok uzadı, durduruldu.".into()));
}

/// Cevaptan [BASLIK: …] ve [SORU] işaretlerini ayırır: (temiz metin, başlık, soru_mu)
pub fn isaretleri_ayir(m: &str) -> (String, Option<String>, bool) {
    let mut t = m.to_string();
    let mut baslik = None;
    if let Some(i) = t.find("[BASLIK:") {
        if let Some(j) = t[i..].find(']') {
            baslik = Some(t[i + 8..i + j].trim().to_string());
            t.replace_range(i..i + j + 1, "");
        }
    }
    let soru = t.contains("[SORU]");
    t = t.replace("[SORU]", "");
    (t.trim().to_string(), baslik, soru)
}
