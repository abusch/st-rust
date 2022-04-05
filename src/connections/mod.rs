use std::sync::Arc;

use anyhow::{anyhow, Result, bail};
use bytes::BufMut;
use prost::Message;
use rustls::{ServerConfig, ServerConnection};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
    sync::{mpsc, watch::Receiver},
};
use tokio_rustls::server::TlsStream;
use tracing::{debug, error, info, warn};

use crate::{
    protocol::{connection::ConnectionHandle, DeviceId, MessageExt, MAGIC},
    protos::Hello, model::Model,
};

pub mod tcp;

pub async fn connection_service(
    model: Arc<Model>,
    tls_config: ServerConfig,
    shutdown_rx: Receiver<bool>,
) -> Result<()> {
    let (conns_tx, conns_rx) = mpsc::channel(10);
    let mut service = Service::new(model, conns_rx, shutdown_rx.clone());

    select! {
        res = service.handle() => {
            if let Err(err) = res {
                error!(cause = %err, "failed to handle connections");
            }
        }
        res = tcp::tcp_listener(conns_tx, tls_config, shutdown_rx.clone()) => {
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
    }

    Ok(())
}

/// Service to manage connections
pub struct Service {
    model: Arc<Model>,
    // Receive connections that have been created by the various listeners (e.g. TcpListener)
    conn_rx: mpsc::Receiver<InternalConn>,
    shutdown_rx: Receiver<bool>,
}

impl Service {
    pub fn new(model: Arc<Model>, conns: mpsc::Receiver<InternalConn>, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            model,
            conn_rx: conns,
            shutdown_rx,
        }
    }

    pub async fn handle(&mut self) -> Result<()> {
        loop {
            select! {
            Some(internal_conn) = self.conn_rx.recv() => {
                let mut stream = internal_conn.conn;
                let (conn, session) = stream.get_ref();
                let peer_addr = conn.peer_addr()?;
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
                        bail!("Hello message too big: {}", len);
                    }
                    let mut buf = vec![0u8; len as usize];
                    stream.read_exact(&mut buf).await?;
                    info!("Read {} bytes", len);
                    let hello = Hello::decode(&buf[..])?;
                    info!("Got a Hello packet! {:?}", hello);

                    // Send our own Hello back
                    let reply = Hello {
                        device_name: "calculon".into(),
                        client_name: "st-rust".into(),
                        client_version: "0.1".into(),
                    };

                    buf.clear();
                    buf.put(MAGIC);
                    reply.write_len16_and_bytes(&mut buf)?;
                    debug!("Sending Hello packet back: {:?}", reply);
                    stream.write_all(&buf).await?;
                    debug!("Sent {} bytes back", buf.len());

                    debug!("Dispatching task to handle connection");
                    let connection_handle = ConnectionHandle::new(self.model.clone(), remote_id, peer_addr, stream, self.shutdown_rx.clone());
                    debug!("Adding connection to model");
                    self.model.add_connection(connection_handle).await?;
                } else {
                    warn!("Invalid magic number in header: {:?}", header);
                }
            },
            _ = self.shutdown_rx.changed() => {
                info!("Shutting down connection service");
            }
            }
        }
    }

    fn validate_connection(&self, session: &ServerConnection) -> Result<DeviceId> {
        if let Some(alpn) = session.alpn_protocol() {
            let protocol = String::from_utf8_lossy(alpn);
            debug!(%protocol);
        }
        if let Some(tls_version) = session.protocol_version() {
            debug!(?tls_version);
        }
        let certs = session
            .peer_certificates()
            .ok_or_else(|| anyhow!("No remote certificates found"))?;
        if certs.len() != 1 {
            bail!("Wrong number of certificates: {}", certs.len());
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
