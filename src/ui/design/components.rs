use super::colors::ThemeColors;
use egui::{Color32, CornerRadius, Frame, Margin, Response, RichText, Ui};

pub struct Components;

impl Components {
    /// Renders a universal standard Card with optional padding
    pub fn card<R>(
        ui: &mut Ui,
        dark: bool,
        inner_margin: i8,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        Frame::NONE
            .fill(ThemeColors::card_bg(dark))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(inner_margin))
            .show(ui, add_contents)
            .inner
    }

    /// Primary solid button with Accent color
    pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
        let btn = egui::Button::new(
            RichText::new(label)
                .size(13.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(ThemeColors::accent())
        .corner_radius(CornerRadius::same(6));
        ui.add(btn)
    }

    /// Ghost button used for period selectors and secondary actions
    pub fn ghost_button(ui: &mut Ui, label: &str, dark: bool) -> Response {
        let btn = egui::Button::new(
            RichText::new(label)
                .size(12.0)
                .strong()
                .color(ThemeColors::text_primary(dark)),
        )
        .fill(Color32::TRANSPARENT)
        .frame(false);
        ui.add(btn)
    }

    /// Colorful rounded badge for status tags
    pub fn badge(ui: &mut Ui, text: &str, text_color: Color32, bg_color: Color32) {
        Frame::NONE
            .fill(bg_color)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(RichText::new(text).size(10.0).strong().color(text_color));
            });
    }

    /// Universal table header row builder config
    pub fn configure_table(table: egui_extras::TableBuilder<'_>) -> egui_extras::TableBuilder<'_> {
        table
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .resizable(false)
            .vscroll(false)
    }
}
