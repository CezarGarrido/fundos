use ehttp::Headers;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug, Default, Serialize, Deserialize)]
struct HeadersStore {
    headers: HashMap<String, HashMap<String, String>>,
}

impl HeadersStore {
    fn new() -> Self {
        HeadersStore {
            headers: HashMap::new(),
        }
    }

    fn load() -> io::Result<Self> {
        let mut file = File::open("index.json")?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;
        let headers: HashMap<String, HashMap<String, String>> = serde_json::from_str(&data)?;
        Ok(HeadersStore { headers })
    }

    fn save(&self) -> io::Result<()> {
        let mut file = File::create("index.json")?;
        let data = serde_json::to_string(&self.headers)?;
        file.write_all(data.as_bytes())
    }

    fn get_headers(&self, url: &str) -> Option<&HashMap<String, String>> {
        self.headers.get(url)
    }

    fn set_headers(&mut self, url: &str, headers: HashMap<String, String>) {
        self.headers.insert(url.to_string(), headers);
    }
}

static HEADERS_STORE: Lazy<RwLock<HeadersStore>> =
    Lazy::new(|| RwLock::new(HeadersStore::load().unwrap_or_else(|_| HeadersStore::new())));

// Funções auxiliares para manipulação do cache de cabeçalhos
fn get_last_modified(url: &str) -> Option<String> {
    let store = HEADERS_STORE.read().ok()?;
    store.get_headers(url)?.get("Last-Modified").cloned()
}

fn get_etag(url: &str) -> Option<String> {
    let store = HEADERS_STORE.read().ok()?;
    store.get_headers(url)?.get("ETag").cloned()
}

pub fn get_path(url: &str) -> Option<String> {
    let store = HEADERS_STORE.read().ok()?;
    store.get_headers(url)?.get("Path").cloned()
}

fn store_headers(url: &str, headers: HashMap<String, String>) -> io::Result<()> {
    let mut store = HEADERS_STORE.write().unwrap();
    store.set_headers(url, headers);
    store.save()
}

// Função que retorna os cabeçalhos de cache (If-Modified-Since, ETag)
pub fn get_cached_headers(url: &str) -> ehttp::Headers {
    let mut headers = ehttp::Headers::default();

    if let Some(last_modified) = get_last_modified(url) {
        headers.insert("If-Modified-Since", last_modified);
    }
    if let Some(etag) = get_etag(url) {
        headers.insert("If-None-Match", etag);
    }

    headers
}

// Função que atualiza o cache com novos cabeçalhos (Last-Modified, ETag)
pub fn update_cache(url: &str, path: PathBuf, response_headers: Headers) -> io::Result<()> {
    let mut headers: HashMap<String, String> = HashMap::new();

    if let Some(last_modified) = response_headers.get("Last-Modified") {
        headers.insert("Last-Modified".to_string(), last_modified.to_string());
    }
    if let Some(etag) = response_headers.get("ETag") {
        headers.insert("ETag".to_string(), etag.to_string());
    }

    headers.insert("Path".to_string(), path.display().to_string());

    store_headers(url, headers)
}
