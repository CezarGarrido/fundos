use egui::Align2;
use egui_toast::Toasts;
use std::sync::LazyLock;

/// Spawn an async future, abstracting over wasm/native.
#[cfg(target_arch = "wasm32")]
pub fn spawn_future(f: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}

/// On native, spawn a dedicated OS thread with a tokio runtime so reqwest
/// can make HTTP calls without blocking the UI thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_future(f: impl std::future::Future<Output = ()> + 'static + Send) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(f);
    });
}

pub fn to_real(value: f64) -> String {
    let abs_val = value.abs();
    let int_part = abs_val.trunc() as i64;
    let dec_part = (abs_val.fract() * 100.0).round() as u8;

    // Format integer part with dots as thousand separators (Brazilian style)
    let int_str = int_part
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(".");

    let sign = if value < 0.0 { "-" } else { "" };
    if int_str.is_empty() || int_str == "0" {
        format!("{}R$ 0,{:02}", sign, dec_part)
    } else {
        format!("{}R$ {},{:02}", sign, int_str, dec_part)
    }
}

static TOASTS: LazyLock<egui::mutex::Mutex<Toasts>> =
    LazyLock::new(|| {
        egui::mutex::Mutex::new(
            Toasts::new()
                .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0))
                .direction(egui::Direction::BottomUp),
        )
    });

pub fn toaster() -> egui::mutex::MutexGuard<'static, Toasts> {
    TOASTS.lock()
}

pub fn normalize_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            _ => c.to_ascii_lowercase(),
        })
        .collect()
}
