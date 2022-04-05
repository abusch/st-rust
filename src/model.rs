use std::{collections::HashMap, str::FromStr};

use crate::{
    config::Configuration,
    protocol::{connection::ConnectionHandle, DeviceId, TypedMessage},
    protos::{ClusterConfig, Device, Index},
};
use anyhow::Result;
use tokio::sync::Mutex;
use tracing::debug;

pub struct Model {
    id: DeviceId,
    cfg: Configuration,
    client_name: String,
    client_version: String,
    conns: Mutex<HashMap<DeviceId, ConnectionHandle>>,
}

impl Model {
    pub fn new(device_id: DeviceId, cfg: Configuration) -> Self {
        Self {
            id: device_id,
            cfg,
            client_name: "st-rust".to_owned(),
            client_version: "0.1".to_owned(),
            conns: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_connection(&self, conn: ConnectionHandle) -> Result<()> {
        let device_id = conn.id();
        // TODO check if the device exists in the configuration

        let mut conns = self.conns.lock().await;
        conns.insert(device_id, conn);
        let c = conns
            .get_mut(&device_id)
            .expect("We just added the connection to the model but we can't get it back?!?");

        debug!("Starting connection...");
        c.start().await;
        debug!("Sending ClusterConfig");
        let config = self.generate_cluster_config();
        c.send(config).await?;
        debug!("Sending Index");
        let index = self.generate_index();
        c.send(index).await?;

        Ok(())
    }

    fn generate_cluster_config(&self) -> TypedMessage {
        // TODO generate proper config
        let mut config = ClusterConfig::default();
        for cfg_folder in self.cfg.folders.iter() {
            let mut folder = crate::protos::Folder {
                id: cfg_folder.id.clone(),
                label: cfg_folder.label.clone(),
                ..crate::protos::Folder::default()
            };
            for d in cfg_folder.device.iter() {
                let id = d.id.clone();
                let device_id = DeviceId::from_str(&id).unwrap();
                folder.devices.push(Device {
                    id: device_id.bytes().to_vec(),
                    ..Device::default()
                });
            }
            config.folders.push(folder);
        }
        TypedMessage::ClusterConfig(config)
    }

    fn generate_index(&self) -> TypedMessage {
        let mut index = Index::default();
        if let Some(cfg_folder) = self.cfg.folders.first() {
            index.folder = cfg_folder.id.clone();
        }
        TypedMessage::Index(index)
    }

    pub fn cluster_config(&self, device_id: DeviceId, _cm: ClusterConfig) {
        debug!(%device_id, "Handling ClusterConfig");
    }

    pub fn index(&self, device_id: DeviceId, _index: Index) {
        debug!(%device_id, "Handling Index message");
    }
}
