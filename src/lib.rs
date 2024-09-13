#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod config;
mod history;
mod message;
mod provider;
mod statusbar;
mod ui;
mod util;
pub use app::TemplateApp;
