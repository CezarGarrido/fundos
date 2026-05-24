use crate::ui::design::{Scale, ThemeColors, Typography};

pub fn show(ui: &mut egui::Ui, text: &str) {
    show_custom(ui, text, egui_phosphor::regular::CHART_BAR);
}

pub fn show_custom(ui: &mut egui::Ui, text: &str, icon: &str) {
    let ctx = ui.ctx().clone();
    ctx.request_repaint();

    let time = ui.input(|i| i.time);
    let dark = ui.visuals().dark_mode;
    let icon_color = ThemeColors::accent();

    ui.vertical_centered(|ui| {
        ui.add_space(30.0);

        // Ícone fixo (sem pulsar — o spinner já fornece a animação)
        ui.label(
            egui::RichText::new(icon)
                .color(icon_color)
                .size(Scale::ICON_JUMBO),
        );

        ui.add_space(10.0);

        // Spinner principal com tamanho fixo, sem paint_at oscilante
        ui.add(egui::Spinner::new().color(icon_color).size(Scale::ICON_SPINNER));

        ui.add_space(16.0);

        // Texto com reticências animadas (progressão contínua)
        let dots = match (time * 3.0) as usize % 3 {
            0 => ".",
            1 => "..",
            _ => "...",
        };
        ui.label(Typography::heading_3(
            format!("{} {}", text, dots),
            dark,
        ));

        ui.add_space(10.0);
    });
}

pub fn show_custom_small(ui: &mut egui::Ui, text: &str) {
    let ctx = ui.ctx().clone();
    ctx.request_repaint();

    let time = ui.input(|i| i.time);
    let dark = ui.visuals().dark_mode;
    let icon_color = ThemeColors::accent();

    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().color(icon_color).size(Scale::ICON_SMALL));
        ui.add_space(6.0);
        let dots = match (time * 3.0) as usize % 3 {
            0 => ".",
            1 => "..",
            _ => "...",
        };
        ui.label(Typography::label_muted(
            format!("{} {}", text, dots),
            dark,
        ));
    });
}
