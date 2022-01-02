use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};

use anyhow::Result;
use rustls::ServerConfig;
use tokio::select;
use tokio::sync::watch;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::Sender,
    time,
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tracing::{debug, error, info, warn};

use super::InternalConn;

pub async fn tcp_listener(conns: Sender<InternalConn>, tls_config: ServerConfig, shutdown_rx: watch::Receiver<bool>) -> Result<()> {
    let socket = TcpListener::bind("0.0.0.0:22000").await?;
    let mut listener = Listener::new(socket, conns, tls_config, shutdown_rx);

    listener.run().await
}

struct Listener {
    listener: TcpListener,
    conn_tx: Sender<InternalConn>,
    config: Arc<ServerConfig>,
    shutdown_rx: watch::Receiver<bool>,
}

impl Listener {
    pub fn new(listener: TcpListener, conn_tx: Sender<InternalConn>, config: ServerConfig, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self {
            listener,
            conn_tx,
            config: Arc::new(config),
            shutdown_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let acceptor = TlsAcceptor::from(self.config.clone());
        info!("Listening for TCP requests on port 22000");
        let mut shutdown_rx = self.shutdown_rx.clone();
        loop {
            select! {
            res = self.accept() => match res {
                Ok((stream, addr)) => {
                    debug!("TCP connection accepted");
                    match acceptor.accept(stream).await {
                        Ok(stream) => {
                            debug!("TLS connection accepted");
                            match self.process_tcp_request(stream, addr).await {
                                Ok(_) => info!("TCP connection successfully handled"),
                                Err(e) => warn!(cause = %e, "Failed to handle TCP connection"),
                            }
                        }
                        Err(e) => {
                            error!(cause = %e, "Error while accepting TLS connection");
                        }
                    }
                }
                Err(e) => error!(cause = %e, "Error while accepting TCP connection"),
            },
            _ = shutdown_rx.changed() => {
                info!("Shutting down TCP listener");
                return Ok(());
            }
            }
        }
    }

    async fn process_tcp_request(
        &self,
        stream: TlsStream<TcpStream>,
        peer_addr: SocketAddr,
    ) -> Result<()> {
        info!("Got a TCP connection from {}", peer_addr);

        let (conn, _session) = stream.get_ref();
        // Set TCP options
        conn.set_nodelay(false)?;
        conn.set_linger(None)?;
        // TODO keep-alive (doesn't seem to be supported in stdlib yet...)

        let internal_conn = InternalConn::new(stream);
        self.conn_tx.send(internal_conn).await?;

        Ok(())
    }

    /// Accept an inbound connection.
    ///
    /// Errors are handled by backing off and retrying. An exponential backoff
    /// strategy is used. After the first failure, the task waits for 1 second.
    /// After the second failure, the task waits for 2 seconds. Each subsequent
    /// failure doubles the wait time. If accepting fails on the 6th try after
    /// waiting for 64 seconds, then this function returns with an error.
    async fn accept(&mut self) -> Result<(TcpStream, SocketAddr)> {
        let mut backoff = 1;

        // Try to accept a few times
        loop {
            // Perform the accept operation. If a socket is successfully
            // accepted, return it. Otherwise, save the error.
            match self.listener.accept().await {
                Ok((socket, addr)) => return Ok((socket, addr)),
                Err(err) => {
                    if backoff > 64 {
                        // Accept has failed too many times. Return the error.
                        return Err(err.into());
                    }
                }
            }

            // Pause execution until the back off period elapses.
            debug!("Error while accepting. Sleeping for {} seconds.", backoff);

            time::sleep(Duration::from_secs(backoff)).await;

            // Double the back off
            backoff *= 2;
        }
    }
}
