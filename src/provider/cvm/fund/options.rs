use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
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
    pub path: String,
}

impl Options {
    pub async fn async_path(&self) -> Result<PathBuf, cached_path::Error> {
        let url = self.url.clone();
        let subdir = self.path.clone();
        // Baixa o arquivo usando `cached_path`
        let path = spawn_blocking(move || {
            let res = cached_path_with_options(
                url.as_str(),
                &cached_path::Options::default().subdir(&subdir),
            );

            match res {
                Ok(path) => Ok(path),
                Err(_err) => {
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
        })
        .await
        .unwrap()?;
        // Cria uma cópia do caminho para usar na verificação e na conversão
        let utf8_path = PathBuf::from(format!("{}.utf8", path.display()));
        // Verifica se o arquivo já foi convertido para UTF-8
        if utf8_path.exists() {
            return Ok(utf8_path);
        }
        // Precisamos mover `path` para dentro deste bloco, mas podemos copiar o valor antes de movê-lo
        let utf8_path_clone = utf8_path.clone();
        let path_for_conversion = path.clone();
        // Converte o arquivo para UTF-8
        spawn_blocking(move || {
            let mut contents = Vec::new();
            let mut file = File::open(&path_for_conversion)?;
            file.read_to_end(&mut contents)?;
            // Decodifica usando WIN1252
            let (cow, _, had_errors) = WINDOWS_1252.decode(&contents);
            if had_errors {
                // Tratar erros de conversão, se necessário
                log::error!("Erro ao converter arquivo para UTF-8.");
            }
            // Salva o arquivo convertido
            let mut output_file = File::create(&utf8_path_clone)?;
            output_file.write_all(cow.as_bytes())?;
            // Retorna o caminho do arquivo convertido
            Ok::<_, std::io::Error>(utf8_path_clone)
        })
        .await
        .unwrap()?;
        Ok(utf8_path)
    }

    pub async fn async_path_offline(&self) -> Result<PathBuf, cached_path::Error> {
        let url = self.url.clone();
        let subdir = self.path.clone();
        // Baixa o arquivo usando `cached_path`
        let path = spawn_blocking(move || {
            let c = Cache::builder().offline(true).build()?;
            c.cached_path_with_options(
                url.as_str(),
                &cached_path::Options::default().subdir(&subdir),
            )
        })
        .await
        .unwrap()?;
        let utf8_path = PathBuf::from(format!("{}.utf8", path.display()));
        Ok(utf8_path)
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
