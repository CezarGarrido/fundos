#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> eframe::Result<()> {
    use std::env;

    env::set_var("RUST_LOG", "debug");
    //env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    egui_logger::init().expect("Error initializing logger");

    let native_options = eframe::NativeOptions {
        default_theme: eframe::Theme::Light,
        viewport: egui::ViewportBuilder::default()
            .with_decorations(true)
            .with_maximized(true)
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([200.0, 100.0])
            .with_resizable(true)
            .with_taskbar(true)
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "fundos",
        native_options,
        Box::new(|cc| Box::new(eframe_template::TemplateApp::new(cc))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| Box::new(eframe_template::TemplateApp::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
