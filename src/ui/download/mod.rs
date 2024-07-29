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

        groups.insert(group.name.clone(), group);

        Self { groups, sender }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        for (group_name, group) in &self.groups {
            ui.group(|ui| {
                ui.label(format!("Group: {}", group_name));

                for (index, download_item) in group.downloads.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("Name: {}", download_item.name));
                        if ui.button("Start").clicked() {
                            self.start_download(group_name, index, ui.ctx());
                        }
                        if ui.button("Cancel").clicked() {
                            self.do_cancel_download(group_name, index);
                        }
                        self.display_progress(ui, &download_item.download.lock().unwrap());
                    });
                }
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

    pub fn do_cancel_download(&self, group_name: &str, index: usize) {
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

    pub fn display_progress(&self, ui: &mut egui::Ui, download: &Download) {
        match download {
            Download::Cancel => {
                ui.label("Cancelado...");
            }
            Download::None => {}
            Download::InProgress(msg) => {
                ui.label(msg);
            }
            Download::Done => {
                ui.label("Concluído");
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
