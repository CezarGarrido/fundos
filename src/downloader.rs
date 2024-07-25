use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use chrono::{Datelike, Utc};
use curl::easy::Easy;
use egui::{Align, Button, Layout};
use partialzip::partzip::PartialZip;
use std::{
    fs::{self, File},
    io::Write,
};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use encoding_rs::WINDOWS_1252;

use crate::message::Message;

#[derive(Clone)]
pub struct Downloader {
    sender: mpsc::UnboundedSender<Message>,
    downloads: Vec<Download>,
}

impl Downloader {
    pub fn new(downloads: Vec<Download>, sender: mpsc::UnboundedSender<Message>) -> Self {
        Self { downloads, sender }
    }

    pub fn start_download_one(&mut self, idx: usize, ctx: &egui::Context) {
        let d = self.downloads.get_mut(idx).unwrap();
        let sender = self.sender.clone();
        let d_clone = d.clone();
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            d_clone.download(sender, &ctx_clone).await;
        });
    }

    pub fn cancel_one(&mut self, idx: usize) {
        let t = self.downloads.get_mut(idx).unwrap();
        t.token.cancel();
        let _ = self
            .sender
            .send(Message::DownloadMessage(idx, "cancelando...".to_string()));
        t.token = CancellationToken::new();
    }

    pub fn update_download(&mut self, idx: usize, progress: String) {
        self.downloads.get_mut(idx).unwrap().progress = progress;
    }

    pub fn get_downloads(&self) -> Vec<Download> {
        self.downloads.clone()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut downloads = self.get_downloads();
        for (i, d) in downloads.iter_mut().enumerate() {
            if i != 0 {
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.label(format!("{}", d.document.description));
            });

            ui.horizontal(|ui| {
                ui.label("Histórico? ");
                if d.document.hist {
                    ui.weak("Sim");
                } else {
                    ui.weak("Não");
                }
            });

            ui.horizontal(|ui| {
                ui.weak(d.progress.to_string());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(Button::new("Cancelar")).clicked() {
                        self.cancel_one(i);
                    }

                    if ui
                        .add(Button::new(format!(
                            "{} Baixar",
                            egui_phosphor::regular::DOWNLOAD
                        )))
                        .clicked()
                    {
                        self.start_download_one(i, ui.ctx());
                    }
                });
            });
        }
    }
}

#[derive(Clone)]
pub struct Document {
    pub description: String,
    pub url: String,
    pub hist: bool,
    pub ext: String,
    pub filename: String,
    pub pattern: String,
    pub pattern_hist: String,
    pub download_path: String,
    pub limit_years: i32,
}

impl Document {
    fn historical_years(&self) -> Vec<i32> {
        let current_year = Utc::now().year();
        (2005..=current_year).collect()
    }

    fn make_historical_date(&self, year: i32) -> String {
        self.pattern_hist.replace("{ano}", &year.to_string())
    }

    fn generate_dates(&self, initial_year: i32) -> Vec<String> {
        let current_year = Utc::now().year();
        let current_month = Utc::now().month();
        let mut dates = Vec::new();
        let ini_year = if self.limit_years > 0 {
            current_year - self.limit_years
        } else {
            initial_year
        };

        for year in ini_year..=current_year {
            let end_month = if year == current_year {
                current_month
            } else {
                12
            };
            for month in 1..=end_month {
                let month_str = format!("{:02}", month);
                let date = self
                    .pattern
                    .replace("{ano}", &year.to_string())
                    .replace("{mes}", &month_str);
                dates.push(date);
            }
        }
        dates
    }
}

#[derive(Clone)]
pub struct Download {
    pub id: usize,
    pub token: CancellationToken,
    pub progress: String,
    pub document: Document,
}

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Operation was cancelled")]
    Cancelled,
    #[error("Failed to download file: {0}")]
    DownloadFailed(String),
    #[error("Failed to create directory: {0}")]
    CreateDirFailed(String),
    #[error("Failed to write file: {0}")]
    WriteFileFailed(String),
    #[error("Failed to decode file: {0}")]
    DecodeFailed(String),
}

impl Download {
    pub async fn download(&self, sender: mpsc::UnboundedSender<Message>, ctx: &egui::Context) {
        println!("Baixando..");

        self.update_progress("preparando...".to_string(), sender.clone(), ctx);

        let result = if self.document.ext == "zip" {
            let current_year = Utc::now().year();
            let initial_year = if self.document.hist && self.document.limit_years <= 0 {
                let res = self
                    .download_historical(&self.token, sender.clone(), ctx)
                    .await;
                match res {
                    Ok(value) => match value {
                        Some(year) => year,
                        None => current_year,
                    },
                    Err(_) => current_year,
                }
            } else {
                current_year
            };

            self.download_zip(initial_year, &self.token, sender.clone(), ctx)
                .await
        } else {
            self.download_file(&self.token)
        };

        match result {
            Ok(_) => self.update_progress("concluido".to_string(), sender.clone(), ctx),
            Err(DownloadError::Cancelled) => {
                self.update_progress("cancelado".to_string(), sender, ctx)
            }
            Err(err) => {
                println!("{}", err);
                self.update_progress("falha".to_string(), sender, ctx)
            }
        }
    }

    async fn download_historical(
        &self,
        token: &CancellationToken,
        sender: mpsc::UnboundedSender<Message>,
        ctx: &egui::Context,
    ) -> Result<Option<i32>, DownloadError> {
        let historical = self.document.historical_years();
        for (i, year) in historical.iter().enumerate() {
            if token.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }

            let final_url = format!(
                "{}/HIST/{}.zip",
                &self.document.url,
                &self.document.make_historical_date(year.clone())
            );

            let msg: String = format!(
                "({}/{}) Baixando histórico do ano {}",
                year.clone(),
                i,
                historical.len()
            );
            self.update_progress(msg, sender.clone(), ctx);
            match self.download_and_save(&final_url, "hist", token).await {
                Ok(_) => continue,
                Err(_) => return Ok(Some(year.clone())),
            }
        }
        Ok(None)
    }

    async fn download_zip(
        &self,
        initial_year: i32,
        token: &CancellationToken,
        sender: mpsc::UnboundedSender<Message>,
        ctx: &egui::Context,
    ) -> Result<(), DownloadError> {
        let dates = self.document.generate_dates(initial_year);

        for (i, date) in dates.iter().enumerate() {
            if token.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            let count = i + 1;
            let msg: String = format!(
                "({}/{}) Baixando arquivo {}...",
                count,
                dates.len() - 1,
                date.clone()
            );

            self.update_progress(msg, sender.clone(), ctx);
            let final_url = format!("{}/{}.zip", self.document.url, date);
            self.download_and_save(&final_url, &date, token).await?;
        }
        Ok(())
    }

    async fn download_and_save(
        &self,
        url: &str,
        subfolder: &str,
        token: &CancellationToken,
    ) -> Result<(), DownloadError> {
        let mut pz = PartialZip::new_check_range(&url.to_string(), true)
            .map_err(|e| DownloadError::DownloadFailed(e.to_string()))?;

        for filename in pz.list_names() {
            if token.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }

            let mut final_path = format!(
                "{}/{}/{}",
                &self.document.download_path, subfolder, filename
            );

            if subfolder.is_empty() {
                final_path = format!("{}/{}", &self.document.download_path, filename);
            }

            let final_dir = Path::new(&final_path).parent().unwrap();
            fs::create_dir_all(final_dir)
                .map_err(|e| DownloadError::CreateDirFailed(e.to_string()))?;

            let mut file = File::create(&final_path)
                .map_err(|e| DownloadError::WriteFileFailed(e.to_string()))?;
            let csv_file: Vec<u8> = pz
                .download(&filename)
                .map_err(|e| DownloadError::DownloadFailed(e.to_string()))?;
            let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&csv_file);
            if had_errors {
                return Err(DownloadError::DecodeFailed(filename));
            }
            file.write_all(decoded_str.as_bytes())
                .map_err(|e| DownloadError::WriteFileFailed(e.to_string()))?;
        }
        Ok(())
    }

    fn download_file(&self, token: &CancellationToken) -> Result<(), DownloadError> {
        fs::create_dir_all(&self.document.download_path)
            .map_err(|e| DownloadError::CreateDirFailed(e.to_string()))?;

        let final_output_file = format!(
            "{}/{}",
            &self.document.download_path, &self.document.filename
        );
        let mut easy = Easy::new();
        easy.url(&self.document.url)
            .map_err(|e| DownloadError::DownloadFailed(e.to_string()))?;
        let path = Path::new(&final_output_file);
        let mut file = File::options()
            .write(true)
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| DownloadError::WriteFileFailed(e.to_string()))?;

        let existing_size = file
            .metadata()
            .map_err(|e| DownloadError::WriteFileFailed(e.to_string()))?
            .len();
        if existing_size > 0 {
            easy.resume_from(existing_size)
                .map_err(|e| DownloadError::DownloadFailed(e.to_string()))?;
        }

        let tk = token.clone();
        let error: Arc<Mutex<Option<DownloadError>>> = Arc::new(Mutex::new(None));
        let error_clone = Arc::clone(&error);

        easy.write_function(move |data| {
            if tk.is_cancelled() {
                let mut error_lock = error_clone.lock().unwrap();
                *error_lock = Some(DownloadError::Cancelled);
                return Ok(0);
            }

            let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&data);
            if had_errors {
                let mut error_lock = error_clone.lock().unwrap();
                *error_lock = Some(DownloadError::DecodeFailed(
                    "error decode windows 1252".to_string(),
                ));
                return Ok(0);
            }

            if let Err(e) = file.write_all(decoded_str.as_bytes()) {
                let mut error_lock = error_clone.lock().unwrap();
                *error_lock = Some(DownloadError::WriteFileFailed(e.to_string()));
                return Ok(0);
            }

            Ok(data.len())
        })
        .unwrap();

        if let Err(e) = easy.perform() {
            if let Some(err) = error.lock().unwrap().take() {
                return Err(err);
            }

            return Err(DownloadError::DownloadFailed(e.to_string()));
        }

        if let Some(err) = error.lock().unwrap().take() {
            return Err(err);
        }

        Ok(())
    }

    fn update_progress(
        &self,
        progress: String,
        sender: mpsc::UnboundedSender<Message>,
        ctx: &egui::Context,
    ) {
        let _ = sender.send(Message::DownloadMessage(self.id, progress));
        ctx.request_repaint();
    }
}
