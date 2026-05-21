use crate::{message::Message, provider::cvm::fund::Class, ui::loading};
use egui::{Align2, Vec2};
use egui_extras::{Column, TableBuilder};
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

pub struct Search {
    sender: UnboundedSender<Message>,
    pub open_window: bool,
    pub query: String,
    pub class: Option<Class>,
    pub result: DataFrame,
    pub loading: bool,
}

enum Msg {
    Search,
    SelectClass(Option<Class>),
}

impl Search {
    pub fn new(open_window: bool, sender: UnboundedSender<Message>) -> Self {
        Search {
            sender,
            open_window,
            query: "".to_string(),
            class: None,
            result: DataFrame::empty(),
            loading: false,
        }
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Search => {
                // Não definimos loading como true para buscas em tempo real para evitar flickering!
                self.search_send();
            }
            Msg::SelectClass(selected_class) => {
                self.class = selected_class;
                self.search_send();
                self.set_loading(true);
            }
        }
    }

    pub fn open(&mut self, value: bool) {
        self.open_window = value;
    }

    pub fn set_result(&mut self, value: DataFrame) {
        self.result = value;
    }

    pub fn set_loading(&mut self, value: bool) {
        self.loading = value;
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let screen_width = ui.ctx().screen_rect().width();
        let default_modal_width = 850.0f32.min(screen_width * 0.95);

        let mut open = self.open_window;
        let is_large = screen_width > 950.0;
        let window_id = if is_large { "Fundos_large" } else { "Fundos_small" };

        egui::Window::new("Fundos")
            .id(egui::Id::new(window_id))
            .resizable(true)
            .collapsible(false)
            .min_width(450.0f32.min(screen_width * 0.95))
            .default_width(default_modal_width)
            .max_width(screen_width * 0.98)
            .max_height(600.0)
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
                    self.update(Msg::Search);
                }

                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    self.handle_selectable_value(ui, None, "Todos");
                    self.handle_selectable_value(ui, Some(Class::Acoes), "Ações");
                    self.handle_selectable_value(ui, Some(Class::RendaFixa), "Renda Fixa");
                    self.handle_selectable_value(ui, Some(Class::Cambial), "Cambial");
                    self.handle_selectable_value(ui, Some(Class::MultiMarket), "MultiMercado");
                });
                ui.add_space(5.0);
                let nr_rows = self.result.height();

                if self.loading {
                    ui.vertical_centered(|ui| {
                        loading::show(ui);
                    });
                } else {
                    TableBuilder::new(ui)
                        .id_salt("search_funds_table")
                        .column(Column::auto().at_least(110.0).resizable(false).clip(true))
                        .column(Column::remainder().clip(true))
                        .column(Column::auto().at_least(110.0).resizable(false).clip(true))
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .resizable(false)
                        .header(20.0, |mut header| {
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
                            body.rows(22.0, nr_rows, |mut row| {
                                let row_index = row.index();
                                
                                let mut cnpj_str = String::new();
                                if let Ok(column) = self.result.column("CNPJ_FUNDO") {
                                    if let Ok(value) = column.get(row_index) {
                                        if let Some(s) = value.get_str() {
                                            cnpj_str = s.to_string();
                                        }
                                    }
                                }

                                // Coluna 1: CNPJ Link
                                row.col(|ui| {
                                    if !cnpj_str.is_empty() {
                                        ui.push_id((&cnpj_str, "link_cnpj"), |ui| {
                                            let resp = ui.link(&cnpj_str).on_hover_cursor(egui::CursorIcon::PointingHand);
                                            if resp.clicked() {
                                                let _ = self.sender.send(Message::NewTab(cnpj_str.clone()));
                                            }
                                        });
                                    } else {
                                        ui.weak("-");
                                    }
                                });

                                // Coluna 2: Nome do Fundo
                                row.col(|ui| {
                                    if !cnpj_str.is_empty() {
                                        ui.push_id((&cnpj_str, "nome_cnpj"), |ui| {
                                            if let Ok(column) = self.result.column("DENOM_SOCIAL") {
                                                if let Ok(value) = column.get(row_index) {
                                                    if let Some(value_str) = value.get_str() {
                                                        ui.add(egui::Label::new(value_str).truncate());
                                                    }
                                                }
                                            }
                                        });
                                    }
                                });

                                // Coluna 3: Badge da Classe
                                row.col(|ui| {
                                    if !cnpj_str.is_empty() {
                                        ui.push_id((&cnpj_str, "badge_cnpj"), |ui| {
                                            if let Ok(column) = self.result.column("CLASSE") {
                                                if let Ok(value) = column.get(row_index) {
                                                    if let Some(value_str) = value.get_str() {
                                                        draw_class_badge(ui, value_str);
                                                    } else {
                                                        ui.weak("-");
                                                    }
                                                } else {
                                                    ui.weak("-");
                                                }
                                            } else {
                                                ui.weak("-");
                                            }
                                        });
                                    } else {
                                        ui.weak("-");
                                    }
                                });
                            });
                        });
                }

                egui::Panel::bottom("top_bottom_window").show_inside(ui, |ui| {
                    ui.add_space(5.0);
                });
            });

        if !open {
            self.query = "".to_string();
            self.class = None;
        }

        self.open_window = open;
    }
    fn handle_selectable_value(&mut self, ui: &mut egui::Ui, class: Option<Class>, label: &str) {
        let resp = ui
            .selectable_value(&mut self.class, class.clone(), label)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            self.update(Msg::SelectClass(class));
        }
    }

    fn search_send(&self) {
        let sender = self.sender.clone();
        let text = self.query.to_string();
        let class = self.class.clone();
        tokio::spawn(async move {
            let _ = sender.send(Message::SearchFunds(text, class));
        });
    }
}

fn draw_class_badge(ui: &mut egui::Ui, class_str: &str) {
    let (bg_color, text_color) = match class_str.to_lowercase().as_str() {
        // Renda Fixa
        s if s.contains("renda fixa") || s.contains("curto prazo") || s.contains("referenciado") || s.contains("dívida externa") => {
            (egui::Color32::from_rgb(232, 240, 254), egui::Color32::from_rgb(26, 115, 232))
        }
        // Ações
        s if s.contains("ações") || s.contains("fmp-fgts") || s.contains("acao") => {
            (egui::Color32::from_rgb(230, 244, 234), egui::Color32::from_rgb(19, 115, 51))
        }
        // Multimercado
        s if s.contains("multimercado") || s.contains("fip") => {
            (egui::Color32::from_rgb(254, 247, 224), egui::Color32::from_rgb(176, 96, 0))
        }
        // Cambial
        s if s.contains("cambial") => {
            (egui::Color32::from_rgb(243, 229, 245), egui::Color32::from_rgb(106, 27, 154))
        }
        // Outros
        _ => (egui::Color32::from_rgb(241, 243, 244), egui::Color32::from_rgb(95, 99, 104)),
    };

    let label_text = match class_str {
        s if s.len() > 15 => format!("{}...", &s[..12]),
        s => s.to_string(),
    };

    ui.push_id(class_str, |ui| {
        ui.label(
            egui::RichText::new(format!(" {} ", label_text))
                .color(text_color)
                .size(10.0)
                .strong()
                .background_color(bg_color),
        );
    });
}
