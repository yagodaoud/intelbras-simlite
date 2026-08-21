use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::{Camera, Channel, Device, DomainError, Layout, Mosaic};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub device: DeviceFile,
    pub cameras: Vec<CameraFile>,
    pub mosaic: MosaicFile,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceFile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub rtsp_port: u16,
    pub http_port: u16,
    pub username: String,
    pub channel_count: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraFile {
    pub channel: u8,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MosaicFile {
    pub layout: Layout,
    pub selected: Vec<u8>,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
}

fn default_sidebar_width() -> u32 {
    168
}

impl AppConfig {
    pub fn default_six(host: &str, username: &str) -> Result<Self, DomainError> {
        let device = Device::new("dvr-casa", "DVR", host, 554, 80, username, 6)?;
        let cameras = (1..=6)
            .map(|n| CameraFile {
                channel: n,
                name: format!("Canal {n}"),
            })
            .collect();
        Ok(Self {
            device: DeviceFile {
                id: device.id,
                name: device.name,
                host: device.host,
                rtsp_port: device.rtsp_port,
                http_port: device.http_port,
                username: device.username,
                channel_count: device.channel_count,
            },
            cameras,
            mosaic: MosaicFile {
                layout: Layout::Four,
                selected: vec![1, 2, 3, 4],
                sidebar_width: 168,
            },
        })
    }

    pub fn device(&self) -> Result<Device, DomainError> {
        Device::new(
            &self.device.id,
            &self.device.name,
            &self.device.host,
            self.device.rtsp_port,
            self.device.http_port,
            &self.device.username,
            self.device.channel_count,
        )
    }

    pub fn mosaic(&self) -> Result<Mosaic, DomainError> {
        let cameras = self
            .cameras
            .iter()
            .map(|c| {
                Ok(Camera::new(Channel::new(c.channel)?, c.name.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut mosaic = Mosaic::new(self.mosaic.layout, cameras);
        let selected: Vec<Channel> = self
            .mosaic
            .selected
            .iter()
            .copied()
            .filter_map(|n| Channel::new(n).ok())
            .collect();
        mosaic.select_only(&selected)?;
        Ok(mosaic)
    }

    pub fn sync_from_mosaic(&mut self, mosaic: &Mosaic) {
        self.mosaic.layout = mosaic.layout();
        self.mosaic.selected = mosaic.selected().iter().map(|c| c.get()).collect();
    }
}

pub fn config_path() -> Result<PathBuf, ConfigError> {
    let dirs = directories::ProjectDirs::from("br", "simlite", "SIM Lite")
        .ok_or(ConfigError::Io)?;
    Ok(dirs.config_dir().join("config.json"))
}

pub fn load(path: &Path) -> Result<AppConfig, ConfigError> {
    let raw = fs::read_to_string(path).map_err(|_| ConfigError::Io)?;
    if json_has_password_key(&raw) {
        return Err(ConfigError::PasswordInFile);
    }
    serde_json::from_str(&raw).map_err(|_| ConfigError::Corrupt)
}

pub fn save(path: &Path, cfg: &AppConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| ConfigError::Io)?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|_| ConfigError::Corrupt)?;
    if json_has_password_key(&json) {
        return Err(ConfigError::PasswordInFile);
    }
    fs::write(path, json).map_err(|_| ConfigError::Io)
}

fn json_has_password_key(raw: &str) -> bool {
    raw.to_ascii_lowercase().contains("\"password\":")
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("não foi possível ler/gravar a configuração")]
    Io,
    #[error("configuração inválida")]
    Corrupt,
    #[error("a configuração não pode guardar senha")]
    PasswordInFile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_config_never_has_password_field() {
        let cfg = AppConfig::default_six("192.168.0.10", "viewer").unwrap();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(!json.to_ascii_lowercase().contains("password"));
        assert!(!json.contains("hunter"));
        assert!(json.contains("192.168.0.10"));
        assert!(json.contains("viewer"));
    }

    #[test]
    fn refuses_to_load_file_that_contains_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"password":"hunter2"}"#).unwrap();
        assert_eq!(load(&path).unwrap_err(), ConfigError::PasswordInFile);
    }

    #[test]
    fn roundtrip_file_and_mosaic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let cfg = AppConfig::default_six("192.168.0.10", "viewer").unwrap();
        save(&path, &cfg).unwrap();
        let loaded = load(&path).unwrap();
        let mosaic = loaded.mosaic().unwrap();
        assert_eq!(mosaic.layout(), Layout::Four);
        assert_eq!(mosaic.selected().len(), 4);
        assert_eq!(mosaic.cameras().len(), 6);
    }
}
