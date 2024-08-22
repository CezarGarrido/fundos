use config::{Config, File as ConfigFile};
use once_cell::sync::Lazy;
use std::io::{Read, Write};
use std::path::Path;
use std::{fs::File, sync::RwLock};

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    let mut settings = Config::default();
    settings
        .merge(ConfigFile::with_name("./config/default.toml"))
        .unwrap();

    settings
        .merge(ConfigFile::with_name("./config/config.toml"))
        .unwrap();

    RwLock::new(settings)
});

/// Get a configuration value from the static configuration object
pub fn get<'a, T: serde::Deserialize<'a>>(key: &str) -> Result<T, config::ConfigError> {
    let config = CONFIG.read().unwrap(); // Acquiring read lock
    config.get::<T>(key)
}

/// Set a configuration value in the static configuration object
pub fn set(key: &str, value: &str) -> Result<(), config::ConfigError> {
    let mut config = CONFIG.write().unwrap(); // Acquiring write lock
    config.set_once(key, value.into())?;
    Ok(())
}

/// Get the entire configuration as a TOML string
pub fn get_string() -> Result<String, toml::ser::Error> {
    let config = CONFIG.read().unwrap();
    let config_value = config.clone().try_deserialize::<toml::Value>().unwrap();

    toml::to_string(&config_value) // Convert to a TOML string
}

pub fn has_changes() -> bool {
    // let old_cfg = config_to_value().unwrap();
    //let new_cfg = load_config().unwrap();
    //new_cfg != old_cfg
    true
}

pub fn load_as_string() -> String {
    let mut buffer = String::new();
    File::open("./config/config.toml")
        .unwrap()
        .read_to_string(&mut buffer)
        .unwrap();

    buffer
}

// Adiciona um método para salvar o código no arquivo
pub fn save_code(code: String) -> std::io::Result<()> {
    let path = Path::new("./config/config.toml");
    let mut file = File::create(path)?;
    file.write_all(code.as_bytes())?;
    Ok(())
}
