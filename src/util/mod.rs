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
