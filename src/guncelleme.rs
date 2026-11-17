use crate::ayarlar::klasor;
use std::io::Read;

pub const SURUM: u32 = match option_env!("GITHUB_RUN_NUMBER") {
    Some(s) => {
        let b = s.as_bytes();
        let (mut i, mut n) = (0, 0u32);
        while i < b.len() { n = n * 10 + (b[i] - b'0') as u32; i += 1; }
        n
    }
    None => 0,
};

pub fn surum_metni() -> String { format!("0.4.{SURUM}") }

/// (yeni sürüm numarası, indirme adresi) — yeni sürüm yoksa None
pub fn kontrol() -> Result<Option<(u32, String)>, String> {
    let j: serde_json::Value = ureq::get("https://api.github.com/repos/1kullanci-2/Asistan_masa_-st-/releases/tags/son-surum")
        .set("User-Agent", "edith")
        .call().map_err(|e| format!("Kontrol edilemedi: {e}"))?
        .into_json().map_err(|e| e.to_string())?;
    let not = j["body"].as_str().unwrap_or("");
    let n: u32 = not.split("Sürüm: 0.4.").nth(1).map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).and_then(|s| s.parse().ok()).unwrap_or(0);
    let url = j["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"] == "edith-windows.zip")).and_then(|x| x["browser_download_url"].as_str()).unwrap_or("").to_string();
    Ok(if n > SURUM && !url.is_empty() { Some((n, url)) } else { None })
}

/// Yeni sürümü indirip açar, kopyalama betiğini başlatır. Başarılıysa uygulama kapanmalı.
pub fn kur(url: &str) -> Result<(), String> {
    let d = klasor().join("guncelleme");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("yeni")).map_err(|e| e.to_string())?;
    let mut b = vec![];
    ureq::get(url).set("User-Agent", "edith").call().map_err(|e| e.to_string())?.into_reader().read_to_end(&mut b).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(b)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut g = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = g.enclosed_name() else { continue };
        if g.is_dir() { continue; }
        let mut f = std::fs::File::create(d.join("yeni").join(rel)).map_err(|e| e.to_string())?;
        std::io::copy(&mut g, &mut f).map_err(|e| e.to_string())?;
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let kok = exe.parent().ok_or("yol yok")?;
    let bat = d.join("guncelle.bat");
    std::fs::write(&bat, format!("@echo off\r\ntimeout /t 2 /nobreak >nul\r\n:d\r\nxcopy /y /e \"{}\\*\" \"{}\\\" >nul 2>&1\r\nif errorlevel 1 (\r\ntimeout /t 1 /nobreak >nul\r\ngoto d\r\n)\r\nstart \"\" \"{}\"\r\n", d.join("yeni").display(), kok.display(), exe.display())).map_err(|e| e.to_string())?;
    std::process::Command::new("cmd").args(["/C", "start", "", "/min", &bat.to_string_lossy()]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}
