use egui_notify::Toasts;

static TOASTS: once_cell::sync::Lazy<egui::mutex::Mutex<Toasts>> =
    once_cell::sync::Lazy::new(|| egui::mutex::Mutex::new(Toasts::default().with_anchor(egui_notify::Anchor::BottomRight)));

pub fn toaster() -> egui::mutex::MutexGuard<'static, Toasts> {
    TOASTS.lock()
}
