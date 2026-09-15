//! Konfigurasi database: driver dipilih saat runtime.
//!
//! Berkas `peoplex.config.toml` di direktori data. Bila belum ada,
//! ditulis default (SQLite lokal) agar instalasi lama tetap jalan.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbConfig {
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
}

fn default_driver() -> String {
    "sqlite".to_string()
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            driver: default_driver(),
            path: None,
            host: None,
            port: None,
            user: None,
            password_env: None,
            database: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub database: DbConfig,
}

/// Muat config dari direktori data; tulis default bila belum ada.
pub fn load(data_dir: &Path) -> Result<AppConfig, String> {
    let path = data_dir.join("peoplex.config.toml");
    if !path.exists() {
        let cfg = AppConfig::default();
        let text = toml::to_string_pretty(&cfg)
            .map_err(|e| format!("gagal menulis config default: {e}"))?;
        std::fs::write(&path, text).map_err(|e| format!("gagal menyimpan config: {e}"))?;
        return Ok(cfg);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("gagal membaca config: {e}"))?;
    toml::from_str(&text).map_err(|e| format!("config tidak valid: {e}"))
}

impl AppConfig {
    /// Konfigurasi SQLite yang menunjuk satu berkas di dalam direktori data.
    pub fn sqlite_file(file_name: &str) -> Self {
        Self {
            database: DbConfig {
                driver: "sqlite".to_string(),
                path: Some(file_name.to_string()),
                ..Default::default()
            },
        }
    }

    /// URL koneksi SeaORM sesuai driver.
    pub fn sea_url(&self, data_dir: &Path) -> Result<String, String> {
        match self.database.driver.as_str() {
            "sqlite" => {
                let file = match self.database.path.as_deref() {
                    Some(p) => data_dir.join(p),
                    None => data_dir.join("peoplex.db"),
                };
                Ok(format!("sqlite://{}?mode=rwc", file.display()))
            }
            "postgres" | "postgresql" => {
                let host = self
                    .database
                    .host
                    .clone()
                    .unwrap_or("localhost".to_string());
                let port = self.database.port.unwrap_or(5432);
                let user = self.database.user.clone().unwrap_or("peoplex".to_string());
                let name = self
                    .database
                    .database
                    .clone()
                    .unwrap_or("peoplex".to_string());
                let pass = self
                    .database
                    .password_env
                    .as_deref()
                    .and_then(|k| std::env::var(k).ok())
                    .map(|p| format!(":{p}"))
                    .unwrap_or_default();
                Ok(format!("postgres://{user}{pass}@{host}:{port}/{name}"))
            }
            "mysql" => {
                let host = self
                    .database
                    .host
                    .clone()
                    .unwrap_or("localhost".to_string());
                let port = self.database.port.unwrap_or(3306);
                let user = self.database.user.clone().unwrap_or("peoplex".to_string());
                let name = self
                    .database
                    .database
                    .clone()
                    .unwrap_or("peoplex".to_string());
                let pass = self
                    .database
                    .password_env
                    .as_deref()
                    .and_then(|k| std::env::var(k).ok())
                    .map(|p| format!(":{p}"))
                    .unwrap_or_default();
                Ok(format!("mysql://{user}{pass}@{host}:{port}/{name}"))
            }
            other => Err(format!("driver database tidak dikenal: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_tertulis_dan_url_sqlite() {
        let dir = tempfile::tempdir().expect("dir");
        let cfg = load(dir.path()).expect("load");
        assert_eq!(cfg.database.driver, "sqlite");
        assert!(dir.path().join("peoplex.config.toml").exists());
        let url = cfg.sea_url(dir.path()).expect("url");
        assert!(url.starts_with("sqlite://"));
        assert!(url.ends_with("peoplex.db?mode=rwc"));
    }

    #[test]
    fn driver_asing_ditolak() {
        let mut cfg = AppConfig::default();
        cfg.database.driver = "oracle".to_string();
        assert!(cfg.sea_url(Path::new("/tmp")).is_err());
    }
}
