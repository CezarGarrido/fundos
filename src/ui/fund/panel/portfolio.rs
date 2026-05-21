use crate::{
    message,
    ui::fund::modal::asset_detail::{AssetDetailModal, AssetModalContext},
    ui::loading,
    util,
};
use chrono::{Datelike, Duration, NaiveDate};
use egui::{epaint::Hsva, ComboBox, Layout, Sense, Ui};
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
    pub search_query: String,

    pub start_date: String,
    pub pl: DataFrame,
    pub assets: DataFrame,
    pub top_assets: DataFrame,
    pub cnpj: String,
    pub sender: Option<UnboundedSender<message::Message>>,
    pub loading: bool,
    pub asset_modal: AssetDetailModal,
    #[allow(dead_code)]
    pub show_insights: bool,
    pub fund_history: Option<DataFrame>,
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
            asset_modal: AssetDetailModal {
                context: AssetModalContext::FundPortfolio,
                ..Default::default()
            },
            show_insights: false,
            search_query: String::new(),
            fund_history: None,
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
                    egui::Panel::top("insights_kpi_panel")
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                        .show_inside(ui, |ui| {
                            crate::ui::fund::panel::insights::show_kpis(
                                self.assets.clone(),
                                self.pl.clone(),
                                ui,
                            );
                        });

                    egui::Panel::right("insights_detailed_panel")
                        .resizable(true)
                        .min_size(320.0)
                        .default_size(380.0)
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 4)))
                        .show_inside(ui, |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                crate::ui::fund::panel::insights::show_detailed(
                                    self.assets.clone(),
                                    self.pl.clone(),
                                    self.fund_history.clone(),
                                    ui,
                                );
                            });
                        });

                    ui.add_space(4.0);
                    self.show_assets_panel(ui);
                }

                // Asset detail modal
                self.asset_modal.show(ui);
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
        let end_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2022, 9, 21).unwrap());
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
        let _card_bg = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(30, 35, 45)
        } else {
            egui::Color32::from_rgb(245, 247, 250)
        };

        let heading_color = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(220, 230, 245)
        } else {
            egui::Color32::from_rgb(30, 40, 60)
        };

        let _progress_text_color = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(250, 250, 250)
        } else {
            egui::Color32::from_rgb(40, 50, 60)
        };

        if self.top_assets.height() == 0 {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::WARNING.to_string())
                        .color(egui::Color32::from_rgb(250, 185, 80))
                        .size(48.0),
                );
                ui.add_space(15.0);
                let desc_color = if ui.visuals().dark_mode {
                    egui::Color32::from_rgb(160, 175, 195)
                } else {
                    egui::Color32::from_rgb(75, 85, 100)
                };

                ui.heading(
                    egui::RichText::new("Nenhuma Carteira Encontrada")
                        .color(heading_color)
                        .size(18.0)
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Não há registros de composição de ativos para a data selecionada.")
                        .color(desc_color)
                        .size(13.0)
                );
                ui.add_space(10.0);
                ui.weak("Certifique-se de que a CVM publicou os dados mensais para este período ou verifique sua conexão.");
                ui.add_space(40.0);
            });
            return;
        }

        egui::Panel::left(ui.id().with("left_assets_panel"))
            .resizable(true)
            .default_size(280.0)
            .size_range(180.0..=400.0)
            .show_inside(ui, |ui| {
                ui.add_space(10.0);
                let nr_rows = self.top_assets.height();
                let cols: Vec<&str> = vec!["TP_APLIC", "VL_MERC_POS_FINAL", "VL_PORCENTAGEM_PL"];
                let colors = generate_colors(self.top_assets.height());

                ui.push_id("top_assets", |ui| {
                    ui.heading(
                        egui::RichText::new(format!(
                            "{} Classes de Ativos",
                            egui_phosphor::regular::CHART_PIE_SLICE
                        ))
                        .size(14.0)
                        .strong()
                        .color(heading_color),
                    );
                    ui.separator();
                    ui.add_space(8.0);
                    TableBuilder::new(ui)
                        .id_salt("portfolio_assets_table")
                        .column(Column::initial(100.0).resizable(true).clip(true))
                        .column(Column::initial(100.0).clip(true))
                        .column(Column::remainder().at_least(120.0))
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .resizable(false)
                        .sense(Sense::click())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.label(egui::RichText::new("Classe").size(11.0).strong());
                            });
                            header.col(|ui| {
                                ui.label(egui::RichText::new("Valor").size(11.0).strong());
                            });
                            header.col(|ui| {
                                ui.label(egui::RichText::new("% PL").size(11.0).strong());
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
                                                        .try_extract::<f64>()
                                                        .unwrap_or_else(|_| {
                                                            value
                                                                .to_string()
                                                                .parse::<f64>()
                                                                .unwrap_or(0.0)
                                                        });

                                                    ui.centered_and_justified(|ui| {
                                                        draw_custom_progress_bar(ui, a);
                                                    });
                                                } else if col.contains("VL_MERC_POS_FINAL") {
                                                    let a = value
                                                        .try_extract::<f64>()
                                                        .unwrap_or_else(|_| {
                                                            value
                                                                .to_string()
                                                                .parse::<f64>()
                                                                .unwrap_or(0.0)
                                                        });
                                                    let r = util::to_real(a).unwrap();
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.label(
                                                                egui::RichText::new(r.format())
                                                                    .size(11.0),
                                                            );
                                                        },
                                                    );
                                                } else if let Some(value_str) = value.get_str() {
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            egui::RichText::new("●")
                                                                .color(colors[row_index])
                                                                .size(11.0),
                                                        );
                                                        ui.label(
                                                            egui::RichText::new(value_str)
                                                                .size(11.0),
                                                        );
                                                    });
                                                }
                                            }
                                        }
                                    });
                                }
                                if row.response().hovered() {
                                    row.response()
                                        .ctx
                                        .set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                toggle_row_selection(
                                    &mut self.tp_aplic_selected,
                                    row_index,
                                    &row.response(),
                                );
                            });
                        });
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

                let mut filtered_df = lf.collect().unwrap();

                let query = crate::util::normalize_string(&self.search_query);
                if !query.is_empty() {
                    let mut mask_vec = Vec::with_capacity(filtered_df.height());
                    for i in 0..filtered_df.height() {
                        let a = crate::util::normalize_string(
                            &get_value_from_column("DS_ATIVO", &filtered_df, i).unwrap_or_default(),
                        );
                        let b = crate::util::normalize_string(
                            &get_value_from_column("NM_FUNDO_COTA", &filtered_df, i)
                                .unwrap_or_default(),
                        );
                        let c = crate::util::normalize_string(
                            &get_value_from_column("TP_APLIC", &filtered_df, i).unwrap_or_default(),
                        );
                        mask_vec
                            .push(a.contains(&query) || b.contains(&query) || c.contains(&query));
                    }
                    if let Ok(mask_series) = Series::new("mask", mask_vec).bool() {
                        if let Ok(f_df) = filtered_df.filter(mask_series) {
                            filtered_df = f_df;
                        }
                    }
                }

                let nr_rows = filtered_df.height();
                let cols: Vec<&str> = vec![
                    "TP_APLIC",
                    "DETALHES",
                    "VL_MERC_POS_FINAL",
                    "VL_PORCENTAGEM_PL",
                ];
                {
                    ui.heading(
                        egui::RichText::new(format!(
                            "{} Detalhamento da Carteira",
                            egui_phosphor::regular::LIST_DASHES
                        ))
                        .size(14.0)
                        .strong()
                        .color(heading_color),
                    );
                    ui.separator();
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                egui_phosphor::regular::MAGNIFYING_GLASS.to_string(),
                            )
                            .size(14.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Pesquisar ativo, fundo ou aplicação..."),
                        );
                    });
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical()
                        .id_salt("detail_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            TableBuilder::new(ui)
                                .auto_shrink([false, false])
                                .column(
                                    Column::initial(150.0)
                                        .at_least(100.0)
                                        .resizable(true)
                                        .clip(true),
                                )
                                .column(Column::remainder().at_least(100.0).clip(true))
                                .column(
                                    Column::initial(150.0)
                                        .at_least(150.0)
                                        .resizable(true)
                                        .clip(true),
                                )
                                .column(
                                    Column::initial(140.0)
                                        .at_least(140.0)
                                        .resizable(false)
                                        .clip(true),
                                )
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .striped(true)
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.label("Aplicação");
                                    });
                                    header.col(|ui| {
                                        ui.label("Detalhes");
                                    });
                                    header.col(|ui| {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label("Valor");
                                            },
                                        );
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
                                                    let cd_ativo = get_value_from_column(
                                                        "CD_ATIVO",
                                                        &filtered_df,
                                                        row_index,
                                                    )
                                                    .unwrap_or_default();
                                                    let isin = get_value_from_column(
                                                        "CD_ISIN",
                                                        &filtered_df,
                                                        row_index,
                                                    )
                                                    .unwrap_or_default();
                                                    let sigla = if !cd_ativo.is_empty() {
                                                        cd_ativo
                                                    } else {
                                                        isin
                                                    };

                                                    let name_part = [
                                                        "DS_ATIVO",
                                                        "NM_FUNDO_COTA",
                                                        "TP_TITPUB",
                                                        "TP_APLIC",
                                                    ]
                                                    .iter()
                                                    .filter_map(|&col| {
                                                        let v = get_value_from_column(
                                                            col,
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        if !v.is_empty() {
                                                            Some(v)
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                    .next()
                                                    .unwrap_or_else(|| "N/A".to_string());

                                                    let details_label = if !sigla.is_empty()
                                                        && name_part != sigla
                                                    {
                                                        format!("{} - {}", sigla, name_part)
                                                    } else if !sigla.is_empty() {
                                                        sigla
                                                    } else {
                                                        name_part
                                                    };
                                                    if ui.link(details_label.clone()).clicked() {
                                                        // Populate AssetDetailModal from the clicked row
                                                        let aplic = get_value_from_column(
                                                            "TP_APLIC",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        let tp_ativo = get_value_from_column(
                                                            "TP_ATIVO",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        let cd_ativo = get_value_from_column(
                                                            "CD_ATIVO",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        let isin = get_value_from_column(
                                                            "CD_ISIN",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        let ds_ativo = get_value_from_column(
                                                            "DS_ATIVO",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        let nm_fundo = get_value_from_column(
                                                            "NM_FUNDO_COTA",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();
                                                        let titpub = get_value_from_column(
                                                            "TP_TITPUB",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .unwrap_or_default();

                                                        let codigo = if !cd_ativo.is_empty() {
                                                            cd_ativo
                                                        } else if !isin.is_empty() {
                                                            isin
                                                        } else {
                                                            String::new()
                                                        };
                                                        let nome = if !titpub.is_empty() {
                                                            titpub
                                                        } else if !ds_ativo.is_empty() {
                                                            ds_ativo
                                                        } else {
                                                            aplic.clone()
                                                        };
                                                        let vl_merc = get_value_from_column(
                                                            "VL_MERC_POS_FINAL",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .and_then(|v| v.parse::<f64>().ok())
                                                        .unwrap_or(0.0);
                                                        let vl_aquis = get_value_from_column(
                                                            "VL_AQUIS_NEGOC",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .and_then(|v| v.parse::<f64>().ok())
                                                        .unwrap_or(0.0);
                                                        let pct = get_value_from_column(
                                                            "VL_PORCENTAGEM_PL",
                                                            &filtered_df,
                                                            row_index,
                                                        )
                                                        .and_then(|v| v.parse::<f64>().ok())
                                                        .unwrap_or(0.0);

                                                        self.asset_modal.title =
                                                            format!("Detalhes - {}", nome);
                                                        self.asset_modal.codigo = codigo;
                                                        self.asset_modal.nome = nome;
                                                        self.asset_modal.tp_ativo = tp_ativo;
                                                        self.asset_modal.tp_aplic = aplic;
                                                        self.asset_modal.dt_venc = String::new();
                                                        self.asset_modal.nm_fundo = nm_fundo;
                                                        self.asset_modal.cnpj = self.cnpj.clone();
                                                        self.asset_modal.sender =
                                                            Some(self.sender.clone().unwrap());
                                                        self.asset_modal.vl_mercado = vl_merc;
                                                        self.asset_modal.vl_aquisicao = vl_aquis;
                                                        self.asset_modal.pct_pl = pct;
                                                        self.asset_modal.n_fundos = 0;
                                                        self.asset_modal.vl_comprado = vl_aquis;
                                                        self.asset_modal.n_compradores = 0;
                                                        self.asset_modal.n_vendedores = 0;
                                                        self.asset_modal.pl_medio = 0.0;
                                                        self.asset_modal.open_with_auto_fetch();
                                                    }
                                                } else if let Ok(column) =
                                                    filtered_df.column(col_name)
                                                {
                                                    if let Ok(value) = column.get(row_index) {
                                                        if col_name.contains("VL_PORCENTAGEM_PL") {
                                                            let a = value
                                                                .try_extract::<f64>()
                                                                .unwrap_or_else(|_| {
                                                                    value
                                                                        .to_string()
                                                                        .parse::<f64>()
                                                                        .unwrap_or(0.0)
                                                                });

                                                            ui.centered_and_justified(|ui| {
                                                                draw_custom_progress_bar(ui, a);
                                                            });
                                                        } else if let Some(value_str) =
                                                            value.get_str()
                                                        {
                                                            if col_name
                                                                .contains("VL_MERC_POS_FINAL")
                                                            {
                                                                let a = value_str
                                                                    .to_string()
                                                                    .parse::<f64>()
                                                                    .unwrap();
                                                                let r = util::to_real(a).unwrap();
                                                                ui.with_layout(
                                                                    egui::Layout::right_to_left(
                                                                        egui::Align::Center,
                                                                    ),
                                                                    |ui| {
                                                                        ui.label(r.format());
                                                                    },
                                                                );
                                                            } else {
                                                                ui.label(value_str);
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    }); // fechar body.rows
                                }); // fechar .body
                        }); // fechar ScrollArea
                } // fechar bloco
            }); // fechar push_id
        }); // fechar CentralPanel
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

fn draw_custom_progress_bar(ui: &mut egui::Ui, percentage: f64) {
    let desired_width = ui.available_width().max(60.0);
    let height = 18.0;
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(desired_width, height), egui::Sense::hover());

    let track_color = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(45, 50, 60)
    } else {
        egui::Color32::from_rgb(245, 247, 250)
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(9), track_color);

    let progress = (percentage / 100.0).clamp(0.0, 1.0) as f32;
    let text = format!("{:.2}%", percentage);
    let text_color = egui::Color32::WHITE;

    let galley = ui
        .painter()
        .layout_no_wrap(text, egui::FontId::proportional(11.0), text_color);
    let text_width = galley.rect.width();

    let mut fill_width = rect.width() * progress;
    let min_fill_width = text_width + 12.0;
    if fill_width < min_fill_width {
        fill_width = min_fill_width;
    }
    if fill_width > rect.width() {
        fill_width = rect.width();
    }

    let fill_color = if percentage < 0.0 {
        egui::Color32::from_rgb(200, 80, 80)
    } else {
        egui::Color32::from_rgb(60, 160, 100)
    };
    let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, height));
    ui.painter()
        .rect_filled(fill_rect, egui::CornerRadius::same(9), fill_color);

    let text_pos = egui::pos2(
        fill_rect.left() + 6.0,
        fill_rect.center().y - galley.rect.height() / 2.0,
    );
    ui.painter().galley(text_pos, galley, text_color);
}
