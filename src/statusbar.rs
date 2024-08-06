use crate::logger::LOG_MESSAGES;

use super::app::TemplateApp;
use eframe::egui::{Context, TopBottomPanel};
use eframe::Frame;
use egui::{global_dark_light_mode_buttons, ScrollArea, Window};

impl TemplateApp {
    pub fn show_statusbar(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.horizontal(|ui| {
                    let log_messages = LOG_MESSAGES.lock().unwrap();

                    if ui
                        .small_button(format!(
                            "{} {}",
                            egui_phosphor::regular::WARNING,
                            log_messages.len(),
                        ))
                        .clicked()
                    {
                        self.open_logs = !self.open_logs;
                    }
                });
                ui.add_space(5.0);
                egui::warn_if_debug_build(ui);
                ui.add_space(5.0);
                global_dark_light_mode_buttons(ui);
            });
        });
        self.show_messages(ctx);
    }

    fn show_messages(&mut self, ctx: &Context) {
        Window::new("Avisos")
            .open(&mut self.open_logs)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    let log_messages = LOG_MESSAGES.lock().unwrap();
                    for (level, msg, _target) in log_messages.iter() {
                        ui.horizontal(|ui| {
                            match level {
                                log::Level::Warn => {
                                    ui.colored_label(egui::Color32::YELLOW, level.as_str())
                                }
                                log::Level::Error => {
                                    ui.colored_label(egui::Color32::RED, level.as_str())
                                }
                                _ => ui.monospace(level.as_str()),
                            };
                            ui.label(msg);
                        });
                    }
                });
            });
    }
}
