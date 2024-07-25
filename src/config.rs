use egui::{Align2, Vec2};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::{
    downloader::{Document, Download, Downloader},
    message::Message,
};

#[derive(Clone)]
pub struct Config {
    pub open: bool,
    pub downloader: Downloader,
}

impl Config {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        let docs = vec![
            Document {
                description: String::from("Informação Cadastral"),
                url: String::from("https://dados.cvm.gov.br/dados/FI/CAD/DADOS/cad_fi.csv"),
                hist: false,
                ext: String::from("csv"),
                filename: "cad_fi.csv".to_owned(),
                pattern: "".to_owned(),
                pattern_hist: "".to_owned(),
                download_path: "./dataset/cad".to_owned(),
                limit_years: -1,
            },
            Document {
                description: String::from("CDI Acumulado"),
                url: String::from(
                    "https://api.bcb.gov.br/dados/serie/bcdata.sgs.12/dados?formato=json",
                ),
                hist: false,
                ext: String::from("json"),
                filename: "bcdata.sgs.12.json".to_owned(),
                pattern: "".to_owned(),
                pattern_hist: "".to_owned(),
                download_path: "./dataset/cdi".to_owned(),
                limit_years: -1,
            },
            Document {
                description: String::from("CDA"),
                url: String::from("https://dados.cvm.gov.br/dados/FI/DOC/CDA/DADOS"),
                hist: false,
                ext: String::from("zip"),
                pattern: "cda_fi_{ano}{mes}".to_owned(),
                pattern_hist: "cda_fi_{ano}".to_owned(),
                download_path: "./dataset/cda".to_owned(),
                filename: "".to_owned(),
                limit_years: -1,
            },
            Document {
                description: String::from("Informes Diários"),
                url: String::from("https://dados.cvm.gov.br/dados/FI/DOC/INF_DIARIO/DADOS"),
                hist: false,
                ext: String::from("zip"),
                pattern: "inf_diario_fi_{ano}{mes}".to_owned(),
                pattern_hist: "inf_diario_fi_{ano}".to_owned(),
                download_path: "./dataset/infdiario".to_owned(),
                filename: "".to_owned(),
                limit_years: 3,
            },
        ];

        let mut downloads = Vec::new();

        for (i, d) in docs.clone().iter().enumerate() {
            downloads.push(Download {
                document: d.clone(),
                token: CancellationToken::new(),
                progress: String::from(""),
                id: i,
            })
        }

        let downloader = Downloader::new(downloads.clone(), sender.clone());

        Self {
            open: false,
            downloader,
        }
    }
}

impl Config {
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        egui::Window::new("Arquivos")
            .resizable(false)
            .collapsible(false)
            .max_width(500.0)
            .max_height(500.0)
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
            .open(&mut open)
            .show(ctx, |ui| self.downloader.ui(ui));
        self.open = open;
    }
}
