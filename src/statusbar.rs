use super::app::TemplateApp;
use eframe::egui::Context;
use eframe::Frame;
use egui::Layout;

impl TemplateApp {
    pub fn show_statusbar(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::global_theme_preference_switch(ui);
                ui.add_space(5.0);
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = ui
                        .small_button(egui_phosphor::regular::WARNING)
                        .on_hover_ui(|ui| {
                            ui.label("Log do Sistema");
                        });

                    if btn.clicked() {
                        self.open_logs = !self.open_logs;
                    }
                });
            });
        });
        self.show_logs(ui.ctx());
    }

    fn show_logs(&mut self, ctx: &Context) {
        egui::Window::new("Log do Sistema")
            .open(&mut self.open_logs)
            .show(ctx, |ui| {
                // draws the actual logger ui
                egui_logger::logger_ui().show(ui);
            });
    }
}
