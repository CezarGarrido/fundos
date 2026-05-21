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
        egui::Window::new("Sobre o Fundos")
            .resizable(false)
            .collapsible(false)
            .default_width(380.0)
            .max_width(380.0)
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, 120.0))
            .open(&mut self.show)
            .show(ui.ctx(), |ui| {
                let dark = ui.visuals().dark_mode;
                let muted = if dark {
                    egui::Color32::from_rgb(140, 150, 170)
                } else {
                    egui::Color32::from_rgb(100, 110, 130)
                };
                let accent = egui::Color32::from_rgb(37, 99, 235);

                ui.vertical_centered(|ui| {
                    // ── Ícone + Nome ──────────────────────────────────────
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("📊")
                            .size(42.0),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Fundos")
                            .size(24.0)
                            .strong(),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .size(12.0)
                            .color(muted),
                    );

                    ui.add_space(14.0);

                    // ── Descrição ─────────────────────────────────────────
                    ui.label(
                        egui::RichText::new(
                            "Visualizador de fundos de investimento brasileiros\ncom dados públicos da CVM.",
                        )
                        .size(13.0)
                        .color(muted),
                    );

                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // ── Dados ─────────────────────────────────────────────
                    ui.label(
                        egui::RichText::new("🔗 Dados")
                            .size(12.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Portal de Dados Abertos da CVM")
                            .size(12.0)
                            .color(muted),
                    );
                    ui.hyperlink_to(
                        "dados.cvm.gov.br",
                        "https://dados.cvm.gov.br/",
                    );

                    ui.add_space(14.0);

                    // ── Tecnologia ────────────────────────────────────────
                    ui.label(
                        egui::RichText::new("⚙️ Tecnologia")
                            .size(12.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Rust · egui · Polars")
                            .size(12.0)
                            .color(muted),
                    );

                    ui.add_space(14.0);

                    // ── Autor ─────────────────────────────────────────────
                    ui.label(
                        egui::RichText::new("👤 Autor")
                            .size(12.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_AUTHORS"))
                            .size(12.0)
                            .color(muted),
                    );

                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // ── Rodapé ────────────────────────────────────────────
                    ui.hyperlink_to(
                        egui::RichText::new("🐙 github.com/cezargarrido/fundos")
                            .size(11.0)
                            .color(accent),
                        "https://github.com/cezargarrido/fundos",
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Distribuído sob licença MIT / Apache-2.0")
                            .size(10.0)
                            .color(muted),
                    );
                    ui.add_space(8.0);
                });
            });
    }
}
