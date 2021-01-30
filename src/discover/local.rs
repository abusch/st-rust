use std::time::Duration;

use anyhow::Result;
use protobuf::{Message, RepeatedField};
use tokio::{net::UdpSocket, time::sleep};
use tracing::{error, info, warn};

use crate::{
    protocol::{DeviceId, MAGIC},
    protos::Announce,
};

pub async fn local_announce(id: DeviceId) -> Result<()> {
    let instance_id: i64 = fastrand::i64(..);
    let duration = Duration::from_secs(30);
    let mut buf = Vec::with_capacity(1024);
    info!("Creating UDP socket");
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    info!("UDP socket created");
    info!("Setting broadcast mode");
    sock.set_broadcast(true)?;
    info!("Connecting to broadcast address");
    sock.connect("255.255.255.255:21027").await?;
    info!("Ready to send announcement packets");
    loop {
        buf.clear();
        std::io::Write::write(&mut buf, MAGIC)?;
        sleep(duration).await;
        info!("Sending announcement packet");
        let addresses = vec!["tcp://10.0.1.214:22000".to_string()];
        let mut pkt = Announce::new();
        pkt.set_id(id.to_vec());
        pkt.set_instance_id(instance_id);
        pkt.set_addresses(RepeatedField::from_vec(addresses));
        pkt.write_to_vec(&mut buf)?;
        match sock.send(&buf).await {
            Ok(n) => info!("Sent {} bytes", n),
            Err(e) => warn!("Failed to send packet: {}", e),
        }
    }
}

pub async fn local_udp_listener() -> Result<()> {
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
                // println!("{:?}", packet);
                } else {
                    info!("Discarding unknown packet");
                }
            }
            Err(e) => error!("Failed to read from UDP socket: {}", e),
        }
    }
}
