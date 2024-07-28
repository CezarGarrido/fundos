use config::Config;
use once_cell::sync::Lazy;
use std::sync::RwLock;

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    let cfg = Config::builder()
        .add_source(config::File::with_name("./config.yaml"))
        .build()
        .unwrap();
    RwLock::new(cfg)
});

/// Get a configuration value from the static configuration object
pub fn get<'a, T: serde::Deserialize<'a>>(key: &str) -> Result<T, config::ConfigError> {
    let config = CONFIG.read().unwrap(); // Acquiring read lock
    config.get::<T>(key)
}
