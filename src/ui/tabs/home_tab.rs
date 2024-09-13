use super::Tab;
use crate::{history::History, message::Message};
use egui::{CentralPanel, Frame, Ui, WidgetText};
use tokio::sync::mpsc;

pub struct HomeTab {
    pub title: String,
    pub history: History,
    pub sender: mpsc::UnboundedSender<Message>,
}

impl HomeTab {
    pub fn new(title: String, sender: mpsc::UnboundedSender<Message>, history: History) -> Self {
        HomeTab {
            title,
            sender,
            history,
        }
    }
}

impl Tab for HomeTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut Ui) {
        CentralPanel::default().show_inside(ui, |ui| {
            Frame::none().inner_margin(45.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() / 2.0);

                        ui.heading("Começar");
                        ui.add_space(5.0);
                        ui.vertical(|ui| {
                            if ui
                                .small_button(format!(
                                    "{} Pesquisar...",
                                    egui_phosphor::regular::MAGNIFYING_GLASS
                                ))
                                .clicked()
                            {
                                let _ = self.sender.send(Message::OpenSearchWindow(true));
                            }
                            if ui
                                .small_button(format!(
                                    "{} Visão Geral...",
                                    egui_phosphor::regular::LIST_DASHES
                                ))
                                .clicked()
                            {
                                let _ = self.sender.send(Message::OpenDashboardTab);
                            }
                        });

                        ui.add_space(50.0);

                        ui.label("Visto Recentemente");
                        ui.vertical(|ui| {
                            for (cnpj, name) in self.history.get_most_accesseds() {
                                let link_btn = ui
                                    .link(format!(
                                        "{} {}",
                                        cnpj.clone(),
                                        egui_phosphor::regular::ARROW_SQUARE_UP_RIGHT,
                                    ))
                                    .on_hover_ui(|ui| {
                                        ui.label(name.clone());
                                    });

                                if link_btn.clicked() {
                                    let _ = self.sender.send(Message::NewTab(cnpj.clone()));
                                }
                            }
                        });
                    });
                });
            });
        });
    }
}
