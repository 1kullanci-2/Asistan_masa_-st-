use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::time::Duration;

#[derive(Clone)]
pub struct Arac {
    pub ad: String,
    pub aciklama: String,
    pub sema: Value,
}

pub struct Mcp {
    pub ad: String,
    pub url: String,
    pub baslik: String,
    pub oturum: Option<String>,
    pub araclar: Vec<Arac>,
    pub bagli: bool,
    pub hata: String,
    sayac: u64,
}

impl Mcp {
    pub fn yeni(ad: &str, url: &str, baslik: &str) -> Self {
        Self { ad: ad.into(), url: url.into(), baslik: baslik.into(), oturum: None, araclar: vec![], bagli: false, hata: String::new(), sayac: 0 }
    }

    fn rpc(&mut self, yontem: &str, params: Value, bildirim: bool) -> Result<Value, String> {
        self.sayac += 1;
        let id = self.sayac;
        let mut govde = json!({"jsonrpc": "2.0", "method": yontem, "params": params});
        if !bildirim { govde["id"] = json!(id); }
        let ajan = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(60)).build();
        let mut i = ajan.post(&self.url).set("Content-Type", "application/json").set("Accept", "application/json, text/event-stream");
        if let Some((a, d)) = self.baslik.split_once(':') { i = i.set(a.trim(), d.trim()); }
        if let Some(o) = &self.oturum { i = i.set("Mcp-Session-Id", o); }
        let y = i.send_json(govde).map_err(|e| e.to_string())?;
        if let Some(o) = y.header("mcp-session-id") { self.oturum = Some(o.to_string()); }
        if bildirim { return Ok(Value::Null); }
        let sse = y.header("content-type").map(|c| c.contains("event-stream")).unwrap_or(false);
        let j: Value = if sse {
            let mut bulunan = Value::Null;
            for l in BufReader::new(y.into_reader()).lines().flatten() {
                if let Some(v) = l.strip_prefix("data:") {
                    if let Ok(j) = serde_json::from_str::<Value>(v.trim()) {
                        if j["id"] == json!(id) { bulunan = j; break; }
                    }
                }
            }
            bulunan
        } else {
            y.into_json().map_err(|e| e.to_string())?
        };
        if let Some(e) = j.get("error") { return Err(e["message"].as_str().unwrap_or("MCP hatası").to_string()); }
        Ok(j["result"].clone())
    }

    pub fn baglan(&mut self) -> Result<(), String> {
        self.oturum = None;
        let r = self.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "EDITH", "version": "0.4"}}), false);
        if let Err(e) = r { self.bagli = false; self.hata = e.clone(); return Err(e); }
        let _ = self.rpc("notifications/initialized", json!({}), true);
        match self.rpc("tools/list", json!({}), false) {
            Ok(r) => {
                self.araclar = r["tools"].as_array().map(|a| a.iter().map(|t| Arac {
                    ad: t["name"].as_str().unwrap_or("").into(),
                    aciklama: t["description"].as_str().unwrap_or("").into(),
                    sema: t["inputSchema"].clone(),
                }).collect()).unwrap_or_default();
                self.bagli = true;
                self.hata.clear();
                Ok(())
            }
            Err(e) => { self.bagli = false; self.hata = e.clone(); Err(e) }
        }
    }

    pub fn cagir(&mut self, ad: &str, args: Value) -> Result<String, String> {
        let p = json!({"name": ad, "arguments": args});
        let r = match self.rpc("tools/call", p.clone(), false) {
            Ok(r) => r,
            Err(_) => {
                self.baglan()?;
                self.rpc("tools/call", p, false)?
            }
        };
        let m: Vec<String> = r["content"].as_array().map(|a| a.iter().filter_map(|c| c["text"].as_str().map(String::from)).collect()).unwrap_or_default();
        Ok(if m.is_empty() { r.to_string() } else { m.join("\n") })
    }
}

/// Gemini'nin kabul ettiği şema anahtarları dışındakileri atar.
pub fn sema_temizle(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut n = serde_json::Map::new();
            for (k, x) in o {
                match k.as_str() {
                    "type" | "description" | "enum" | "required" | "nullable" | "format" => { n.insert(k.clone(), x.clone()); }
                    "items" => { n.insert(k.clone(), sema_temizle(x)); }
                    "properties" => {
                        if let Value::Object(p) = x {
                            n.insert(k.clone(), Value::Object(p.iter().map(|(a, b)| (a.clone(), sema_temizle(b))).collect()));
                        }
                    }
                    _ => {}
                }
            }
            Value::Object(n)
        }
        _ => v.clone(),
    }
}

/// Ayardaki bağlantı listesinden yeniden kurar ve arka planda bağlanır.
pub fn yenile(a: &std::sync::Arc<std::sync::Mutex<Vec<Mcp>>>, liste: &[crate::ayarlar::McpAyar]) {
    let yeni: Vec<Mcp> = liste.iter().map(|m| Mcp::yeni(&m.ad, &m.url, &m.baslik)).collect();
    let n = yeni.len();
    *a.lock().unwrap() = yeni;
    let a = a.clone();
    std::thread::spawn(move || {
        for i in 0..n {
            let mut k = a.lock().unwrap();
            if let Some(m) = k.get_mut(i) { let _ = m.baglan(); }
        }
    });
}
