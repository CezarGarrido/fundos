use egui::{Align2, Frame, Grid, Ui, Vec2};
use polars::prelude::*;

pub struct AssetDetail {
    pub asset: DataFrame,
    pub open_window: bool,
}

impl AssetDetail {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Detalhes")
            .resizable(false)
            .collapsible(false)
            .default_width(550.0)
            .max_width(550.0)
            .max_height(600.0)
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
            .open(&mut self.open_window)
            .show(ui.ctx(), |ui| show_ui(self.asset.clone(), ui));
    }
}

pub fn show_ui(df: DataFrame, ui: &mut Ui) {
    Frame::none().inner_margin(5.0).show(ui, |ui| {
        show_dataframe(df, ui);
    });
}

fn show_dataframe(df: DataFrame, ui: &mut Ui) {
    let n_rows = df.height();
    let cols = df.get_columns();

    egui::ScrollArea::vertical().show(ui, |ui| {
        Grid::new("data_grid")
            .striped(true)
            .max_col_width(ui.available_width() / 2.0)
            .min_col_width(ui.available_width() / 2.0)
            .show(ui, |ui| {
                ui.label("Campo");
                ui.label("Valor");
                ui.end_row();
                for row in 0..n_rows {
                    for col in cols {
                        let field_name = col.name();
                        let value = col.get(row).unwrap();
                        let value_str = if value.is_nested_null() {
                            "N/A".to_string() // Substitua "N/A" pela string que preferir para representar valores nulos
                        } else {
                            let value_str = if let Some(value_str) = value.get_str() {
                                value_str.to_string()
                            } else {
                                value.to_string()
                            };
                            value_str
                        };

                        ui.label(field_name);
                        ui.label(value_str);
                        ui.end_row();
                    }
                }
            });
    });
}
