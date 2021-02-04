use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use protobuf::Message;
use rustls::{ServerConfig, Session};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tracing::{error, info, warn};

use crate::{
    protocol::{DeviceId, MAGIC},
    protos::Hello,
};

pub async fn tcp_listener(config: ServerConfig) -> Result<()> {
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let socket = TcpListener::bind("0.0.0.0:22000").await?;
    info!("Listening for TCP requests on port 22000");
    loop {
        match socket.accept().await {
            Ok((stream, addr)) => {
                let stream = acceptor.accept(stream).await?;
                tokio::spawn(async move {
                    match process_tcp_request(stream, addr).await {
                        Ok(_) => info!("TCP connection successfully handled"),
                        Err(e) => warn!("Failed to handle TCP connection: {}", e),
                    }
                });
            }
            Err(e) => error!("Error while accepting TCP connection: {}", e),
        }
    }
}

async fn process_tcp_request(
    mut stream: TlsStream<TcpStream>,
    peer_addr: SocketAddr,
) -> Result<()> {
    info!("Got a TCP connection from {}", peer_addr);
    let (_inner, session) = stream.get_ref();
    if let Some(alpn) = session.get_alpn_protocol() {
        let protocol = String::from_utf8_lossy(alpn);
        info!("Negotiated protocol (ALPN): {}", protocol);
    }
    if let Some(proto_version) = session.get_protocol_version() {
        info!("Protocol version = {:?}", proto_version);
    }
    if let Some(certs) = session.get_peer_certificates() {
        info!("Got {} certificates from the peer", certs.len());
        if !certs.is_empty() {
            let peer_id = DeviceId::from_der_cert(&certs[0].0);
            info!("Peer device id = {}", peer_id);
        }
    }
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
    } else {
        warn!("Invalid magic number in header: {:?}", header);
    }

    Ok(())
}
