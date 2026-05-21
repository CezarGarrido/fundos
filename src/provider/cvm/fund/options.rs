use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use cached_path::{cached_path_with_options, Cache};
use encoding_rs::WINDOWS_1252;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

use crate::config::get;

const ROOT: &str = "cvm.fundo.cadastro";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub url: String,
    pub fallback_url: Option<String>,
    pub path: String,
}

impl Options {
    pub async fn async_path(&self) -> Result<PathBuf, cached_path::Error> {
        let url = self.url.clone();
        let fallback_url = self.fallback_url.clone();
        let subdir = self.path.clone();
        let description = self.description.clone();

        super::set_status(&format!("Baixando cadastro da CVM ({})", description));

        // Baixa o arquivo usando `cached_path`
        let path = spawn_blocking(move || {
            let res = cached_path_with_options(
                url.as_str(),
                &cached_path::Options::default().subdir(&subdir),
            );

            match res {
                Ok(path) => Ok(path),
                Err(err) => {
                    if let Some(fb_url) = fallback_url {
                        super::set_status("Baixando arquivo de contingência (cad_fi.csv)...");
                        log::warn!(
                            "Falha ao baixar URL principal ({}), tentando fallback: {}",
                            err,
                            fb_url
                        );
                        cached_path_with_options(
                            fb_url.as_str(),
                            &cached_path::Options::default().subdir(&subdir),
                        )
                    } else {
                        super::set_status("Carregando do cache local (modo offline)...");
                        let cache = Cache::builder()
                            .progress_bar(Some(cached_path::ProgressBar::Full))
                            .offline(true)
                            .build()?;
                        cache.cached_path_with_options(
                            url.as_str(),
                            &cached_path::Options::default().subdir(&subdir),
                        )
                    }
                }
            }
        })
        .await
        .unwrap()?;

        let result_path = if path.is_dir() {
            // cached_path já extraiu o zip e retornou um diretório
            log::info!("cached_path retornou diretório: {:?}", path);
            Self::ensure_utf8_for_dir(&path).await?
        } else if self.url.split('?').next().unwrap_or("").ends_with(".zip")
            || path.extension().is_some_and(|ext| ext == "zip")
            || Self::is_zip_file(&path)
        {
            // Arquivo zip: extrair e converter para UTF-8
            let utf8_dir = path.parent().unwrap().join("extracted_utf8");
            if !(utf8_dir.exists()
                && utf8_dir.join("registro_classe.csv").exists()
                && utf8_dir.join("registro_fundo.csv").exists())
            {
                Self::extract_and_convert_zip(&path, &utf8_dir).await?;
            }
            utf8_dir
        } else {
            // Arquivo único (ex: cad_fi.csv): converter para UTF-8
            let utf8_path = PathBuf::from(format!("{}.utf8", path.display()));
            Self::convert_single_file_to_utf8(&path, &utf8_path).await?;
            utf8_path
        };

        Ok(result_path)
    }

    fn is_zip_file(path: &Path) -> bool {
        if path.is_dir() {
            return false;
        }
        if let Ok(mut file) = File::open(path) {
            let mut magic = [0u8; 4];
            if file.read_exact(&mut magic).is_ok() {
                return &magic == b"PK\x03\x04";
            }
        }
        false
    }

    /// Extrai um arquivo ZIP e converte todos os CSVs para UTF-8
    async fn extract_and_convert_zip(
        zip_path: &Path,
        utf8_dir: &Path,
    ) -> Result<(), cached_path::Error> {
        // Remove diretório antigo se existir (força reconversão)
        if utf8_dir.exists() {
            log::info!(
                "Removendo diretório UTF-8 existente para recriação: {:?}",
                utf8_dir
            );
            fs::remove_dir_all(utf8_dir).map_err(|e| {
                cached_path::Error::from(std::io::Error::other(format!(
                    "Falha ao remover diretório antigo: {}",
                    e
                )))
            })?;
        }

        super::set_status("Extraindo arquivos compactados (CVM 175)...");
        let zip_path = zip_path.to_path_buf();
        let utf8_dir = utf8_dir.to_path_buf();

        spawn_blocking(move || -> Result<(), std::io::Error> {
            let temp_dir = zip_path.parent().unwrap().join("temp_extracted");
            let _ = fs::create_dir_all(&temp_dir);

            // Extrair ZIP
            {
                let zip_file = File::open(&zip_path)?;
                let mut archive = zip::ZipArchive::new(zip_file)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                for i in 0..archive.len() {
                    let mut file = archive
                        .by_index(i)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    let outpath = temp_dir.join(file.name());

                    if file.name().ends_with('/') {
                        fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            fs::create_dir_all(p)?;
                        }
                        let mut outfile = File::create(&outpath)?;
                        std::io::copy(&mut file, &mut outfile)?;
                    }
                }
            }

            // Converter todos os CSVs para UTF-8 (incluindo subdiretórios)
            super::set_status("Decodificando tabelas para UTF-8...");
            fs::create_dir_all(&utf8_dir)?;
            Self::convert_csv_files_recursively(&temp_dir, &utf8_dir)?;

            // Limpar diretório temporário
            let _ = fs::remove_dir_all(&temp_dir);
            Ok(())
        })
        .await
        .unwrap()?;

        Ok(())
    }

    /// Converte recursivamente todos os arquivos CSV de um diretório para UTF-8
    fn convert_csv_files_recursively(src_dir: &Path, dst_dir: &Path) -> Result<(), std::io::Error> {
        if !src_dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(src_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Processar subdiretórios recursivamente
                let subdir_name = path.file_name().unwrap();
                let dst_subdir = dst_dir.join(subdir_name);
                Self::convert_csv_files_recursively(&path, &dst_subdir)?;
            } else if path.extension().is_some_and(|ext| ext == "csv") {
                // Converter arquivo CSV para UTF-8
                let file_name = path.file_name().unwrap();
                let dst_path = dst_dir.join(file_name);

                let mut contents = Vec::new();
                let mut file = File::open(&path)?;
                file.read_to_end(&mut contents)?;

                // Tentar detectar se já é UTF-8 válido
                if std::str::from_utf8(&contents).is_ok() {
                    // Já é UTF-8 válido, copiar diretamente
                    fs::copy(&path, &dst_path)?;
                    log::debug!("Arquivo já UTF-8: {:?}", path);
                } else {
                    // Converter de Windows-1252 para UTF-8
                    let (cow, _, had_errors) = WINDOWS_1252.decode(&contents);
                    if had_errors {
                        log::warn!("Erro parcial ao decodificar {:?} como Windows-1252", path);
                    }
                    let mut output_file = File::create(&dst_path)?;
                    output_file.write_all(cow.as_bytes())?;
                    log::info!("Convertido para UTF-8: {:?}", path);
                }
            }
        }

        Ok(())
    }

    /// Converte um único arquivo CSV para UTF-8
    async fn convert_single_file_to_utf8(
        file_path: &Path,
        utf8_path: &Path,
    ) -> Result<(), cached_path::Error> {
        if utf8_path.exists() {
            return Ok(());
        }

        if let Some(p) = utf8_path.parent() {
            fs::create_dir_all(p).map_err(|e| {
                cached_path::Error::from(std::io::Error::other(format!(
                    "Falha ao criar diretório: {}",
                    e
                )))
            })?;
        }

        super::set_status("Decodificando arquivo único para UTF-8...");
        let src = file_path.to_path_buf();
        let dst = utf8_path.to_path_buf();

        spawn_blocking(move || -> Result<(), std::io::Error> {
            let mut contents = Vec::new();
            let mut file = File::open(&src)?;
            file.read_to_end(&mut contents)?;

            // Tentar detectar se já é UTF-8 válido
            if std::str::from_utf8(&contents).is_ok() {
                fs::copy(&src, &dst)?;
                log::debug!("Arquivo único já UTF-8: {:?}", src);
            } else {
                let (cow, _, had_errors) = WINDOWS_1252.decode(&contents);
                if had_errors {
                    log::warn!("Erro parcial ao decodificar {:?} como Windows-1252", src);
                }
                let mut output_file = File::create(&dst)?;
                output_file.write_all(cow.as_bytes())?;
                log::info!("Arquivo único convertido para UTF-8: {:?}", src);
            }
            Ok(())
        })
        .await
        .unwrap()?;

        Ok(())
    }

    /// Garante que todos os CSVs em um diretório existente estejam em UTF-8
    async fn ensure_utf8_for_dir(dir: &Path) -> Result<PathBuf, cached_path::Error> {
        let utf8_dir = dir.parent().unwrap_or(dir).join(format!(
            "{}-utf8",
            dir.file_name().unwrap().to_string_lossy()
        ));

        if utf8_dir.exists()
            && utf8_dir.join("registro_classe.csv").exists()
            && utf8_dir.join("registro_fundo.csv").exists()
        {
            return Ok(utf8_dir);
        }

        super::set_status("Convertendo arquivos do diretório para UTF-8...");
        let src_dir = dir.to_path_buf();
        let dst_dir = utf8_dir.clone();

        spawn_blocking(move || -> Result<(), std::io::Error> {
            fs::create_dir_all(&dst_dir)?;
            Options::convert_csv_files_recursively(&src_dir, &dst_dir)?;
            Ok(())
        })
        .await
        .unwrap()?;

        Ok(utf8_dir)
    }

    pub async fn async_path_offline(&self) -> Result<PathBuf, cached_path::Error> {
        let url = self.url.clone();
        let subdir = self.path.clone();

        let path = spawn_blocking(move || {
            let c = Cache::builder().offline(true).build()?;
            c.cached_path_with_options(
                url.as_str(),
                &cached_path::Options::default().subdir(&subdir),
            )
        })
        .await
        .unwrap()?;

        let is_zip = self.url.split('?').next().unwrap_or("").ends_with(".zip")
            || path.extension().is_some_and(|ext| ext == "zip")
            || Self::is_zip_file(&path);

        let result_path = if path.is_dir() {
            let utf8_dir = path.parent().unwrap_or(&path).join(format!(
                "{}-utf8",
                path.file_name().unwrap().to_string_lossy()
            ));
            if !utf8_dir.exists() {
                log::warn!(
                    "Diretório UTF-8 não encontrado em modo offline: {:?}",
                    utf8_dir
                );
            }
            utf8_dir
        } else if is_zip {
            let utf8_dir = path.parent().unwrap().join("extracted_utf8");
            if !utf8_dir.exists() {
                log::warn!(
                    "Diretório extraído não encontrado em modo offline: {:?}",
                    utf8_dir
                );
            }
            utf8_dir
        } else {
            let utf8_path = PathBuf::from(format!("{}.utf8", path.display()));
            if !utf8_path.exists() {
                log::warn!(
                    "Arquivo UTF-8 não encontrado em modo offline: {:?}",
                    utf8_path
                );
            }
            utf8_path
        };

        Ok(result_path)
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
