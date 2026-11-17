use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub enum Olay {
    Parca(String),
    Bitti,
    Hata(String),
}

pub const SISTEM: &str = "Adın E.D.I.T.H. Rol yapma; kendini başka bir kimlik, şirket veya aksesuarla ilişkilendirme. \
Kısa, doğal ve net cevap ver; kullanıcının dilinde konuş. Kod yazarsan ``` bloğu içinde ver. \
Kullanıcıdan cevap bekliyorsan cevabın sonuna [SORU] ekle. Sohbetin ilk mesajında cevabın sonuna [BASLIK: en fazla 4 kelimelik başlık] ekle.";

pub fn akis(servis: &str, model: &str, anahtar: &str, gecmis: Vec<(String, String)>, tx: Sender<Olay>, yenile: impl Fn()) {
    let (url, govde, gemini) = if servis == "gemini" {
        let contents: Vec<Value> = gecmis
            .iter()
            .map(|(r, m)| json!({"role": if r == "user" {"user"} else {"model"}, "parts": [{"text": m}]}))
            .collect();
        (
            format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"),
            json!({"systemInstruction": {"parts": [{"text": SISTEM}]}, "contents": contents}),
            true,
        )
    } else {
        let taban = match servis {
            "groq" => "https://api.groq.com/openai/v1/chat/completions",
            "cerebras" => "https://api.cerebras.ai/v1/chat/completions",
            _ => "https://api.mistral.ai/v1/chat/completions",
        };
        let mut msgs = vec![json!({"role": "system", "content": SISTEM})];
        msgs.extend(gecmis.iter().map(|(r, m)| json!({"role": r, "content": m})));
        (taban.to_string(), json!({"model": model, "messages": msgs, "stream": true}), false)
    };

    let ajan = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(60)).build();
    let mut deneme = 0;
    let yanit = loop {
        let istek = ajan.post(&url).set("Content-Type", "application/json");
        let istek = if gemini { istek.set("x-goog-api-key", anahtar) } else { istek.set("Authorization", &format!("Bearer {anahtar}")) };
        match istek.send_json(govde.clone()) {
            Ok(r) => break r,
            Err(ureq::Error::Status(429, _)) => {
                let _ = tx.send(Olay::Hata("Bu modelin kotası doldu, biraz sonra tekrar dene ya da başka model seç.".into()));
                yenile();
                return;
            }
            Err(ureq::Error::Status(kod, _)) if (kod == 503 || kod >= 500) && deneme < 3 => {
                deneme += 1;
                std::thread::sleep(Duration::from_millis(1200 * deneme));
            }
            Err(ureq::Error::Status(kod, r)) => {
                let g = r.into_string().unwrap_or_default();
                let _ = tx.send(Olay::Hata(format!("Hata {kod}: {}", g.chars().take(200).collect::<String>())));
                yenile();
                return;
            }
            Err(_) if deneme < 3 => {
                deneme += 1;
                std::thread::sleep(Duration::from_millis(1200 * deneme));
            }
            Err(e) => {
                let _ = tx.send(Olay::Hata(format!("Bağlantı kurulamadı: {e}")));
                yenile();
                return;
            }
        }
    };

    for satir in BufReader::new(yanit.into_reader()).lines() {
        let Ok(satir) = satir else { break };
        let Some(veri) = satir.strip_prefix("data:") else { continue };
        let veri = veri.trim();
        if veri == "[DONE]" { break; }
        let Ok(j) = serde_json::from_str::<Value>(veri) else { continue };
        let mut metin = String::new();
        if gemini {
            if let Some(parts) = j["candidates"][0]["content"]["parts"].as_array() {
                for p in parts {
                    if p["thought"].as_bool() == Some(true) { continue; }
                    if let Some(t) = p["text"].as_str() { metin.push_str(t); }
                }
            }
        } else if let Some(t) = j["choices"][0]["delta"]["content"].as_str() {
            metin.push_str(t);
        }
        if !metin.is_empty() {
            let _ = tx.send(Olay::Parca(metin));
            yenile();
        }
    }
    let _ = tx.send(Olay::Bitti);
    yenile();
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
