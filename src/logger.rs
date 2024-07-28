use log::{Level, Metadata, Record};
use once_cell::sync::Lazy;
use std::sync::Mutex;

pub static LOG_MESSAGES: Lazy<Mutex<Vec<(log::Level, String, String)>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut log_messages = LOG_MESSAGES.lock().unwrap();
            log_messages.push((
                record.level(),
                record.args().to_string(),
                record.target().to_owned(),
            ));
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
}
