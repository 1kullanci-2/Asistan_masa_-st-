//! İlk kurulum: Vosk modellerini indirir ve açar.
use crate::dinleme::{modeller_dizini, Bildir, EN_MODEL, TR_MODEL};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Ilerleme {
    pub binde: AtomicU32,
    pub bitti: AtomicBool,
    pub hata: Mutex<String>,
}

fn indir(ad: &str, agirlik: f32, taban: f32, il: &Arc<Ilerleme>, bildir: &Bildir) -> Result<(), String> {
    let dizin = modeller_dizini();
    if dizin.join(ad).exists() { il.binde.store(((taban + agirlik) * 1000.0) as u32, Ordering::Relaxed); return Ok(()); }
    let y = ureq::get(&format!("https://alphacephei.com/vosk/models/{ad}.zip")).call().map_err(|e| format!("İndirilemedi: {e}"))?;
    let toplam: f32 = y.header("content-length").and_then(|s| s.parse().ok()).unwrap_or(40_000_000.0);
    let yol = dizin.join(format!("{ad}.zip"));
    let mut dosya = std::fs::File::create(&yol).map_err(|e| e.to_string())?;
    let mut o = y.into_reader();
    let mut tampon = vec![0u8; 64 * 1024];
    let mut alinan = 0f32;
    loop {
        let n = o.read(&mut tampon).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        dosya.write_all(&tampon[..n]).map_err(|e| e.to_string())?;
        alinan += n as f32;
        il.binde.store(((taban + agirlik * 0.9 * (alinan / toplam).min(1.0)) * 1000.0) as u32, Ordering::Relaxed);
        (bildir.as_ref())();
    }
    drop(dosya);
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&yol).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut g = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = g.enclosed_name() else { continue };
        let hedef = dizin.join(rel);
        if g.is_dir() { let _ = std::fs::create_dir_all(&hedef); continue; }
        if let Some(p) = hedef.parent() { let _ = std::fs::create_dir_all(p); }
        let mut f = std::fs::File::create(&hedef).map_err(|e| e.to_string())?;
        std::io::copy(&mut g, &mut f).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(&yol);
    il.binde.store(((taban + agirlik) * 1000.0) as u32, Ordering::Relaxed);
    Ok(())
}

pub fn basla(bildir: Bildir) -> Arc<Ilerleme> {
    let il = Arc::new(Ilerleme::default());
    let i2 = il.clone();
    std::thread::spawn(move || {
        let s = indir(TR_MODEL, 0.47, 0.0, &i2, &bildir).and_then(|_| indir(EN_MODEL, 0.5, 0.47, &i2, &bildir));
        match s {
            Ok(_) => { i2.binde.store(1000, Ordering::Relaxed); i2.bitti.store(true, Ordering::Relaxed); }
            Err(e) => *i2.hata.lock().unwrap() = e,
        }
        (bildir.as_ref())();
    });
    il
}
