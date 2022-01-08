use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::protocol::DeviceId;

pub fn load_config() -> Result<Configuration> {
    let project_dirs =
        ProjectDirs::from("", "", "st-rust").ok_or_else(|| anyhow!("No $HOME directory found!"))?;
    let config_dir = project_dirs.config_dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir)?;
    }

    let mut config_file = std::path::PathBuf::from(config_dir);
    config_file.push("config.toml");

    let config_content = std::fs::read_to_string(&config_file).context(format!(
        "Unable to open configuration file: {}",
        config_file.display()
    ))?;
    let config = toml::from_str(&config_content)?;

    Ok(config)
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    pub folders: Vec<Folder>,
    pub devices: Vec<DeviceConfiguration>,
}

impl Configuration {
   pub fn device(&self, id: &DeviceId) -> Option<DeviceConfiguration> {
       self.devices.iter().find(|d| d.id == *id).cloned()
   }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfiguration {
    pub id: DeviceId,
    pub name: Option<String>,
    #[serde(default)]
    pub compression: Compression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    Metadata,
    Always,
    Never,
}

impl Default for Compression {
    fn default() -> Self {
        Self::Never
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    #[serde(default)]
    pub r#type: FolderSyncType,
    pub device: Vec<FolderDevice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FolderSyncType {
    SendReceive,
    SendOnly,
    ReceiveOnly,
}

impl Default for FolderSyncType {
    fn default() -> Self {
        Self::SendReceive
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderDevice {
    pub id: String,
    pub introduced_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_full() {
        let cfg = r#"
[[folders]]
id = "abcd-1234"
label = "Default"
path = "/home/abusch/Default"
[[folders.device]]
id =  "UYA4Y6A-RMU7JXN-RU4CXE4-22XPLPZ-67VCOXJ-PLGGPID-KC25D3A-BBREDQD"

[[devices]]
id = "UYA4Y6A-RMU7JXN-RU4CXE4-22XPLPZ-67VCOXJ-PLGGPID-KC25D3A-BBREDQD"

[[devices]]
id = "MFZWI3D-BONSGYC-YLTMRWG-C43ENR5-QXGZDMM-FZWI3DP-BONSGYY-LTMRWAD"
name="foo"
compression = "always"
"#;
        let data = toml::from_str::<Configuration>(cfg).unwrap();

        assert_eq!(1, data.folders.len());
        assert_eq!(1, data.folders[0].device.len());
        assert_eq!(2, data.devices.len());
    }

    #[test]
    fn missing_device_id() {
        let cfg = r#"
[[devices]]
name="foo"
compression = "always"
"#;
        let data = toml::from_str::<Configuration>(cfg);

        assert!(data.is_err())
    }
}
