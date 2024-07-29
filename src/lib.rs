#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod config;
mod history;
pub mod logger;
mod message;
mod provider;
mod statusbar;
mod ui;
mod util;
pub use app::TemplateApp;
