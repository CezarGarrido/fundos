#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use std::env;

    env::set_var("RUST_LOG", "debug");
    egui_logger::builder()
        .init()
        .expect("Error initializing logger");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(true)
            .with_maximized(true)
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([200.0, 100.0])
            .with_resizable(true)
            .with_taskbar(true)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Fundos",
        native_options,
        Box::new(|cc| Ok(Box::new(fundos_frontend::TemplateApp::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use web_sys::wasm_bindgen::JsCast;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("no window or document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("no canvas element");
        let canvas: web_sys::HtmlCanvasElement = canvas
            .dyn_into()
            .expect("element is not a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(fundos_frontend::TemplateApp::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
