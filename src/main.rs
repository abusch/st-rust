use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber;

mod connections;
mod protocol;
mod discover;
mod protos;
mod tls;

use connections::tcp::tcp_listener;
use discover::local::{local_announce, local_udp_listener};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let certs = tls::load_certs("cert.pem")?;
    info!("Loaded {} certificates", certs.len());
    let mut keys = tls::load_keys("key.pem")?;
    info!("Loaded {} keys", keys.len());
    let device_id = protocol::DeviceId::from_der_cert(certs[0].0.as_slice());

    info!("DeviceId = {}", device_id);

    let config = tls::tls_config(certs, keys.remove(0))?;

    let handle = tokio::spawn(async move { local_announce(device_id).await.unwrap() });
    let handle2 = tokio::spawn(async move { local_udp_listener().await.unwrap() });
    let handle3 = tokio::spawn(async move { tcp_listener(config).await.unwrap() });

    match tokio::try_join!(handle, handle2, handle3) {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("Error: {}", err);
            Ok(())
        }
    }
}
