use lru::LruCache;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader, BufWriter};
use std::num::NonZeroUsize;
use std::sync::Mutex;

// Definindo a estrutura do link
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Data {
    url: String,
    access_count: u32,
}

impl Data {
    fn new(url: &str) -> Self {
        Data {
            url: url.to_string(),
            access_count: 1,
        }
    }
}

// Singleton para o cache compartilhado
static CACHE: Lazy<Mutex<LruCache<u64, Data>>> = Lazy::new(|| {
    let size = 7; // Defina o tamanho do cache conforme necessário
    Mutex::new(LruCache::new(NonZeroUsize::new(size).unwrap()))
});

// Estrutura do History
#[derive(Debug, Clone)]
pub struct History {
    filename: String,
}

impl History {
    pub fn new() -> Self {
        let filename = "./dataset/history.json";
        History {
            filename: filename.to_string(),
        }
    }

    fn calculate_hash<T: Hash>(&self, t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    pub fn load(&self) -> io::Result<()> {
        let file = File::open(&self.filename)?;
        let reader = BufReader::new(file);
        let links: Vec<Data> = serde_json::from_reader(reader)?;

        let mut cache = CACHE.lock().unwrap();
        for link in links {
            let hash = self.calculate_hash(&link.url);
            cache.put(hash, link);
        }

        Ok(())
    }

    pub fn save(&self) -> io::Result<()> {
        let cache = CACHE.lock().unwrap();
        let links: Vec<Data> = cache.iter().map(|(_, link)| link.clone()).collect();
        let file = File::create(&self.filename)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &links)?;
        Ok(())
    }

    pub fn add(&self, url: String) {
        let hash = self.calculate_hash(&url.clone());
        let mut cache = CACHE.lock().unwrap();
        if let Some(link) = cache.get_mut(&hash) {
            link.access_count += 1;
        } else {
            cache.put(hash, Data::new(url.as_str()));
        }
    }

    pub fn get_most_accesseds(&self) -> Vec<String> {
        let cache = CACHE.lock().unwrap();
        cache.iter().map(|(_, link)| link.url.clone()).collect()
    }
}
