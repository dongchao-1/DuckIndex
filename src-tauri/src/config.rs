use std::fs::{self};

use config::{Config, File};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub data_path: Vec<String>,
    pub index_path: String,
}


impl AppConfig {
    pub fn load(app_handle: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        // 获取 Tauri 应用的运行目录（推荐使用 app_dir）
        let config_path = tauri::path::PathResolver::app_config_dir(app_handle.path()).unwrap();

        println!("config_path: {:?}", config_path);

        let config_file = config_path.join("config.yaml");
        if !config_file.exists() {
            fs::create_dir_all(config_path)?;

            let index_path = tauri::path::PathResolver::app_data_dir(app_handle.path()).unwrap().join("index");
            println!("index_path: {:?}", index_path);
            if !index_path.exists() {
                fs::create_dir_all(&index_path)?;
            }

            let default_config = AppConfig {
                data_path: vec![],
                index_path: index_path.to_string_lossy().to_string(),
            };
            let yaml = serde_yaml::to_string(&default_config)?;
            fs::write(&config_file, yaml)?;
        }

        Ok(Config::builder()
            .add_source(File::from(config_file))
            .build()?
            .try_deserialize::<AppConfig>()?)
    }
}
