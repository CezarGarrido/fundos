use super::app::TemplateApp;
use eframe::egui::{widgets::global_dark_light_mode_switch, Context, TopBottomPanel};
use eframe::Frame;

impl TemplateApp {
    pub fn show_statusbar(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                global_dark_light_mode_switch(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}
