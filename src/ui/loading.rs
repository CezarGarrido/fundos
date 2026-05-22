use crate::ui::design::{ThemeColors, Typography};

pub fn show(ui: &mut egui::Ui) {
    let status = if let Ok(guard) = crate::provider::cvm::fund::LOADING_STATUS.read() {
        guard.clone()
    } else {
        "Carregando...".to_string()
    };
    show_custom(ui, &status, egui_phosphor::regular::CHART_BAR);
}

pub fn show_custom(ui: &mut egui::Ui, text: &str, icon: &str) {
    let height = 50.0;
    let ctx = ui.ctx().clone();

    // Solicita repintar continuamente para manter as animações fluidas
    ctx.request_repaint();

    let time = ui.input(|i| i.time);
    let pulse = (time * 3.5).sin() as f32; // -1.0 a 1.0
    let scale = 1.0 + pulse * 0.08;

    let dark = ui.visuals().dark_mode;
    let icon_color = ThemeColors::accent();

    ui.vertical_centered(|ui| {
        ui.add_space(30.0);

        // Ícone com pulso de escala e cor azul premium elegante
        let rect = ui
            .label(
                egui::RichText::new(icon)
                    .color(icon_color)
                    .size(height * scale),
            )
            .rect;

        // Spinner girando com glow ao redor do ícone
        egui::Spinner::new()
            .color(icon_color)
            .paint_at(ui, rect.expand(12.0 * scale));

        ui.add_space(20.0);

        ui.label(Typography::heading_3(text, dark));

        ui.add_space(10.0);
    });
}

pub fn show_custom_small(ui: &mut egui::Ui, text: &str) {
    let ctx = ui.ctx().clone();
    ctx.request_repaint();

    let dark = ui.visuals().dark_mode;
    let icon_color = ThemeColors::accent();

    ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        egui::Spinner::new()
            .color(icon_color)
            .paint_at(ui, ui.cursor().expand(8.0));
        ui.spinner();
        ui.add_space(4.0);
        ui.label(Typography::label_muted(text, dark));
        ui.add_space(8.0);
    });
}
