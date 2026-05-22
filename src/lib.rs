#![warn(clippy::all, rust_2018_idioms)]

pub mod analytics;
mod app;
pub mod backtest;
pub mod config;
mod history;
mod message;
mod provider;
mod statusbar;
mod ui;
mod util;
pub use app::TemplateApp;
