use lru::LruCache;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader, BufWriter};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Mutex;

// Definindo a estrutura do link
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Data {
    cnpj: String,
    name: String,
    access_count: u32,
}

impl Data {
    fn new(cnpj: &str, name: &str) -> Self {
        Data {
            cnpj: cnpj.to_string(),
            name: name.to_string(),
            access_count: 1,
        }
    }
}

// Singleton para o cache compartilhado
static CACHE: Lazy<Mutex<LruCache<u64, Data>>> = Lazy::new(|| {
    let size = 10; // Defina o tamanho do cache conforme necessário
    Mutex::new(LruCache::new(NonZeroUsize::new(size).unwrap()))
});

// Estrutura do History
#[derive(Debug, Clone)]
pub struct History {
    filename: PathBuf,
}

impl History {
    pub fn new() -> Self {
        let filename = env::temp_dir().join("cache/history.json");

        History { filename }
    }

    fn calculate_hash<T: Hash>(&self, t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    pub fn load(&self) -> io::Result<()> {
        let file = File::open(&self.filename)?;
        let reader = BufReader::new(file);
        let items: Vec<Data> = serde_json::from_reader(reader)?;

        let mut cache = CACHE.lock().unwrap();
        for item in items {
            let hash = self.calculate_hash(&item.cnpj);
            cache.put(hash, item);
        }

        Ok(())
    }

    pub fn save(&self) -> io::Result<()> {
        let cache = CACHE.lock().unwrap();
        let links: Vec<Data> = cache.iter().map(|(_, link)| link.clone()).collect();
        log::info!("History File {}", self.filename.display());
        let file = File::create(&self.filename)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &links)?;
        Ok(())
    }

    pub fn add(&self, cnpj: String, name: String) {
        let hash = self.calculate_hash(&cnpj.clone());
        let mut cache = CACHE.lock().unwrap();
        if let Some(link) = cache.get_mut(&hash) {
            link.access_count += 1;
        } else {
            cache.put(hash, Data::new(cnpj.as_str(), name.as_str()));
        }
    }

    pub fn get_most_accesseds(&self) -> Vec<(String, String)> {
        let cache = CACHE.lock().unwrap();
        // cache.to_owned()
        cache
            .iter()
            .map(|(_, item)| (item.cnpj.clone(), item.name.clone()))
            .collect()
    }
}
