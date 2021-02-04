use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Result;
use bytes::BufMut;
use protobuf::{Chars, Message};
use tokio::{net::UdpSocket, sync::Mutex, time::sleep};
use tracing::{error, info, warn};

use crate::{
    protocol::{DeviceId, MAGIC},
    protos::Announce,
};

pub async fn local_discovery(device_id: DeviceId) -> Result<()> {
    info!("Creating UDP socket");
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    info!("UDP socket created");
    info!("Setting broadcast mode");
    sock.set_broadcast(true)?;
    info!("Connecting to broadcast address");
    sock.connect("255.255.255.255:21027").await?;

    let beacon = LocalBeacon::new(device_id, sock);
    let listener = LocalListener::new();

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

pub struct CacheEntry {
    addresses: Vec<String>,
    when: Instant,
    valid_until: Instant,
    instance_id: i64,
}

struct LocalListener {
    cache: Mutex<HashMap<DeviceId, CacheEntry>>,
}

impl LocalListener {
    fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub async fn local_udp_listener(&self) -> Result<()> {
        let mut buf = [0u8; 1024];
        let sock = UdpSocket::bind("0.0.0.0:21027").await?;
        info!("Listening to UDP packets");
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    info!("{:?} bytes received from {:?}", len, addr);
                    if len == 0 {
                        info!("Dropping empty packet");
                        continue;
                    }
                    if &buf[0..4] == MAGIC {
                        let packet = Announce::parse_from_bytes(&buf[4..len]).unwrap();
                        let device_id = DeviceId::new(packet.get_id());
                        info!("Got an announcement packet from {}", device_id);
                    } else {
                        info!("Discarding unknown packet");
                    }
                }
                Err(e) => error!("Failed to read from UDP socket: {}", e),
            }
        }
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
        let mut buf = Vec::with_capacity(1024);
        info!("Ready to send announcement packets");
        loop {
            buf.clear();
            buf.put_slice(MAGIC);
            sleep(self.sleep_duration).await;
            info!("Sending announcement packet");
            let announce = self.announce_msg();
            announce.write_to_vec(&mut buf)?;
            match self.sock.send(&buf).await {
                Ok(n) => info!("Sent {} bytes", n),
                Err(e) => warn!("Failed to send packet: {}", e),
            }
        }
    }

    fn announce_msg(&self) -> Announce {
        let addresses = vec![Chars::from("tcp://10.0.1.214:22000")];
        let mut pkt = Announce::new();
        pkt.set_id(self.my_id.to_bytes());
        pkt.set_instance_id(self.instance_id);
        pkt.set_addresses(addresses);

        pkt
    }
}
