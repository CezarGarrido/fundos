use super::app::TemplateApp;
use crate::logger::LOG_MESSAGES;
use eframe::egui::{Context, TopBottomPanel};
use eframe::Frame;
use egui::{ScrollArea, Window};

impl TemplateApp {
    pub fn show_statusbar(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let log_messages = LOG_MESSAGES.lock().unwrap();

                ui.horizontal(|ui| {
                    if ui
                        .button(format!(
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
                ui.horizontal(|ui| {
                    ui.label(self.status.to_string());
                });

                ui.add_space(5.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.button(format!("{}", egui_phosphor::regular::BELL));
                });
            });
        });
        self.show_messages(ctx);
    }

    fn show_messages(&mut self, ctx: &Context) {
        Window::new("Avisos")
            .open(&mut self.open_logs)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2::new(-10.0, -150.0))
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
