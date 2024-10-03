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
            .default_width(400.0)
            .max_width(400.0)
            .max_height(600.0)
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, 150.0))
            .open(&mut self.show)
            .show(ui.ctx(), |ui| {
                ui.vertical_centered(|ui| {

                    ui.label(
                        egui::RichText::new("Fundos").font(egui::FontId::proportional(32.0)),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Fundos é um programa visualizador de fundos de investimentos brasileiros. Escrito em Rust, compilado para Linux, Mac e Windows.",
                        )
                        .font(egui::FontId::proportional(20.0)),
                    );
                    ui.add_space(5.0);

                    ui.label("Dados obtidos pelo Portal de Dados Abertos da CVM, disponível em:");
                    ui.hyperlink("https://dados.cvm.gov.br/");
                    ui.separator();
                    ui.hyperlink("https://egui.rs/");
                });
            });
    }
}
