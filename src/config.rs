use config::{Config, File};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use std::sync::mpsc::channel;
use std::{path::Path, sync::RwLock, time::Duration};
use tokio::sync::mpsc::{self};

use crate::message::Message;

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    let mut settings = Config::default();
    settings
        .merge(File::with_name("./config/default.yaml"))
        .unwrap();

    settings
        .merge(File::with_name("./config/config.yaml"))
        .unwrap();

    RwLock::new(settings)
});

pub static SCHEMA: Lazy<RwLock<Config>> = Lazy::new(|| {
    let cfg = Config::builder()
        .add_source(config::File::with_name("./config/schema.yaml"))
        .build()
        .unwrap();
    RwLock::new(cfg)
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

/// Save the configuration to a YAML file
pub fn save_config() -> Result<(), std::io::Error> {
    let config = CONFIG.read().unwrap(); // Acquiring read lock
    let updated_yaml = serde_yaml::to_string(
        &config
            .clone()
            .try_deserialize::<serde_yaml::Value>()
            .unwrap(),
    )
    .unwrap();
    std::fs::write("./config/config.yaml", updated_yaml)
}

/// Convert the entire configuration to a YAML string
pub fn config_to_string() -> Result<String, serde_yaml::Error> {
    let config = CONFIG.read().unwrap(); // Acquiring read lock
    let yaml_string = serde_yaml::to_string(
        &config
            .clone()
            .try_deserialize::<serde_yaml::Value>()
            .unwrap(),
    )?;
    Ok(yaml_string)
}

pub fn config_to_value() -> Result<serde_yaml::Value, serde_yaml::Error> {
    let config = CONFIG.read().unwrap(); // Acquiring read lock
    let yaml_string = serde_yaml::to_value(
        &config
            .clone()
            .try_deserialize::<serde_yaml::Value>()
            .unwrap(),
    )?;
    Ok(yaml_string)
}

pub fn schema_to_value() -> Result<serde_yaml::Value, serde_yaml::Error> {
    let config = SCHEMA.read().unwrap(); // Acquiring read lock
    let yaml_string = serde_yaml::to_value(
        &config
            .clone()
            .try_deserialize::<serde_yaml::Value>()
            .unwrap(),
    )?;
    Ok(yaml_string)
}

pub fn watch(ctx: &egui::Context, sender: mpsc::UnboundedSender<Message>) {
    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher: RecommendedWatcher = Watcher::new(
        tx,
        notify::Config::default().with_poll_interval(Duration::from_secs(3)),
    )
    .unwrap();

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher
        .watch(
            Path::new("./config/config.yaml"),
            RecursiveMode::NonRecursive,
        )
        .unwrap();

    // This is a simple loop, but you may want to use more complex logic here,
    // for example to handle I/O.
    loop {
        match rx.recv() {
            Ok(Ok(Event {
                kind: notify::event::EventKind::Modify(_),
                ..
            })) => {
                println!("refreshing configuration ...");
                CONFIG.write().unwrap().refresh().unwrap();
                let _ = sender.send(Message::RefreshConfig);
                ctx.request_repaint();
            }

            Err(e) => println!("watch error: {:?}", e),
            _ => {
                // Ignore event
            }
        }
    }
}
