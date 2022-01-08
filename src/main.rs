use std::{time::Duration, sync::Arc};

use anyhow::Result;
use tokio::{signal::ctrl_c, time::timeout};
use tracing::{error, info};

mod config;
mod connections;
mod discover;
mod protocol;
mod protos;
mod tls;
mod model;

use connections::connection_service;
use discover::local;

use crate::model::Model;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    // console_subscriber::init();

    // TODO do something with it now
    let config = config::load_config()?;

    let certs = tls::load_certs("cert.pem")?;
    info!("Loaded {} certificates", certs.len());
    let mut keys = tls::load_keys("key.pem")?;
    info!("Loaded {} keys", keys.len());

    let device_id = protocol::DeviceId::from_der_cert(certs[0].0.as_slice());
    info!("DeviceId = {}", device_id);

    let model = Arc::new(Model::new(device_id, config));

    let tls_config = tls::tls_config(certs, keys.remove(0))?;

    // Every async "service" running in the background holds a clone of the Receiver side of this.
    // When it's time to shutdown, we just publish `true` using the sender side.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::select! {
        res = local::local_discovery(device_id, shutdown_rx.clone()) => {
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
        res = connection_service(model.clone(), tls_config, shutdown_rx.clone()) => {
            if let Err(err) = res {
                error!(cause = %err, "Connection service failed");
            }
        }
        _ = ctrl_c() => {
            info!("ctrl-c received!");
        }
    }

    info!("Notifying all listeners and connections to shutdown...");
    // Drop our copy of the receiver otherwise we'll hang forever waiting for it to close.
    drop(shutdown_rx);
    // Notify everyone that it's time to shutdown.
    shutdown_tx.send(true)?;
    // Wait for everyone to shutdown (i.e. for every Receiver to be dropped), but force quit after
    // 10 seconds.
    timeout(Duration::from_secs(10), shutdown_tx.closed()).await?;
    info!("Bye!");

    Ok(())
}
