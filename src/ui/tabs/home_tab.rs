use super::Tab;
use crate::{history::History, message::Message, ui::download::DownloadManager};
use egui::{CentralPanel, Frame, Ui, WidgetText};
use tokio::sync::mpsc;

pub struct HomeTab {
    pub title: String,
    pub history: History,
    pub sender: mpsc::UnboundedSender<Message>,
    pub download_manager: DownloadManager,
}

impl HomeTab {
    pub fn new(title: String, sender: mpsc::UnboundedSender<Message>, history: History) -> Self {
        let dm = DownloadManager::new(sender.clone());

        HomeTab {
            title,
            sender,
            history,
            download_manager: dm,
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
                        });

                        ui.add_space(50.0);

                        ui.vertical(|ui| {
                            ui.set_max_width(500.0);
                            ui.heading("Estatisticas".to_string());
                            ui.add_space(5.0);
                            if ui.link("Estatisticas de Fundos").clicked() {
                                let _ = self.sender.send(Message::OpenDashboardTab);
                            }
                            ui.add_space(5.0);
                        });

                        ui.add_space(50.0);
                        ui.label("Recentes");
                        ui.vertical(|ui| {
                            for cnpj in self.history.get_most_accesseds() {
                                if ui
                                    .link(format!(
                                        "{} {}",
                                        cnpj.clone(),
                                        egui_phosphor::regular::ARROW_SQUARE_UP_RIGHT,
                                    ))
                                    .clicked()
                                {
                                    let _ = self.sender.send(Message::NewTab(cnpj.clone()));
                                }
                            }
                        });

                        ui.add_space(50.0);

                        ui.vertical(|ui| {
                            ui.set_max_width(500.0);
                            ui.heading("Downloads".to_string());
                            ui.add_space(10.0);
                            //ui.separator();
                            //   Frame::none().inner_margin(10.0).show(ui, |ui| {
                            //     ui.group(|ui| {
                            self.download_manager.ui(ui);
                            //   });
                            //   });
                        });
                    });
                });
            });
        });
    }
}
