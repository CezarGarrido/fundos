use crate::{cvm, message, util};
use chrono::Datelike;
use egui::{ComboBox, Layout, Sense, TopBottomPanel, Ui};
use egui_extras::{Column, TableBuilder};
use polars::{
    frame::DataFrame,
    lazy::dsl::{col, lit},
    prelude::{IntoLazy, NamedFrom},
    series::Series,
};
use std::collections::HashSet;
use tokio::sync::mpsc::UnboundedSender;

pub struct PortfolioUI {
    pub assets_filter_date: String,
    pub assets_filter_year: String,
    pub assets_filter_month: String,
    pub assets_filter_tp_aplic_selection: std::collections::HashSet<usize>,
    pub pl: DataFrame,
    pub assets: DataFrame,
    pub top_assets: DataFrame,
    pub cnpj: String,
    pub sender: Option<UnboundedSender<message::Message>>,
    pub available_dates: Vec<String>,
}

impl Default for PortfolioUI {
    fn default() -> Self {
        let now = chrono::offset::Utc::now().date_naive();
        let available_dates = cvm::portfolio_available_dates();
        let mut year = now.year().to_string();
        let mut month = now.month().to_string();

        if !available_dates.is_empty() {
            let date = available_dates[0].clone();
            let v: Vec<&str> = date.split('/').collect();
            year = v[0].to_string();
            month = v[1].to_string();
        }

        let assets_filter_date = format!("{}/{}", year, month);

        PortfolioUI {
            cnpj: String::from(""),
            sender: None,
            assets: DataFrame::empty(),
            pl: DataFrame::empty(),
            top_assets: DataFrame::empty(),
            assets_filter_year: year,
            assets_filter_month: month,
            assets_filter_tp_aplic_selection: Default::default(),
            available_dates,
            assets_filter_date,
        }
    }
}

impl PortfolioUI {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(format!(
                                "{} Composição da Carteira",
                                egui_phosphor::regular::WALLET
                            ));
                        });
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.horizontal(|ui| {
                            self.create_date_combobox(ui);
                        });
                    });
                });
                ui.separator();
                self.show_assets_panel(ui);
            });
        });
    }

    fn create_date_combobox(&mut self, ui: &mut egui::Ui) {
        ComboBox::from_label("Selecione a data")
            .selected_text(self.assets_filter_date.to_string())
            .show_ui(ui, |ui| {
                for date in self.available_dates.clone() {
                    let v: Vec<&str> = date.split('/').collect();
                    let year = v[0].to_string();
                    let month = v[1].to_string();
                    let date = format!("{}/{:02}", year, month);
                    if ui
                        .selectable_value(&mut self.assets_filter_date, date.clone(), date)
                        .clicked()
                    {
                        let m = format!("{:02}", month);
                        self.assets_filter_year = year.to_string();
                        self.assets_filter_month = m;

                        let _ = self.sender.clone().unwrap().send(message::Message::Assets(
                            self.cnpj.to_string(),
                            self.assets_filter_year.clone(),
                            self.assets_filter_month.clone(),
                        ));
                    }
                }
            });
    }

    fn show_assets_panel(&mut self, ui: &mut egui::Ui) {
        assets_ui(
            &self.pl,
            &self.assets,
            &self.top_assets,
            &mut self.assets_filter_tp_aplic_selection,
            ui,
        );
    }
}

pub fn assets_ui(
    pl: &DataFrame,
    assets: &DataFrame,
    top_assets: &DataFrame,
    selection: &mut std::collections::HashSet<usize>,
    ui: &mut Ui,
) {
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .default_width(400.0)
        .width_range(200.0..=450.0)
        .show_inside(ui, |ui| {
            ui.add_space(10.0);
            //egui::Frame::none().inner_margin(10.0).show(ui, |ui| {
            let nr_rows = top_assets.height();
            let cols: Vec<&str> = vec!["TP_APLIC", "VL_MERC_POS_FINAL", "VL_PORCENTAGEM_PL"];
            ui.push_id("top_assets", |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    TableBuilder::new(ui)
                        .column(Column::auto().resizable(true).clip(true))
                        .column(Column::auto().at_most(150.0))
                        .column(Column::remainder())
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .resizable(false)
                        .sense(Sense::click())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.label("Aplicação");
                            });
                            header.col(|ui| {
                                ui.label("Posição Final");
                            });
                            header.col(|ui| {
                                ui.label("% Patrim. Líq");
                            });
                        })
                        .body(|body| {
                            body.rows(20.0, nr_rows, |mut row| {
                                let row_index = row.index();
                                row.set_selected(selection.contains(&row_index));
                                for col in &cols {
                                    row.col(|ui| {
                                        if let Ok(column) = top_assets.column(col) {
                                            if let Ok(value) = column.get(row_index) {
                                                if col.contains("VL_PORCENTAGEM_PL") {
                                                    let a =
                                                        value.to_string().parse::<f64>().unwrap();
                                                    ui.colored_label(
                                                        egui::Color32::DARK_GREEN,
                                                        format!("{}%", a),
                                                    );
                                                } else if col.contains("VL_MERC_POS_FINAL") {
                                                    let a =
                                                        value.to_string().parse::<f64>().unwrap();
                                                    let r = util::to_real(a).unwrap();
                                                    ui.weak(r.format());
                                                } else if let Some(value_str) = value.get_str() {
                                                    ui.weak(value_str);
                                                }
                                            }
                                        }
                                    });
                                }
                                toggle_row_selection(selection, row_index, &row.response());
                            });
                        });
                });

                TopBottomPanel::bottom("vl_pl").show_inside(ui, |ui: &mut Ui| {
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.weak("Patrimonio Líquido");
                        pl.column("VL_PATRIM_LIQ")
                            .ok()
                            .and_then(|col| col.get(0).ok())
                            .and_then(|val| val.get_str().map(|s| s.to_string()))
                            .and_then(|value_str| value_str.parse::<f64>().ok())
                            .and_then(|parsed_value| util::to_real(parsed_value).ok())
                            .map(|v| ui.heading(v.format()))
                            .unwrap_or_else(|| ui.label("-"));
                    });
                });
                //});
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.push_id("filter_assets", |ui| {
            let mut filters = Vec::new();
            for r in selection.iter() {
                if let Ok(column) = top_assets.column("TP_APLIC") {
                    if let Ok(value) = column.get(*r) {
                        let v = value.get_str().unwrap();
                        filters.push(v.to_string())
                    }
                }
            }

            let filters_series = Series::new("filters", filters);
            let lf = if filters_series.len() > 0 {
                assets
                    .clone()
                    .lazy()
                    .filter(col("TP_APLIC").is_in(lit(filters_series)))
                    .sort(
                        "VL_PORCENTAGEM_PL",
                        polars::prelude::SortOptions {
                            descending: true,
                            ..Default::default()
                        },
                    )
            } else {
                assets.clone().lazy()
            };

            let filtered_df = lf.collect().unwrap();
            let nr_rows = filtered_df.height();
            let cols: Vec<&str> = vec![
                "TP_APLIC",
                "DS_ATIVO",
                "NM_FUNDO_COTA",
                "VL_MERC_POS_FINAL",
                "VL_PORCENTAGEM_PL",
            ];
            ui.group(|ui| {
                ui.set_min_height(ui.available_height());

                egui::ScrollArea::horizontal().show(ui, |ui| {
                    TableBuilder::new(ui)
                        .column(Column::auto().resizable(true).clip(true))
                        .column(Column::auto().at_least(100.0).resizable(true))
                        .column(Column::auto().at_least(50.0).resizable(true).clip(true))
                        .column(Column::remainder().at_least(50.0).resizable(true))
                        .column(Column::remainder())
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .resizable(false)
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.label("Aplicação");
                            });

                            header.col(|ui| {
                                ui.label("Ativo");
                            });
                            header.col(|ui| {
                                ui.label("Detalhes");
                            });
                            header.col(|ui| {
                                ui.label("Valor");
                            });
                            header.col(|ui| {
                                ui.label("% Patrim. Liq");
                            });
                        })
                        .body(|body| {
                            body.rows(20.0, nr_rows, |mut row| {
                                let row_index = row.index();
                                for col in &cols {
                                    row.col(|ui| {
                                        if let Ok(column) = filtered_df.column(col) {
                                            if let Ok(value) = column.get(row_index) {
                                                if col.contains("VL_PORCENTAGEM_PL") {
                                                    let a =
                                                        value.to_string().parse::<f64>().unwrap();
                                                    ui.label(format!("{}%", a));
                                                } else if let Some(value_str) = value.get_str() {
                                                    if col.contains("VL_MERC_POS_FINAL") {
                                                        let a = value_str
                                                            .to_string()
                                                            .parse::<f64>()
                                                            .unwrap();
                                                        let r = util::to_real(a).unwrap();
                                                        ui.label(r.format());
                                                    } else {
                                                        ui.label(value_str);
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                            });
                        });
                });
            });
        });
    });
}

fn toggle_row_selection(
    selection: &mut HashSet<usize>,
    row_index: usize,
    row_response: &egui::Response,
) {
    if row_response.clicked() {
        if selection.contains(&row_index) {
            selection.remove(&row_index);
        } else {
            selection.insert(row_index);
        }
    }
}
