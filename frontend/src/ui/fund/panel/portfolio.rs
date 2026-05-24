use crate::{
    ui::design::{Components, Scale, Typography},
    ui::fund::modal::asset_detail::{AssetDetailModal, AssetModalContext},
    ui::loading,
    util,
};
use chrono::{Datelike, Duration, NaiveDate};
use egui::{epaint::Hsva, ComboBox, Layout, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use fundos_common::types::{PortfolioAsset, PortfolioPL};
use std::collections::HashSet;

#[derive(Clone)]
pub struct PortfolioUI {
    pub filter_date: String,
    pub filter_year: String,
    pub filter_month: String,
    pub tp_aplic_selected: HashSet<usize>,
    pub search_query: String,

    pub start_date: String,
    pub pl: Vec<PortfolioPL>,
    pub assets: Vec<PortfolioAsset>,
    pub top_assets: Vec<PortfolioAsset>,
    pub cnpj: String,
    pub loading: bool,
    pub loading_status: String,
    pub asset_modal: AssetDetailModal,
    pub show_insights: bool,
    /// Set when user selects a date. Parent reads and clears.
    pub requested_date: Option<(String, String)>,
}

impl Default for PortfolioUI {
    fn default() -> Self {
        let now = chrono::offset::Utc::now().date_naive();
        // Subtract ~90 days for CVM disclosure lag
        let target_date = now.checked_sub_signed(chrono::Duration::days(90)).unwrap_or(now);
        let target_month = target_date.month();
        let target_year = target_date.year();
        let now_str = format!("{}/{}", month_name(target_month as i32), target_year);

        PortfolioUI {
            cnpj: String::new(),
            assets: vec![],
            pl: vec![],
            top_assets: vec![],
            filter_year: target_year.to_string(),
            filter_month: format!("{:02}", target_month),
            tp_aplic_selected: Default::default(),
            start_date: String::new(),
            filter_date: now_str,
            loading_status: "Carregando composição de carteira...".to_string(),
            asset_modal: AssetDetailModal {
                context: AssetModalContext::FundPortfolio,
                ..Default::default()
            },
            show_insights: false,
            search_query: String::new(),
            requested_date: Some((target_year.to_string(), format!("{:02}", target_month))),
            loading: true,
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
                            let dark = ui.visuals().dark_mode;
                            ui.label(Typography::heading_3("Composição da Carteira", dark));
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
                        loading::show(ui, &self.loading_status);
                    });
                } else {
                    egui::Panel::top("insights_kpi_panel")
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                        .show_inside(ui, |ui| {
                            crate::ui::fund::panel::insights::show_kpis(
                                &self.assets, &self.pl, ui,
                            );
                        });

                    egui::Panel::right("insights_detailed_panel")
                        .resizable(true)
                        .min_size(320.0)
                        .default_size(Scale::MODAL_NARROW)
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 4)))
                        .show_inside(ui, |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                crate::ui::fund::panel::insights::show_detailed(
                                    &self.assets, &self.pl, ui,
                                );
                            });
                        });

                    ui.add_space(4.0);
                    self.show_assets_panel(ui);
                }

                self.asset_modal.show(ui);
            });
        });
    }

    fn generate_available_dates(&self, end_date: NaiveDate) -> Vec<String> {
        let mut dates = Vec::new();
        let mut current_date = chrono::Local::now().naive_local().date();

        while current_date >= end_date {
            let month_name = match current_date.month() {
                1 => "Janeiro", 2 => "Fevereiro", 3 => "Março",
                4 => "Abril", 5 => "Maio", 6 => "Junho",
                7 => "Julho", 8 => "Agosto", 9 => "Setembro",
                10 => "Outubro", 11 => "Novembro", 12 => "Dezembro",
                _ => unreachable!(),
            };
            dates.push(format!("{}/{}", month_name, current_date.year()));
            current_date = current_date - Duration::days(current_date.day() as i64);
        }
        dates
    }

    fn create_date_combobox(&mut self, ui: &mut egui::Ui) {
        let end_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2022, 9, 21).unwrap());
        let available_dates = self.generate_available_dates(end_date);

        ComboBox::from_label("Selecione a data")
            .selected_text(self.filter_date.clone())
            .show_ui(ui, |ui| {
                let mut last_year = String::new();
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
                        self.filter_month = m.clone();
                        self.requested_date = Some((year.clone(), m));
                        self.loading = true;
                    }
                    last_year = year;
                }
            });
    }

    pub fn show_assets_panel(&mut self, ui: &mut Ui) {
        if self.top_assets.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::WARNING.to_string())
                        .color(egui::Color32::from_rgb(250, 185, 80))
                        .size(Scale::ICON_JUMBO),
                );
                ui.add_space(15.0);
                let dark = ui.visuals().dark_mode;
                ui.label(Typography::heading_2("Nenhuma Carteira Encontrada", dark));
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        "Não há registros de composição de ativos para a data selecionada.",
                    )
                    .color(if ui.visuals().dark_mode {
                        egui::Color32::from_rgb(160, 175, 195)
                    } else {
                        egui::Color32::from_rgb(75, 85, 100)
                    })
                    .size(Scale::DEFAULT.button()),
                );
                ui.add_space(10.0);
                ui.weak("Certifique-se de que a CVM publicou os dados mensais para este período.");
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
                let nr_rows = self.top_assets.len();
                let colors = generate_colors(self.top_assets.len());

                ui.push_id("top_assets", |ui| {
                    let dark = ui.visuals().dark_mode;
                    ui.label(Typography::heading_3(
                        format!("{} Classes de Ativos", egui_phosphor::regular::CHART_PIE_SLICE),
                        dark,
                    ));
                    ui.separator();
                    ui.add_space(8.0);
                    Components::configure_table(TableBuilder::new(ui))
                        .id_salt("portfolio_assets_table")
                        .column(Column::initial(100.0).resizable(true).clip(true))
                        .column(Column::initial(100.0).clip(true))
                        .column(Column::remainder().at_least(120.0))
                        .resizable(false)
                        .sense(Sense::click())
                        .header(Scale::DEFAULT.table_header_height(), |mut header| {
                            header.col(|ui| {
                                ui.label(
                                    egui::RichText::new("Classe")
                                        .size(Scale::DEFAULT.label())
                                        .strong(),
                                );
                            });
                            header.col(|ui| {
                                ui.label(
                                    egui::RichText::new("Valor")
                                        .size(Scale::DEFAULT.label())
                                        .strong(),
                                );
                            });
                            header.col(|ui| {
                                ui.label(
                                    egui::RichText::new("% PL")
                                        .size(Scale::DEFAULT.label())
                                        .strong(),
                                );
                            });
                        })
                        .body(|body| {
                            body.rows(Scale::DEFAULT.table_row_height(), nr_rows, |mut row| {
                                let row_index = row.index();
                                row.set_selected(self.tp_aplic_selected.contains(&row_index));
                                let asset = &self.top_assets[row_index];

                                row.col(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("●")
                                                .color(colors[row_index])
                                                .size(Scale::DEFAULT.label()),
                                        );
                                        ui.label(
                                            egui::RichText::new(&asset.tipo_ativo)
                                                .size(Scale::DEFAULT.label()),
                                        );
                                    });
                                });
                                row.col(|ui| {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(util::to_real(
                                                    asset.valor_mercado,
                                                ))
                                                .size(Scale::DEFAULT.label()),
                                            );
                                        },
                                    );
                                });
                                row.col(|ui| {
                                    ui.centered_and_justified(|ui| {
                                        draw_custom_progress_bar(
                                            ui,
                                            asset.vl_porcentagem_pl,
                                        );
                                    });
                                });

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
                // Build filter set from selected rows
                let filters: HashSet<&str> = self
                    .tp_aplic_selected
                    .iter()
                    .filter_map(|&r| self.top_assets.get(r).map(|a| a.tipo_ativo.as_str()))
                    .collect();

                // Filter and sort assets
                let mut filtered: Vec<&PortfolioAsset> = if filters.is_empty() {
                    self.assets.iter().collect()
                } else {
                    self.assets
                        .iter()
                        .filter(|a| filters.contains(a.tipo_ativo.as_str()))
                        .collect()
                };
                filtered.sort_by(|a, b| {
                    b.vl_porcentagem_pl
                        .partial_cmp(&a.vl_porcentagem_pl)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                // Search filter
                let query = util::normalize_string(&self.search_query);
                let filtered: Vec<&&PortfolioAsset> = if !query.is_empty() {
                    filtered
                        .iter()
                        .filter(|a| {
                            let name = util::normalize_string(&a.nome_ativo);
                            let tipo = util::normalize_string(&a.tipo_ativo);
                            let code = util::normalize_string(&a.codigo_negociacao);
                            name.contains(&query)
                                || tipo.contains(&query)
                                || code.contains(&query)
                        })
                        .collect()
                } else {
                    filtered.iter().collect()
                };

                let nr_rows = filtered.len();
                {
                    let dark = ui.visuals().dark_mode;
                    ui.label(Typography::heading_3(
                        format!("{} Detalhamento da Carteira", egui_phosphor::regular::LIST_DASHES),
                        dark,
                    ));
                    ui.separator();
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                egui_phosphor::regular::MAGNIFYING_GLASS.to_string(),
                            )
                            .size(Scale::ICON_SMALL),
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
                            Components::configure_table(TableBuilder::new(ui))
                                .auto_shrink([false, false])
                                .column(
                                    Column::initial(150.0).at_least(100.0).resizable(true).clip(true),
                                )
                                .column(Column::remainder().at_least(100.0).clip(true))
                                .column(
                                    Column::initial(150.0).at_least(150.0).resizable(true).clip(true),
                                )
                                .column(
                                    Column::initial(140.0).at_least(140.0).resizable(false).clip(true),
                                )
                                .header(Scale::DEFAULT.table_header_height(), |mut header| {
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
                                    body.rows(
                                        Scale::DEFAULT.table_row_height(),
                                        nr_rows,
                                        |mut row| {
                                            let asset = filtered[row.index()];

                                            row.col(|ui| {
                                                ui.label(&asset.tipo_ativo);
                                            });
                                            row.col(|ui| {
                                                if asset.codigo_negociacao.is_empty() && asset.nome_ativo.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new(format!("{} Ativo Confidencial", egui_phosphor::regular::LOCK_KEY))
                                                            .color(ui.visuals().weak_text_color())
                                                            .italics()
                                                    ).on_hover_text("A composição deste ativo está sob sigilo temporário da CVM.");
                                                } else {
                                                    let details = if !asset.codigo_negociacao.is_empty()
                                                        && asset.nome_ativo != asset.codigo_negociacao
                                                    {
                                                        format!(
                                                            "{} - {}",
                                                            asset.codigo_negociacao, asset.nome_ativo
                                                        )
                                                    } else if !asset.codigo_negociacao.is_empty() {
                                                        asset.codigo_negociacao.clone()
                                                    } else {
                                                        asset.nome_ativo.clone()
                                                    };
                                                    if ui.link(&details).clicked() {
                                                        self.asset_modal.title =
                                                            format!("Detalhes - {}", asset.nome_ativo);
                                                        self.asset_modal.codigo =
                                                            asset.codigo_isin.clone();
                                                        self.asset_modal.nome =
                                                            asset.nome_ativo.clone();
                                                        self.asset_modal.tp_aplic =
                                                            asset.tipo_ativo.clone();
                                                        self.asset_modal.cnpj = self.cnpj.clone();
                                                        self.asset_modal.vl_mercado =
                                                            asset.valor_mercado;
                                                        self.asset_modal.pct_pl =
                                                            asset.vl_porcentagem_pl;
                                                        self.asset_modal.open_with_auto_fetch();
                                                    }
                                                }
                                            });
                                            row.col(|ui| {
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(util::to_real(
                                                            asset.valor_mercado,
                                                        ));
                                                    },
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.centered_and_justified(|ui| {
                                                    draw_custom_progress_bar(
                                                        ui,
                                                        asset.vl_porcentagem_pl,
                                                    );
                                                });
                                            });
                                        },
                                    );
                                });
                        });
                }
            });
        });
    }
}

fn toggle_row_selection(selection: &mut HashSet<usize>, row_index: usize, row_response: &egui::Response) {
    if row_response.clicked() {
        if selection.contains(&row_index) {
            selection.remove(&row_index);
        } else {
            selection.insert(row_index);
        }
    }
}

fn generate_colors(n: usize) -> Vec<egui::Color32> {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
    (0..n)
        .map(|i| {
            let h = i as f32 * golden_ratio;
            egui::Color32::from(Hsva::new(h.fract(), 0.85, 0.5, 1.0))
        })
        .collect()
}

fn month_name_to_i32(month_name: &str) -> i32 {
    match month_name {
        "Janeiro" => 1, "Fevereiro" => 2, "Março" => 3,
        "Abril" => 4, "Maio" => 5, "Junho" => 6,
        "Julho" => 7, "Agosto" => 8, "Setembro" => 9,
        "Outubro" => 10, "Novembro" => 11, "Dezembro" => 12,
        _ => unreachable!(),
    }
}

fn month_name(month: i32) -> String {
    match month {
        1 => "Janeiro", 2 => "Fevereiro", 3 => "Março",
        4 => "Abril", 5 => "Maio", 6 => "Junho",
        7 => "Julho", 8 => "Agosto", 9 => "Setembro",
        10 => "Outubro", 11 => "Novembro", 12 => "Dezembro",
        _ => unreachable!(),
    }
    .to_string()
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
