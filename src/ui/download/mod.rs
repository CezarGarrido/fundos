use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use eframe::egui;

use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;

#[derive(Clone)]
pub struct DownloadItem {
    pub id: String,
    pub name: String,
    pub download: Arc<Mutex<Download>>,
}

#[derive(Clone)]
pub enum Download {
    None,
    Cancel,
    InProgress(String),
    Done,
}

pub struct Group {
    pub name: String,
    pub downloads: Vec<DownloadItem>,
}

pub struct DownloadManager {
    groups: HashMap<String, Group>,
    sender: UnboundedSender<Message>,
}

impl DownloadManager {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        let mut groups = HashMap::new();
        let mut group = Group {
            name: "Indices".to_string(),
            downloads: Vec::new(),
        };

        group.downloads.push(DownloadItem {
            id: "CDI".to_string(),
            name: "CDI".to_string(),
            download: Arc::new(Mutex::new(Download::None)),
        });

        group.downloads.push(DownloadItem {
            id: "IBOV".to_string(),
            name: "Ibovespa".to_string(),
            download: Arc::new(Mutex::new(Download::None)),
        });

        let mut fundo = Group {
            name: "Fundos".to_string(),
            downloads: Vec::new(),
        };

        fundo.downloads.push(DownloadItem {
            id: "cad".to_string(),
            name: "Informação Cadastral".to_string(),
            download: Arc::new(Mutex::new(Download::None)),
        });

        fundo.downloads.push(DownloadItem {
            id: "informe".to_string(),
            name: "Informes Diários".to_string(),
            download: Arc::new(Mutex::new(Download::None)),
        });

        fundo.downloads.push(DownloadItem {
            id: "carteira".to_string(),
            name: "Composição da Carteira".to_string(),
            download: Arc::new(Mutex::new(Download::None)),
        });

        groups.insert(fundo.name.clone(), fundo);
        groups.insert(group.name.clone(), group);

        Self { groups, sender }
    }

    /// Cria uma lista temporária de tuplas contendo (nome do grupo, índice, download_item)
    fn all_downloads(&self) -> Vec<(String, usize, &DownloadItem)> {
        let mut all_downloads = Vec::new();
        for (group_name, group_data) in &self.groups {
            for (index, download_item) in group_data.downloads.iter().enumerate() {
                all_downloads.push((group_name.clone(), index, download_item));
            }
        }
        all_downloads
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        let all_downloads = self.all_downloads();
        for (all_index, (group_name, index, download_item)) in all_downloads.iter().enumerate() {
            if all_index > 0 {
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.label(download_item.name.to_string());
                    self.display_progress(ui, &download_item.download.lock().unwrap());
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        self.start_or_cancel(
                            ui,
                            group_name,
                            *index,
                            &download_item.download.lock().unwrap(),
                        )
                    });
                });
            });
        }
    }

    pub fn start_download(&self, group_name: &str, index: usize, _egui_ctx: &egui::Context) {
        if let Some(item) = self.find_download_item(group_name, index) {
            let _ = self.sender.send(Message::StartDownload(
                group_name.to_string(),
                index,
                item.clone(),
            ));
        }
    }

    pub fn cancel_download(&self, group_name: &str, index: usize) {
        if self.find_download_item(group_name, index).is_some() {
            let _ = self
                .sender
                .send(Message::CancelDownload(format!("{}_{}", group_name, index)));
        }
    }

    pub fn update_download(&self, group_name: &str, index: usize, dl: Download) {
        if let Some(download_item) = self.find_download_item(group_name, index) {
            *download_item.download.lock().unwrap() = dl;
        }
    }

    pub fn start_or_cancel(
        &self,
        ui: &mut egui::Ui,
        group_name: &str,
        index: usize,
        download: &Download,
    ) {
        match download {
            Download::None | Download::Done | Download::Cancel => {
                if ui.button("Baixar").clicked() {
                    self.start_download(group_name, index, ui.ctx());
                }
            }
            Download::InProgress(_) => {
                if ui.button("Cancelar").clicked() {
                    self.cancel_download(group_name, index);
                }
            }
        }
    }

    pub fn display_progress(&self, ui: &mut egui::Ui, download: &Download) {
        match download {
            Download::None => {}
            Download::Cancel => {
                ui.weak("Cancelado");
            }
            Download::InProgress(msg) => {
                ui.weak(msg);
            }
            Download::Done => {
                ui.weak("Concluído");
            }
        }
    }

    fn find_download_item(&self, group_name: &str, index: usize) -> Option<&DownloadItem> {
        self.groups.get(group_name).and_then(|group| {
            if index < group.downloads.len() {
                Some(&group.downloads[index])
            } else {
                None
            }
        })
    }
}
