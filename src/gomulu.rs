//! Vosk DLL'leri exe'nin içine gömülüdür; ilk açılışta %APPDATA%\\EDITH\\bin klasörüne çıkarılır.
use crate::ayarlar::klasor;
use std::io::Read;
use std::path::PathBuf;

static VOSK_ZIP: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/vosk.zip"));

pub fn vosk_yolu() -> Result<PathBuf, String> {
    let dizin = klasor().join("bin");
    std::fs::create_dir_all(&dizin).map_err(|e| e.to_string())?;
    let dll = dizin.join("libvosk.dll");
    let isaret = dizin.join("surum.txt");
    let etiket = VOSK_ZIP.len().to_string();
    if dll.exists() && std::fs::read_to_string(&isaret).map(|s| s == etiket).unwrap_or(false) { return Ok(dll); }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(VOSK_ZIP)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut g = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(ad) = g.enclosed_name().and_then(|p| p.file_name().map(|f| f.to_owned())) else { continue };
        if g.is_dir() { continue; }
        let mut b = vec![];
        g.read_to_end(&mut b).map_err(|e| e.to_string())?;
        std::fs::write(dizin.join(ad), b).map_err(|e| format!("Vosk dosyası yazılamadı: {e}"))?;
    }
    let _ = std::fs::write(&isaret, etiket);
    Ok(dll)
}
