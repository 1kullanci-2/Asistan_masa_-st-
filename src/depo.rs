use crate::ayarlar::klasor;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Mesaj {
    /// "k" kullanıcı, "a" asistan, "n" not
    pub k: String,
    pub m: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Sohbet {
    pub id: String,
    pub baslik: String,
    pub guncelleme: i64,
    pub model: String,
    pub ozet: String,
    pub kapsam: usize,
    pub mesajlar: Vec<Mesaj>,
}

fn dizin() -> std::path::PathBuf {
    let d = klasor().join("sohbetler");
    let _ = std::fs::create_dir_all(&d);
    d
}

impl Sohbet {
    pub fn yeni(model: String) -> Self {
        let t = chrono::Local::now().timestamp_millis();
        Self { id: t.to_string(), baslik: "Yeni sohbet".into(), guncelleme: t, model, ..Default::default() }
    }
    pub fn kaydet(&mut self) {
        self.guncelleme = chrono::Local::now().timestamp_millis();
        if let Ok(s) = serde_json::to_string(self) {
            let _ = std::fs::write(dizin().join(format!("{}.json", self.id)), s);
        }
    }
    pub fn sil(&self) {
        let _ = std::fs::remove_file(dizin().join(format!("{}.json", self.id)));
    }
}

pub fn hepsi() -> Vec<Sohbet> {
    let mut v: Vec<Sohbet> = std::fs::read_dir(dizin())
        .map(|d| d.flatten().filter_map(|e| std::fs::read_to_string(e.path()).ok()).filter_map(|s| serde_json::from_str(&s).ok()).collect())
        .unwrap_or_default();
    v.sort_by_key(|s| -s.guncelleme);
    v
}

const YAKIN: usize = 8;
const TAVAN: usize = 14;

fn kisalt(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n).collect::<String>()) }
}

/// Modele gidecek bağlam: özet + son mesajlar. (rol, metin) — rol "user"/"assistant"
pub fn baglam(s: &Sohbet) -> Vec<(String, String)> {
    let gercek: Vec<&Mesaj> = s.mesajlar.iter().filter(|m| m.k != "n").collect();
    let bas = gercek.len().saturating_sub(TAVAN);
    let mut v = vec![];
    if !s.ozet.is_empty() {
        v.push(("user".to_string(), format!("[Önceki konuşmanın özeti: {}]", s.ozet)));
        v.push(("assistant".to_string(), "Anlaşıldı.".to_string()));
    }
    for m in &gercek[bas..] {
        let (rol, n) = if m.k == "k" { ("user", 500) } else { ("assistant", 700) };
        v.push((rol.to_string(), kisalt(&m.m, n)));
    }
    v
}

/// Eski mesajlar için mekanik özet (model özetleyici sonra eklenecek).
pub fn ozetle(s: &mut Sohbet) {
    let gercek: Vec<&Mesaj> = s.mesajlar.iter().filter(|m| m.k != "n").collect();
    if gercek.len() <= YAKIN + 6 { return; }
    let son = gercek.len() - YAKIN;
    if son <= s.kapsam { return; }
    let mut t = s.ozet.clone();
    for m in &gercek[s.kapsam..son] {
        t.push_str(&format!(" {}: {}", if m.k == "k" { "K" } else { "A" }, kisalt(&m.m, 120)));
    }
    s.ozet = kisalt(t.trim(), 1000);
    s.kapsam = son;
}
