use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::providers::Provider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: Provider,
    pub model_name: String,
}

impl Config {
    fn get_config_path() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not determine home directory"))?;
        Ok(home_dir.join(".fs-organiser").join("config.json"))
    }

    pub fn load() -> Result<Option<Config>> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(Some(config))
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        
        println!("Configuration saved to: {}", config_path.display());
        Ok(())
    }

    pub fn get_config_file_path() -> Result<PathBuf> {
        Self::get_config_path()
    }
}