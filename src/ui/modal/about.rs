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
                ui.vertical_centered(|ui| {
                    ui.heading("");
                    ui.label(
                        egui::RichText::new("Fundos 1.0").font(egui::FontId::proportional(32.0)),
                    );
                    ui.label(
                        egui::RichText::new("Fundos - ").font(egui::FontId::proportional(20.0)),
                    );
                    ui.separator();
                    ui.hyperlink("https://egui.rs/");
                    //ui.hyperlink("https://trunkrs.dev");
                });
            });
    }
}
