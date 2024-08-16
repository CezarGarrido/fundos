use eframe::egui;

pub struct About {
    pub show: bool,
}

impl About {
    pub fn new() -> Self {
        Self { show: false }
    }

    pub fn open(&mut self, value: bool) {
        self.show = value;
    }
}

impl About {
    pub fn show(&mut self, ui: &egui::Ui) {
        egui::Window::new("Sobre")
            .resizable(false)
            .collapsible(false)
            .default_width(550.0)
            .max_width(550.0)
            .max_height(600.0)
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, 150.0))
            .open(&mut self.show)
            .show(ui.ctx(), |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.set_width(ui.available_width());
                    egui::Grid::new("app_about_info")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .show(ui, |ui| {
                            ui.monospace("Nome");
                            ui.monospace("Analisador de Fundos de Investimentos");
                            ui.end_row();

                            ui.monospace("Versão");
                            ui.monospace("V0.0.1");
                            ui.end_row();

                            ui.monospace("Autor");
                            ui.monospace("Cezar Garrido Britez");
                            ui.end_row();
                        });
                });
            });
    }
}
