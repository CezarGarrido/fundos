use egui::{Align2, TopBottomPanel, Vec2};
use egui_extras::{Column, TableBuilder};
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::{message::Message, provider::cvm::fund::Class};

pub struct Search {
    sender: UnboundedSender<Message>,
    pub open_window: bool,
    pub query: String,
    pub class: Option<Class>,
    pub result: DataFrame,
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
        }
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Search => {
                self.search_send();
            }
            Msg::SelectClass(selected_class) => {
                self.class = selected_class;
                self.search_send();
            }
        }
    }

    pub fn open(&mut self, value: bool) {
        self.open_window = value;
    }

    pub fn set_result(&mut self, value: DataFrame) {
        self.result = value;
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let mut open = self.open_window;
        egui::Window::new("Fundos")
            .resizable(false)
            .collapsible(false)
            .default_width(550.0)
            .max_width(550.0)
            .max_height(500.0)
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 120.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let search_bar = egui::TextEdit::singleline(&mut self.query)
                    .font(egui::TextStyle::Body)
                    .hint_text("🔍 Busque pelo nome ou cnpj do fundo..")
                    .frame(true)
                    .desired_width(ui.available_width())
                    .margin(egui::vec2(15.0, 10.0));

                let search_response: egui::Response = ui.add(search_bar);
                if search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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
                let cols: Vec<&str> = vec!["CNPJ_FUNDO", "DENOM_SOCIAL"];

                egui::ScrollArea::horizontal().show(ui, |ui| {
                    TableBuilder::new(ui)
                        //.column(Column::auto().at_most(20.0))
                        .column(Column::auto().at_least(40.0).resizable(false))
                        .column(Column::remainder().at_most(40.0))
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .resizable(false)
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.label("cnpj");
                            });
                            header.col(|ui| {
                                ui.label("nome");
                            });
                        })
                        .body(|body| {
                            body.rows(20.0, nr_rows, |mut row| {
                                let row_index = row.index();

                                for col in &cols {
                                    row.col(|ui| {
                                        if let Ok(column) = self.result.column(col) {
                                            if let Ok(value) = column.get(row_index) {
                                                if let Some(value_str) = value.get_str() {
                                                    if col.contains("CNPJ_FUNDO") {
                                                        if ui.link(value_str).clicked() {
                                                            let strcnpj = value_str.to_string();
                                                            let _ = self.sender.send(
                                                                Message::NewTab(strcnpj.clone()),
                                                            );
                                                        }
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

                TopBottomPanel::bottom("top_bottom_window").show_inside(ui, |ui| {
                    ui.add_space(5.0);
                });
            });

        if !open{
            self.query = "".to_string();
            self.class = None;
        }
        
        self.open_window = open;
    }
    fn handle_selectable_value(&mut self, ui: &mut egui::Ui, class: Option<Class>, label: &str) {
        if ui
            .selectable_value(&mut self.class, class.clone(), label)
            .clicked()
        {
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
