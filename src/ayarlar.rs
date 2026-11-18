use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn klasor() -> PathBuf {
    let taban = std::env::var("APPDATA").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| ".".into());
    let p = PathBuf::from(taban).join("EDITH");
    let _ = std::fs::create_dir_all(&p);
    p
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Ayar {
    pub gemini_anahtar: String,
    pub groq_anahtar: String,
    pub cerebras_anahtar: String,
    pub mistral_anahtar: String,
    pub gemini_modeller: String,
    pub groq_modeller: String,
    pub cerebras_modeller: String,
    pub mistral_modeller: String,
    pub eleven_anahtar: String,
    pub ses_sadece_sesli: bool,
    pub gemini_ses: String,
    pub eleven_ses: String,
    pub eleven_model: String,
    pub eleven_kararlilik: f32,
    pub eleven_benzerlik: f32,
    pub ses_sirasi: Vec<String>,
    pub mcp: Vec<McpAyar>,
    pub uyandirma_ekstra: Vec<String>,
    pub cumle_basi: bool,
    pub sesle_kes: bool,
    pub hoparlor_modu: bool,
    pub soru_sorunca_mikrofon: bool,
    pub her_cevaptan_sonra: bool,
    pub takip_s: f32,
    pub sessizlik_s: f32,
    pub sessizlik_kapisi: bool,
    pub ses_sirayla: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct McpAyar {
    pub ad: String,
    pub url: String,
    pub baslik: String,
    pub kapali: Vec<String>,
}

impl Default for Ayar {
    fn default() -> Self {
        Self {
            gemini_anahtar: String::new(),
            groq_anahtar: String::new(),
            cerebras_anahtar: String::new(),
            mistral_anahtar: String::new(),
            gemini_modeller: "gemini-flash-lite-latest,gemini-flash-latest".into(),
            groq_modeller: "openai/gpt-oss-120b".into(),
            cerebras_modeller: "zai-glm-4.7".into(),
            mistral_modeller: "mistral-small-latest".into(),
            eleven_anahtar: String::new(),
            ses_sadece_sesli: true,
            gemini_ses: "Kore".into(),
            eleven_ses: String::new(),
            eleven_model: "eleven_flash_v2_5".into(),
            eleven_kararlilik: 0.5,
            eleven_benzerlik: 0.75,
            ses_sirasi: vec!["gemini".into(), "eleven".into()],
            mcp: vec![],
            uyandirma_ekstra: vec![],
            cumle_basi: true,
            sesle_kes: true,
            hoparlor_modu: true,
            soru_sorunca_mikrofon: true,
            her_cevaptan_sonra: false,
            takip_s: 8.0,
            sessizlik_s: 1.2,
            sessizlik_kapisi: true,
            ses_sirayla: true,
        }
    }
}

impl Ayar {
    pub fn yukle() -> Self {
        std::fs::read_to_string(klasor().join("ayar.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn kaydet(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(klasor().join("ayar.json"), s);
        }
    }
    pub fn anahtar(&self, servis: &str) -> &str {
        match servis {
            "gemini" => &self.gemini_anahtar,
            "groq" => &self.groq_anahtar,
            "cerebras" => &self.cerebras_anahtar,
            _ => &self.mistral_anahtar,
        }
    }
    fn modeller_ham(&self, servis: &str) -> &str {
        match servis {
            "gemini" => &self.gemini_modeller,
            "groq" => &self.groq_modeller,
            "cerebras" => &self.cerebras_modeller,
            _ => &self.mistral_modeller,
        }
    }
    pub fn modeller(&self, servis: &str) -> Vec<String> {
        self.modeller_ham(servis).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).take(2).collect()
    }
    /// Anahtarı olan servislerin "servis|model" listesi.
    pub fn secenekler(&self) -> Vec<String> {
        let mut v = vec![];
        for s in SERVISLER {
            if !self.anahtar(s).trim().is_empty() {
                for m in self.modeller(s) {
                    v.push(format!("{s}|{m}"));
                }
            }
        }
        v
    }
    pub fn dinleme_ayari(&self) -> crate::dinleme::DAyar {
        crate::dinleme::DAyar {
            ekstra: self.uyandirma_ekstra.clone(),
            cumle_basi: self.cumle_basi,
            sesle_kes: self.sesle_kes,
            hoparlor: self.hoparlor_modu,
            takip_s: self.takip_s,
            sessizlik_s: self.sessizlik_s,
            kapi: self.sessizlik_kapisi,
        }
    }
    pub fn varsayilan_model(&self) -> String {
        self.secenekler().into_iter().next().unwrap_or_default()
    }
}

pub const SERVISLER: [&str; 4] = ["gemini", "groq", "cerebras", "mistral"];

pub fn servis_adi(s: &str) -> &'static str {
    match s {
        "gemini" => "Gemini",
        "groq" => "Groq",
        "cerebras" => "Cerebras",
        _ => "Mistral",
    }
}
