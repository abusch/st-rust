#![allow(dead_code)]
use crate::{config::Configuration, protocol::DeviceId, protos::ClusterConfig};
use tracing::debug;

pub struct Model {
    id: DeviceId,
    cfg: Configuration,
    client_name: String,
    client_version: String,
}

impl Model {
    pub fn new(device_id: DeviceId, cfg: Configuration) -> Self {
        Self { id: device_id, cfg,
        client_name: "st-rust".to_owned(),
        client_version: "0.1".to_owned()}
    }

    fn cluster_config(&self, device_id: DeviceId, _cm: ClusterConfig) {
        debug!(?device_id, "Handling ClusterConfig");
    }
}
