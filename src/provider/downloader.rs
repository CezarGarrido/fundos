use crate::provider::{
    cvm::{
        fund::{self},
        informe::{self},
        portfolio::{self},
    },
    indices::{self},
};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use ehttp::Request;
use encoding_rs::WINDOWS_1252;
use log::error;
use serde_json::{to_writer_pretty, Value};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use zip::read::ZipArchive;

use crate::provider::cache::{get_cached_headers, get_path, update_cache};

use super::indices::ibovespa;

#[derive(Clone, PartialEq)]
pub enum DownloadStatus {
    Done,
    InProgress(String),
    Failed(String),
    Cancelled,
    NoChange,
}

#[derive(Clone)]
pub struct DownloadItem {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub urls: Vec<String>,
    pub download: Arc<Mutex<DownloadStatus>>,
}

pub struct Group {
    pub name: String,
    pub downloads: Vec<DownloadItem>,
}

fn make_groups() -> HashMap<String, Group> {
    let mut groups = HashMap::new();
    // Índices
    let cdi_opts = indices::cdi::options::load().unwrap();
    let ibov_opts = indices::ibovespa::options::load().unwrap();
    let mut indices_group = Group {
        name: "Indices".to_string(),
        downloads: Vec::new(),
    };

    indices_group.downloads.push(DownloadItem {
        id: "CDI".to_string(),
        name: cdi_opts.description.clone(),
        path: cdi_opts.path.clone(),
        urls: cdi_opts.urls(),
        download: Arc::new(Mutex::new(DownloadStatus::Done)),
    });

    indices_group.downloads.push(DownloadItem {
        id: "IBOV".to_string(),
        name: ibov_opts.description,
        path: ibov_opts.path,
        urls: vec![],
        download: Arc::new(Mutex::new(DownloadStatus::Done)),
    });

    groups.insert(indices_group.name.clone(), indices_group);

    // Fundos
    let fund_opts = fund::options::load().unwrap();
    let portfolio_opts = portfolio::options::load().unwrap();
    let informe_opts = informe::options::load().unwrap();

    let mut fundos_group = Group {
        name: "Fundos".to_string(),
        downloads: Vec::new(),
    };

    fundos_group.downloads.push(DownloadItem {
        id: "cad".to_string(),
        name: fund_opts.description,
        path: fund_opts.path,
        urls: vec![fund_opts.url],
        download: Arc::new(Mutex::new(DownloadStatus::Done)),
    });

    fundos_group.downloads.push(DownloadItem {
        id: "informe".to_string(),
        name: informe_opts.description.clone(),
        path: informe_opts.path.clone(),
        urls: informe_opts.urls(),
        download: Arc::new(Mutex::new(DownloadStatus::Done)),
    });

    fundos_group.downloads.push(DownloadItem {
        id: "carteira".to_string(),
        name: portfolio_opts.description.clone(),
        path: portfolio_opts.path.clone(),
        urls: portfolio_opts.urls(),
        download: Arc::new(Mutex::new(DownloadStatus::Done)),
    });

    groups.insert(fundos_group.name.clone(), fundos_group);
    groups
}

pub fn download_all(
    token: CancellationToken,
    on_progress: impl 'static + Send + FnMut(DownloadStatus) + Clone,
    max_concurrent_downloads: usize,
) {
    let groups = make_groups();

    let total_urls: usize = groups
        .iter()
        .map(|(_, group_data)| {
            group_data
                .downloads
                .iter()
                .map(|d| d.urls.len())
                .sum::<usize>()
        })
        .sum();

    let completed = Arc::new(Mutex::new(0));
    let on_progress = Arc::new(Mutex::new(on_progress));
    let semaphore = Arc::new(Semaphore::new(max_concurrent_downloads));

    let msg = format!("Baixando: {}/{}", 0, total_urls);
    let mut on_progress1 = on_progress.lock().unwrap();
    on_progress1(DownloadStatus::InProgress(msg));

    let mut futures: Vec<JoinHandle<()>> = Vec::new();

    for (group_name, group_data) in groups {
        for download_item in group_data.downloads {
            if download_item.id == "IBOV" {
                let token = token.clone();
                let completed = Arc::clone(&completed);
                let on_progress = Arc::clone(&on_progress);
                let semaphore = Arc::clone(&semaphore);
                let id = download_item.id.clone();
                let total_urls = total_urls.clone();
                let group_name = group_name.clone();

                let future = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    ibovespa::download(token, move |_| {
                        let mut completed_guard = completed.lock().unwrap();
                        *completed_guard += 1;
                        let new_completed = *completed_guard;
                        let msg = format!(
                            "Baixando: {}/{} ({} - {})",
                            new_completed, total_urls, group_name, id
                        );
    
                        let mut on_progress = on_progress.lock().unwrap();
                        on_progress(DownloadStatus::InProgress(msg));
                        if new_completed == total_urls {
                            on_progress(DownloadStatus::Done);
                        }
                    });
                });
                futures.push(future);
                continue;
            }

            for url in download_item.urls {
                let token = token.clone();
                let completed = Arc::clone(&completed);
                let on_progress = Arc::clone(&on_progress);
                let semaphore = Arc::clone(&semaphore);
                let path = download_item.path.clone();
                let group_name = group_name.clone();
                let id = download_item.id.clone();
                let total_urls = total_urls.clone();

                let future = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    if token.is_cancelled() {
                        let mut on_progress = on_progress.lock().unwrap();
                        on_progress(DownloadStatus::Cancelled);
                        return;
                    }

                    let result = download_file(&url, path.clone()).await;
                    if let Err(e) = result {
                        log::error!("Falha ao baixar {}: {}", url, e);
                    }

                    let mut completed_guard = completed.lock().unwrap();
                    *completed_guard += 1;
                    let new_completed = *completed_guard;

                    let msg = format!(
                        "Baixando: {}/{} ({} - {})",
                        new_completed, total_urls, group_name, id
                    );

                    let mut on_progress = on_progress.lock().unwrap();
                    on_progress(DownloadStatus::InProgress(msg));
                    if new_completed == total_urls {
                        on_progress(DownloadStatus::Done);
                    }
                });
                futures.push(future);
            }
        }
    }

    tokio::spawn(async move {
        for handle in futures {
            let _res = handle.await;
        }
    });
}

pub async fn download_file(
    url: &str,
    download_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut request = Request::get(url);
    let mut file_path = download_path.clone();

    if file_path.is_dir() {
        let file_name = url.split('/').last().unwrap_or("downloaded_file");
        file_path = download_path.join(file_name);
    }

    if let Some(path) = get_path(url) {
        let p = Path::new(&path);
        if p.exists() {
            request.headers = get_cached_headers(url);
        }
    }

    let response = ehttp::fetch_blocking(&request)?;
    if response.status == 304 {
        return Ok(());
    }

    let content_type = response.content_type().unwrap_or_default();
    let content_type = content_type.split(';').next().unwrap_or_default();

    match content_type {
        "text/csv" | "application/octet-stream" => {
            let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&response.bytes);
            if had_errors {
                error!("Erro ao decodificar CSV: {:?}", file_path);
            }

            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if file_path.is_dir() {
                let file_name = url.split('/').last().unwrap_or("downloaded_file");
                file_path = download_path.join(file_name);
            }

            let mut file = File::create(&file_path)?;
            file.write_all(&decoded_str.as_bytes())?;
            if let Err(err) = update_cache(url, file_path, response.headers) {
                eprintln!("Erro ao armazenar os cabeçalhos: {}", err);
            }
        }
        "application/json" => {
            let json: Value = serde_json::from_slice(&response.bytes)?;
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if file_path.is_dir() {
                let file_name = url.split('/').last().unwrap_or("downloaded_file");
                file_path = download_path.join(file_name);
            }

            let file = File::create(&file_path)?;
            to_writer_pretty(file, &json)?;
            if let Err(err) = update_cache(url, file_path, response.headers) {
                eprintln!("Erro ao armazenar os cabeçalhos: {}", err);
            }
        }
        "application/zip" => {
            let mut archive = ZipArchive::new(Cursor::new(&response.bytes))?;
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = download_path.join(file.name());

                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(&p)?;
                    }
                }

                let mut bytes = Vec::new();
                io::copy(&mut file, &mut bytes)?;

                let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&bytes);
                if had_errors {
                    error!("Erro ao decodificar CSV: {:?}", outpath);
                    continue;
                }
                let mut outfile = File::create(&outpath)?;
                outfile.write_all(decoded_str.as_bytes())?;
            }

            if file_path.is_dir() {
                let file_name = url.split('/').last().unwrap_or("downloaded_file");
                file_path = download_path.join(file_name);
            }

            if let Err(err) = update_cache(url, file_path, response.headers) {
                eprintln!("Erro ao armazenar os cabeçalhos: {}", err);
            }
        }
        _ => {
            println!(
                "Tipo de arquivo não suportado: {} {} {}",
                content_type, url, response.status,
            );
        }
    }
    Ok(())
}
