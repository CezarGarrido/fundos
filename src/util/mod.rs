use egui::Align2;
use egui_toast::Toasts;

pub fn to_real(value: f64) -> Result<currency_rs::Currency, currency_rs::CurrencyErr> {
    let otp = currency_rs::CurrencyOpts::new()
        .set_separator(".")
        .set_decimal(",")
        .set_symbol("R$ ");

    Ok(currency_rs::Currency::new_float(value, Some(otp)))
}

static TOASTS: once_cell::sync::Lazy<egui::mutex::Mutex<Toasts>> =
    once_cell::sync::Lazy::new(|| {
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
