pub fn show(ui: &mut egui::Ui) {
    let height = 50.0;
    let ctx = ui.ctx().clone();

    // Solicita repintar continuamente para manter as animações fluidas
    ctx.request_repaint();

    let time = ui.input(|i| i.time);
    let pulse = (time * 3.5).sin() as f32; // -1.0 a 1.0
    let scale = 1.0 + pulse * 0.08;

    let status = if let Ok(guard) = crate::provider::cvm::fund::LOADING_STATUS.read() {
        guard.clone()
    } else {
        "Carregando...".to_string()
    };

    ui.vertical_centered(|ui| {
        ui.add_space(30.0);

        // Ícone com pulso de escala e cor azul premium elegante
        let rect = ui
            .label(
                egui::RichText::new(egui_phosphor::regular::CHART_BAR)
                    .color(egui::Color32::from_rgb(90, 160, 250))
                    .size(height * scale),
            )
            .rect;

        // Spinner girando com glow ao redor do ícone
        egui::Spinner::new()
            .color(egui::Color32::from_rgb(140, 200, 255))
            .paint_at(ui, rect.expand(12.0 * scale));

        ui.add_space(20.0);

        // Texto de status com cor legível e adaptável ao tema
        let text_color = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(200, 215, 245) // Prateado-azul suave e legível no tema escuro
        } else {
            egui::Color32::from_rgb(30, 45, 70) // Azul marinho escuro com excelente contraste no tema claro
        };

        ui.label(
            egui::RichText::new(status)
                .color(text_color)
                .font(egui::FontId::proportional(15.0)),
        );

        ui.add_space(10.0);
    });
}
