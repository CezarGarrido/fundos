#![warn(clippy::all, rust_2018_idioms)]

mod app;
mod charts;
mod config;
mod cvm;
mod downloader;
mod history;
mod message;
mod statusbar;
mod tabs;
pub use app::TemplateApp;
