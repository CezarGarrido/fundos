#![warn(clippy::all, rust_2018_idioms)]

mod util;
mod app;
mod charts;
pub mod config;
mod cvm;
mod history;
pub mod logger;
mod message;
mod statusbar;
mod tabs;
pub use app::TemplateApp;
