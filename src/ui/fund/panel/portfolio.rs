use crate::{message, ui::loading, util};
use chrono::{Datelike, Duration, NaiveDate};
use egui::{epaint::Hsva, Color32, ComboBox, Layout, Sense, TopBottomPanel, Ui};
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
    pub filter_date: String,
    pub filter_year: String,
    pub filter_month: String,
    pub tp_aplic_selected: std::collections::HashSet<usize>,

    pub start_date: String,
    pub pl: DataFrame,
    pub assets: DataFrame,
    pub top_assets: DataFrame,
    pub cnpj: String,
    pub sender: Option<UnboundedSender<message::Message>>,
    pub loading: bool,
}

impl Default for PortfolioUI {
    fn default() -> Self {
        let now = chrono::offset::Utc::now().date_naive();
        let now_str = format!("{}/{}", month_name(now.month() as i32), now.year());

        PortfolioUI {
            cnpj: String::from(""),
            sender: None,
            assets: DataFrame::empty(),
            pl: DataFrame::empty(),
            top_assets: DataFrame::empty(),
            filter_year: now.year().to_string(),
            filter_month: format!("{:02}", now.month()),
            tp_aplic_selected: Default::default(),
            start_date: "".to_string(),
            filter_date: now_str,
            loading: false,
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
                            ui.heading(egui::RichText::new("Composição da Carteira").size(16.0));
                        });
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.horizontal(|ui| {
                            self.create_date_combobox(ui);
                        });
                    });
                });
                ui.separator();

                if self.loading {
                    ui.vertical_centered(|ui| {
                        loading::show(ui);
                    });
                } else {
                    self.show_assets_panel(ui);
                }
            });
        });
    }

    fn generate_available_dates(&self, end_date: NaiveDate) -> Vec<String> {
        let mut dates = Vec::new();
        let mut current_date = chrono::Local::now().naive_local().date(); // data atual

        while current_date >= end_date {
            let month_name = match current_date.month() {
                1 => "Janeiro",
                2 => "Fevereiro",
                3 => "Março",
                4 => "Abril",
                5 => "Maio",
                6 => "Junho",
                7 => "Julho",
                8 => "Agosto",
                9 => "Setembro",
                10 => "Outubro",
                11 => "Novembro",
                12 => "Dezembro",
                _ => unreachable!(),
            };
            dates.push(format!("{}/{}", month_name, current_date.year()));

            // Subtrai um mês
            current_date = current_date - Duration::days(current_date.day() as i64);
        }

        dates
    }

    fn create_date_combobox(&mut self, ui: &mut egui::Ui) {
        //2022-09-21
        let end_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").unwrap();
        let available_dates = self.generate_available_dates(end_date);

        ComboBox::from_label("Selecione a data")
            .selected_text(self.filter_date.to_string())
            .show_ui(ui, |ui| {
                let mut last_year = "".to_string();
                for date in available_dates.clone() {
                    let v: Vec<&str> = date.split('/').collect();
                    let month = v[0].to_string();
                    let year = v[1].to_string();
                    if !last_year.is_empty() && last_year != year {
                        ui.separator();
                    }

                    if ui
                        .selectable_value(&mut self.filter_date, date.clone(), date.clone())
                        .clicked()
                    {
                        let m = format!("{:02}", month_name_to_i32(month.as_str()));
                        self.filter_year = year.to_string();
                        self.filter_month = m;
                        self.send_assets_message();
                    }

                    last_year = year;
                }
            });
    }

    pub fn send_assets_message(&mut self) {
        let _ = self.sender.clone().unwrap().send(message::Message::Assets(
            self.cnpj.to_string(),
            self.filter_year.clone(),
            self.filter_month.clone(),
        ));
        self.loading = true;
    }

    pub fn show_assets_panel(&mut self, ui: &mut Ui) {
        egui::SidePanel::left(ui.id().with("left_assets_panel"))
            .resizable(true)
            .default_width(400.0)
            .width_range(200.0..=450.0)
            .show_inside(ui, |ui| {
                ui.add_space(10.0);
                let nr_rows = self.top_assets.height();
                let cols: Vec<&str> = vec!["TP_APLIC", "VL_MERC_POS_FINAL", "VL_PORCENTAGEM_PL"];
                let colors = generate_colors(self.top_assets.height());

                ui.push_id("top_assets", |ui| {
                    TopBottomPanel::top(ui.id().with("bottom_pl_panel")).show_inside(
                        ui,
                        |ui: &mut Ui| {
                            ui.horizontal(|ui| {
                                ui.weak("Patrimonio Líquido");
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        self.pl
                                            .column("VL_PATRIM_LIQ")
                                            .ok()
                                            .and_then(|col| col.get(0).ok())
                                            .and_then(|val| val.get_str().map(|s| s.to_string()))
                                            .and_then(|value_str| value_str.parse::<f64>().ok())
                                            .and_then(|parsed_value| {
                                                util::to_real(parsed_value).ok()
                                            })
                                            .map(|v| ui.heading(v.format()))
                                            .unwrap_or_else(|| ui.label("-"));
                                    },
                                );
                            });
                            ui.add_space(5.0);
                        },
                    );
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        TableBuilder::new(ui)
                            .column(Column::auto().at_least(100.0).resizable(true).clip(true))
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
                                    row.set_selected(self.tp_aplic_selected.contains(&row_index));
                                    for col in &cols {
                                        row.col(|ui| {
                                            if let Ok(column) = self.top_assets.column(col) {
                                                if let Ok(value) = column.get(row_index) {
                                                    if col.contains("VL_PORCENTAGEM_PL") {
                                                        let a = value
                                                            .to_string()
                                                            .parse::<f64>()
                                                            .unwrap();
                                                        if a > 0.0 {
                                                            ui.colored_label(
                                                                Color32::DARK_GREEN,
                                                                format!("{}%", a),
                                                            );
                                                        } else {
                                                            ui.colored_label(
                                                                Color32::RED,
                                                                format!("{}%", a),
                                                            );
                                                        }
                                                    } else if col.contains("VL_MERC_POS_FINAL") {
                                                        let a = value
                                                            .to_string()
                                                            .parse::<f64>()
                                                            .unwrap();
                                                        let r = util::to_real(a).unwrap();
                                                        ui.label(r.format());
                                                    } else if let Some(value_str) = value.get_str()
                                                    {
                                                        circle(
                                                            value_str.to_string(),
                                                            colors[row_index],
                                                            ui,
                                                        );
                                                        ui.label(value_str);
                                                    }
                                                }
                                            }
                                        });
                                    }
                                    toggle_row_selection(
                                        &mut self.tp_aplic_selected,
                                        row_index,
                                        &row.response(),
                                    );
                                });
                            });
                    });

                    //});
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.push_id("filter_assets", |ui| {
                let mut filters = Vec::new();
                for r in self.tp_aplic_selected.iter() {
                    if let Ok(column) = self.top_assets.column("TP_APLIC") {
                        if let Ok(value) = column.get(*r) {
                            let v = value.get_str().unwrap();
                            filters.push(v.to_string())
                        }
                    }
                }

                let filters_series = Series::new("filters", filters);
                let lf = if filters_series.len() > 0 {
                    self.assets
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
                    self.assets.clone().lazy()
                };

                let filtered_df = lf.collect().unwrap();
                let nr_rows = filtered_df.height();
                let cols: Vec<&str> = vec![
                    "TP_APLIC",
                    "DETALHES",
                    "VL_MERC_POS_FINAL",
                    "VL_PORCENTAGEM_PL",
                ];
                ui.group(|ui| {
                    ui.set_min_height(ui.available_height());

                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        TableBuilder::new(ui)
                            .column(Column::auto().at_least(300.0).resizable(true).clip(true))
                            .column(Column::auto().at_least(400.0).resizable(true).clip(true))
                            .column(
                                Column::remainder()
                                    .at_least(200.0)
                                    .resizable(true)
                                    .clip(true),
                            )
                            .column(Column::remainder())
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .striped(true)
                            .resizable(false)
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.label("Aplicação");
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
                                    for (i, col_name) in cols.iter().enumerate() {
                                        row.col(|ui| {
                                            if i == 1 {
                                                let details_label = [
                                                    "CD_ATIVO",
                                                    "DS_ATIVO",
                                                    "NM_FUNDO_COTA",
                                                    "TP_TITPUB",
                                                    "CD_SELIC",
                                                    "TP_APLIC",
                                                ]
                                                .iter()
                                                .filter_map(|&col| {
                                                    get_value_from_column(
                                                        col,
                                                        &filtered_df,
                                                        row_index,
                                                    )
                                                })
                                                .next()
                                                .unwrap_or_else(|| "N/A".to_string());
                                                if ui.link(details_label.clone()).clicked() {
                                                    let row_values =
                                                        filtered_df.get_row(row_index).unwrap().0;
                                                    let row_df = DataFrame::new(
                                                        filtered_df
                                                            .get_column_names()
                                                            .iter()
                                                            .cloned()
                                                            .zip(row_values)
                                                            .map(|(name, value)| {
                                                                Series::new(name, vec![value])
                                                            })
                                                            .collect(),
                                                    )
                                                    .unwrap();
                                                    let _ = self.sender.clone().unwrap().send(
                                                        message::Message::ShowAssetDetail(
                                                            row_df.clone(),
                                                        ),
                                                    );
                                                }
                                            } else if let Ok(column) = filtered_df.column(col_name)
                                            {
                                                if let Ok(value) = column.get(row_index) {
                                                    if col_name.contains("VL_PORCENTAGEM_PL") {
                                                        let a = value
                                                            .to_string()
                                                            .parse::<f64>()
                                                            .unwrap();
                                                        if a > 0.0 {
                                                            ui.colored_label(
                                                                Color32::DARK_GREEN,
                                                                format!("{}%", a),
                                                            );
                                                        } else {
                                                            ui.colored_label(
                                                                Color32::RED,
                                                                format!("{}%", a),
                                                            );
                                                        }
                                                    } else if let Some(value_str) = value.get_str()
                                                    {
                                                        if col_name.contains("VL_MERC_POS_FINAL") {
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

fn get_value_from_column(
    column_name: &str,
    filtered_df: &DataFrame,
    row_index: usize,
) -> Option<String> {
    if let Ok(column) = filtered_df.column(column_name) {
        if let Ok(value) = column.get(row_index) {
            if let Some(value_str) = value.get_str() {
                return Some(value_str.to_string());
            }
        }
    }
    None
}

fn circle(_name: String, color: egui::Color32, ui: &mut Ui) {
    let r = 5.0;
    let size = egui::Vec2::splat(2.0 * r + 5.0);
    let (rect, _response) = ui.allocate_at_least(size, Sense::hover());
    ui.painter().circle_filled(rect.center(), r, color);
}

fn generate_colors(n: usize) -> Vec<egui::Color32> {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875

    (0..n)
        .map(|i| {
            let h = i as f32 * golden_ratio;
            egui::Color32::from(Hsva::new(h.fract(), 0.85, 0.5, 1.0))
        })
        .collect()
}

fn month_name_to_i32(month_name: &str) -> i32 {
    match month_name {
        "Janeiro" => 1,
        "Fevereiro" => 2,
        "Março" => 3,
        "Abril" => 4,
        "Maio" => 5,
        "Junho" => 6,
        "Julho" => 7,
        "Agosto" => 8,
        "Setembro" => 9,
        "Outubro" => 10,
        "Novembro" => 11,
        "Dezembro" => 12,
        _ => unreachable!(),
    }
}

fn month_name(month: i32) -> String {
    match month {
        1 => "Janeiro".to_string(),
        2 => "Fevereiro".to_string(),
        3 => "Março".to_string(),
        4 => "Abril".to_string(),
        5 => "Maio".to_string(),
        6 => "Junho".to_string(),
        7 => "Julho".to_string(),
        8 => "Agosto".to_string(),
        9 => "Setembro".to_string(),
        10 => "Outubro".to_string(),
        11 => "Novembro".to_string(),
        12 => "Dezembro".to_string(),
        _ => unreachable!(),
    }
}
