use std::collections::HashMap;

use anyhow::{anyhow, Result};
use protobuf::Message;
use rustls::{ServerConfig, Session};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
    sync::mpsc::Receiver,
};
use tokio_rustls::server::TlsStream;
use tracing::{debug, error, info, warn};

use crate::{
    protocol::{connection::ConnectionHandle, DeviceId, MAGIC},
    protos::Hello,
};

pub mod tcp;

pub async fn connection_service(my_id: DeviceId, tls_config: ServerConfig) -> Result<()> {
    let (conns_tx, conns_rx) = tokio::sync::mpsc::channel(10);
    let mut service = Service::new(my_id, conns_rx);

    select! {
        res = service.handle() => {
            if let Err(err) = res {
                error!(cause = %err, "failed to handle connections");
            }
        }
        res = tcp::tcp_listener(conns_tx, tls_config) => {
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
    }

    Ok(())
}

/// Service to manage connections
pub struct Service {
    my_id: DeviceId,
    conns: Receiver<InternalConn>,
    connections: HashMap<DeviceId, ConnectionHandle>,
}

impl Service {
    pub fn new(my_id: DeviceId, conns: Receiver<InternalConn>) -> Self {
        Self {
            my_id,
            conns,
            connections: HashMap::new(),
        }
    }

    pub async fn handle(&mut self) -> Result<()> {
        loop {
            if let Some(internal_conn) = self.conns.recv().await {
                let mut stream = internal_conn.conn;
                let (_conn, session) = stream.get_ref();
                let remote_id = match self.validate_connection(session) {
                    Ok(device_id) => {
                        info!(remote_id = %device_id, "Connection is valid");
                        device_id
                    }
                    Err(e) => {
                        warn!("Closing invalid connection: {}", e);
                        drop(stream);
                        continue;
                    }
                };

                let mut header = [0u8; 4];
                stream.read_exact(&mut header).await?;
                if &header[..] == MAGIC {
                    info!("Found magic header");
                    let len = stream.read_u16().await?;
                    if len > 32767 {
                        anyhow!("Hello message too big: {}", len);
                    }
                    let mut buf = vec![0u8; len as usize];
                    stream.read_exact(&mut buf).await?;
                    info!("Read {} bytes", len);
                    let hello = Hello::parse_from_bytes(&buf)?;
                    info!("Got a Hello packet! {:?}", hello);

                    // Send our own Hello back
                    let mut reply = Hello::default();
                    reply.set_device_name("calculon".into());
                    reply.set_client_name("st-rust".into());
                    reply.set_client_version("0.1".into());

                    buf.clear();
                    std::io::Write::write(&mut buf, MAGIC)?;
                    let mut reply_bytes = reply.write_to_bytes()?;
                    let len = reply_bytes.len() as u16;
                    std::io::Write::write(&mut buf, &len.to_be_bytes())?;
                    buf.append(&mut reply_bytes);
                    info!("Sending Hello packet back: {:?}", reply);
                    stream.write_all(&buf).await?;
                    info!("Sent {} bytes back", buf.len());

                    debug!("Dispatching task to handle connection");
                    let mut connection_handle = ConnectionHandle::new(stream);
                    debug!("Sending ClusterConfig");
                    connection_handle.config_cluster().await?;
                    self.connections.insert(remote_id, connection_handle);
                } else {
                    warn!("Invalid magic number in header: {:?}", header);
                }
            } else {
                info!("Shutting down...");
            }
        }
    }

    fn validate_connection(&self, session: &dyn Session) -> Result<DeviceId> {
        if let Some(alpn) = session.get_alpn_protocol() {
            let protocol = String::from_utf8_lossy(alpn);
            debug!(%protocol);
        }
        if let Some(tls_version) = session.get_protocol_version() {
            debug!(?tls_version);
        }
        let certs = session
            .get_peer_certificates()
            .ok_or_else(|| anyhow!("No remote certificates found"))?;
        if certs.len() != 1 {
            anyhow!("Wrong number of certificates: {}", certs.len());
        }
        let peer_id = DeviceId::from_der_cert(&certs[0].0);

        Ok(peer_id)
    }
}

#[derive(Debug)]
pub struct InternalConn {
    conn: TlsStream<TcpStream>,
    // TODO connection type
}

impl InternalConn {
    pub fn new(conn: TlsStream<TcpStream>) -> Self {
        Self { conn }
    }
}
