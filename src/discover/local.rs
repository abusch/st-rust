use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use bytes::{BufMut, BytesMut};
use prost::Message;
use tokio::{net::UdpSocket, sync::Mutex, time::sleep};
use tracing::{debug, error, info, instrument, warn};
use url::Url;

use crate::{
    protocol::{DeviceId, MAGIC},
    protos::Announce,
};

/// Start the local discovery service. This will periodically announce our
/// presence on then network, and listens for other peers on the network.
pub async fn local_discovery(device_id: DeviceId) -> Result<()> {
    info!("Creating UDP socket");
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    info!("UDP socket created");
    info!("Setting broadcast mode");
    sock.set_broadcast(true)?;
    info!("Connecting to broadcast address");
    sock.connect("255.255.255.255:21027").await?;

    let beacon = LocalBeacon::new(device_id, sock);
    let listener = LocalListener::new(device_id);

    tokio::select! {
        res = beacon.announce() => {
            if let Err(err) = res {
                error!(cause = %err, "Beacon failed");
            }
        }
        res = listener.local_udp_listener() => {
            if let Err(err) = res {
                error!(cause = %err, "Listener failed");
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub struct CacheEntry {
    addresses: Vec<String>,
    when: Instant,
    // valid_until: Instant,
    instance_id: i64,
}

struct LocalListener {
    #[allow(dead_code)]
    my_id: DeviceId,
    cache: Mutex<HashMap<DeviceId, CacheEntry>>,
}

impl LocalListener {
    const CACHE_LIFE_TIME: Duration = Duration::from_secs(90);

    fn new(id: DeviceId) -> Self {
        Self {
            my_id: id,
            cache: Mutex::new(HashMap::new()),
        }
    }

    #[instrument(skip(self))]
    pub async fn local_udp_listener(&self) -> Result<()> {
        let mut buf = [0u8; 1024];
        let sock = UdpSocket::bind("0.0.0.0:21027").await?;
        info!("Listening to UDP packets");
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    debug!("{:?} bytes received from {:?}", len, addr);
                    if len == 0 {
                        debug!("Dropping empty packet");
                        continue;
                    }
                    if &buf[0..4] == MAGIC {
                        let announce = Announce::decode(&buf[4..len])?;
                        let _is_new_device = self.register_device(announce, addr).await;
                        // TODO send a broadcast message to announce ourselves if is_new_device
                    } else {
                        info!("Discarding unknown packet");
                    }
                }
                Err(e) => error!("Failed to read from UDP socket: {}", e),
            }
        }
    }

    async fn register_device(&self, announce: Announce, src_addr: SocketAddr) -> bool {
        let device_id = DeviceId::new(&announce.id[..]);
        info!(
            "Got an announcement packet from {} for {}",
            src_addr, device_id
        );

        let mut valid_addresses = vec![];
        for addr in announce.addresses.iter() {
            match self.validate_address(addr, &src_addr).await {
                Ok(valid_address) => valid_addresses.push(valid_address),
                Err(e) => debug!("Ignoring invalid address {}: {}", addr, e),
            }
        }

        let mut guard = self.cache.lock().await;
        // Adds the device to our cache
        let old_value = guard.insert(
            device_id,
            CacheEntry {
                addresses: valid_addresses,
                when: Instant::now(),
                instance_id: announce.instance_id,
            },
        );

        old_value
            .map(|e| {
                Instant::now().duration_since(e.when) < Self::CACHE_LIFE_TIME
                    && e.instance_id == announce.instance_id
            })
            .unwrap_or(false)
    }

    async fn validate_address(&self, addr: &str, src: &SocketAddr) -> Result<String> {
        // Make sure it's a parsable URL
        let mut url = Url::parse(addr)?;
        if url.has_host() {
            // Try to resolve the host. Need to do that on a blocking thread as `Url::socket_addrs()` is not async.
            let url2 = url.clone();
            let mut _addrs =
                tokio::task::spawn_blocking(move || url2.socket_addrs(|| None)).await?;
            // let _tcp_addr = self.resolve_host(host).await?;
            debug!("discover: Accepted address {} verbatim", url.to_string());
        } else {
            // If there is no host, use the src IP address instead. But first, make sure its IP version matches what was requested.
            if (url.scheme() == "tcp4" && src.is_ipv6())
                || (url.scheme() == "tcp6" && src.is_ipv4())
            {
                anyhow!("Source address IP version doesn't match requested type");
            } else {
                // join the IP of the src address with the port of the current address
                url.set_host(Some(&src.ip().to_string()))?;
            }
        }

        Ok(url.to_string())
    }
}

struct LocalBeacon {
    my_id: DeviceId,
    instance_id: i64,
    sock: UdpSocket,
    sleep_duration: Duration,
}

impl LocalBeacon {
    pub fn new(device_id: DeviceId, udp_sock: UdpSocket) -> Self {
        Self {
            my_id: device_id,
            instance_id: fastrand::i64(..),
            sock: udp_sock,
            sleep_duration: Duration::from_secs(30),
        }
    }

    pub async fn announce(&self) -> Result<()> {
        let mut buf = BytesMut::with_capacity(1024);
        info!("Ready to send announcement packets");
        loop {
            buf.clear();
            buf.put_slice(MAGIC);
            sleep(self.sleep_duration).await;
            info!("Sending announcement packet");
            let announce = self.announce_msg();
            announce.encode(&mut buf)?;
            match self.sock.send(&buf).await {
                Ok(n) => info!("Sent {} bytes", n),
                Err(e) => warn!("Failed to send packet: {}", e),
            }
        }
    }

    fn announce_msg(&self) -> Announce {
        let addresses = vec!["tcp://10.0.1.214:22000".to_string()];
        let mut pkt = Announce::default();
        pkt.id = self.my_id.to_bytes().to_vec();
        pkt.instance_id = self.instance_id;
        pkt.addresses = addresses;

        pkt
    }
}
