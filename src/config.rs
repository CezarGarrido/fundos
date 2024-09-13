use config::{Config, File as ConfigFile};
use once_cell::sync::Lazy;
use std::sync::RwLock;

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    let res = Config::builder()
        //.add_source(ConfigFile::with_name("./config/default.toml"))
        .add_source(ConfigFile::with_name("./config/config.toml"))
        .build();
    match res {
        Ok(s) => RwLock::new(s),
        Err(err) => {
            log::error!("{}", err);
            RwLock::new(Config::default())
        }
    }
});

/// Get a configuration value from the static configuration object
pub fn get<'a, T: serde::Deserialize<'a>>(key: &str) -> Result<T, config::ConfigError> {
    let config = CONFIG.read().unwrap(); // Acquiring read lock
    config.get::<T>(key)
}
