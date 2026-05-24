use crate::ui::{design::Scale, loading};
use crate::util;
use egui::{Align2, Vec2};
use egui_extras::{Column, TableBuilder};
use fundos_common::types::{FundSearchResponse, FundSummary};
use std::sync::{Arc, Mutex};

pub struct Search {
    pub open_window: bool,
    pub query: String,
    pub class_filter: Option<String>,
    pub results: Vec<FundSummary>,
    pub loading: bool,
    pub loading_status: String,
    pending_result: Option<Arc<Mutex<Option<FundSearchResponse>>>>,
}

impl Search {
    pub fn new(open_window: bool) -> Self {
        Search {
            open_window,
            query: String::new(),
            class_filter: None,
            results: vec![],
            loading: false,
            loading_status: "Buscando fundos...".to_string(),
            pending_result: None,
        }
    }

    pub fn open(&mut self, value: bool, ctx: &egui::Context) {
        if value && !self.open_window {
            self.do_search(ctx);
        }
        self.open_window = value;
    }

    pub fn set_loading(&mut self, value: bool) {
        self.loading = value;
        if value {
            self.loading_status = "Carregando lista de fundos...".to_string();
        }
    }

    fn do_search(&mut self, ctx: &egui::Context) {
        let query_opt = if self.query.trim().is_empty() { None } else { Some(self.query.clone()) };
        self.loading = true;
        let query = self.query.clone();
        let class = self.class_filter.clone();
        let ctx_c = ctx.clone();

        let result: Arc<Mutex<Option<FundSearchResponse>>> = Arc::new(Mutex::new(None));
        let r = result.clone();
        self.pending_result = Some(result);

        util::spawn_future(async move {
            match crate::api_client::search_funds(query_opt.as_deref(), class.as_deref()).await {
                Ok(resp) => {
                    log::info!("Search returned {} funds", resp.total);
                    *r.lock().unwrap() = Some(resp);
                    ctx_c.request_repaint();
                }
                Err(e) => log::error!("Search error: {}", e),
            }
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut clicked_cnpj = None;
        // Poll pending async result
        if let Some(pending) = self.pending_result.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(resp) = data {
                self.results = resp.funds;
                self.loading = false;
            } else {
                self.pending_result = Some(pending);
            }
        }

        let screen_width = ui.ctx().content_rect().width();
        let default_modal_width = 850.0f32.min(screen_width * 0.95);

        let mut open = self.open_window;
        let is_large = screen_width > 950.0;
        let window_id = if is_large {
            "Fundos_large"
        } else {
            "Fundos_small"
        };

        egui::Window::new("Fundos")
            .id(egui::Id::new(window_id))
            .resizable(true)
            .collapsible(false)
            .min_width(450.0f32.min(screen_width * 0.95))
            .default_width(default_modal_width)
            .max_width(screen_width * 0.98)
            .max_height(Scale::MODAL_WIDE)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, -60.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let search_bar = egui::TextEdit::singleline(&mut self.query)
                    .font(egui::TextStyle::Body)
                    .hint_text("🔍 Busque pelo nome ou cnpj do fundo..")
                    .desired_width(ui.available_width())
                    .margin(egui::vec2(15.0, 10.0));

                let search_response: egui::Response = ui.add(search_bar);
                if search_response.changed() {
                    let ctx_c = ui.ctx().clone();
                    self.do_search(&ctx_c);
                }

                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    self.class_filter_button(ui, None, "Todos");
                    self.class_filter_button(ui, Some("Ações"), "Ações");
                    self.class_filter_button(ui, Some("Renda Fixa"), "Renda Fixa");
                    self.class_filter_button(ui, Some("Cambial"), "Cambial");
                    self.class_filter_button(ui, Some("MultiMercado"), "MultiMercado");
                });
                ui.add_space(5.0);

                if self.loading {
                    ui.vertical_centered(|ui| {
                        loading::show(ui, &self.loading_status);
                    });
                } else {
                    let nr_rows = self.results.len();
                    TableBuilder::new(ui)
                        .id_salt("search_funds_table")
                        .column(Column::auto().at_least(110.0).resizable(false).clip(true))
                        .column(Column::remainder().clip(true))
                        .column(Column::auto().at_least(110.0).resizable(false).clip(true))
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .resizable(false)
                        .header(Scale::DEFAULT.table_header_height(), |mut header| {
                            header.col(|ui| {
                                ui.label("CNPJ");
                            });
                            header.col(|ui| {
                                ui.label("Nome do Fundo");
                            });
                            header.col(|ui| {
                                ui.label("Classe");
                            });
                        })
                        .body(|body| {
                            body.rows(Scale::DEFAULT.table_row_height(), nr_rows, |mut row| {
                                let fund = &self.results[row.index()];

                                row.col(|ui| {
                                    ui.push_id((&fund.cnpj, "link_cnpj"), |ui| {
                                        let resp = ui
                                            .link(&fund.cnpj)
                                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        if resp.clicked() {
                                            clicked_cnpj = Some(fund.cnpj.clone());
                                        }
                                    });
                                });

                                row.col(|ui| {
                                    ui.push_id((&fund.cnpj, "nome_cnpj"), |ui| {
                                        ui.add(egui::Label::new(&fund.name).truncate());
                                    });
                                });

                                row.col(|ui| {
                                    ui.push_id((&fund.cnpj, "badge_cnpj"), |ui| {
                                        draw_class_badge(ui, &fund.class);
                                    });
                                });
                            });
                        });
                }

                egui::Panel::bottom("top_bottom_window").show_inside(ui, |ui| {
                    ui.add_space(5.0);
                });
            });

        if !open {
            self.query.clear();
            self.class_filter = None;
        }

        self.open_window = open;
        clicked_cnpj
    }

    fn class_filter_button(&mut self, ui: &mut egui::Ui, class_str: Option<&str>, label: &str) {
        let current = self.class_filter.clone();
        let mut selected = current.as_deref() == class_str;
        let resp = ui
            .selectable_value(&mut selected, true, label)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            self.class_filter = class_str.map(String::from);
            if class_str.is_some() || current.is_some() {
                let ctx_c = ui.ctx().clone();
                self.do_search(&ctx_c);
            }
        }
    }
}

fn draw_class_badge(ui: &mut egui::Ui, class_str: &str) {
    let (bg_color, text_color) = match class_str.to_lowercase().as_str() {
        s if s.contains("renda fixa")
            || s.contains("curto prazo")
            || s.contains("referenciado")
            || s.contains("dívida externa") =>
        {
            (
                egui::Color32::from_rgb(232, 240, 254),
                egui::Color32::from_rgb(26, 115, 232),
            )
        }
        s if s.contains("ações") || s.contains("fmp-fgts") || s.contains("acao") => (
            egui::Color32::from_rgb(230, 244, 234),
            egui::Color32::from_rgb(19, 115, 51),
        ),
        s if s.contains("multimercado") || s.contains("fip") => (
            egui::Color32::from_rgb(254, 247, 224),
            egui::Color32::from_rgb(176, 96, 0),
        ),
        s if s.contains("cambial") => (
            egui::Color32::from_rgb(243, 229, 245),
            egui::Color32::from_rgb(106, 27, 154),
        ),
        _ => (
            egui::Color32::from_rgb(241, 243, 244),
            egui::Color32::from_rgb(95, 99, 104),
        ),
    };

    let label_text = if class_str.len() > 15 {
        format!("{}...", &class_str[..12])
    } else {
        class_str.to_string()
    };

    ui.push_id(class_str, |ui| {
        ui.label(
            egui::RichText::new(format!(" {} ", label_text))
                .color(text_color)
                .size(Scale::DEFAULT.badge())
                .strong()
                .background_color(bg_color),
        );
    });
}
