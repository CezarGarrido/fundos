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
                ui.heading("Começar");
                ui.add_space(5.0);
                ui.vertical(|ui| {
                    if ui.small_button("Pesquisar...").clicked() {
                        let _ = self.sender.send(Message::OpenSearchWindow(true));
                    }
                    let _ = ui.small_button("Abrir Configuração..");
                    let _ = ui.small_button("Mudar Tema");
                });

                ui.add_space(50.0);
                ui.label("Recentes");
                ui.vertical(|ui| {
                    for cnpj in self.history.get_most_accesseds() {
                        if ui.link(cnpj.clone()).clicked() {
                            let _ = self.sender.send(Message::NewTab(cnpj.clone()));
                        }
                    }
                });

                ui.add_space(50.0);
                ui.group(|ui| {
                    ui.heading(format!("{} Downloads", egui_phosphor::regular::DOWNLOAD));
                    ui.separator();
                    self.download_manager.ui(ui);
                });
            });
        });
    }
}
