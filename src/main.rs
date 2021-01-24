use std::{io::Write, sync::Arc};
use std::time::Duration;

use anyhow::Result;
use deviceid::DeviceId;
use protobuf::{Message, RepeatedField};
use tokio::net::UdpSocket;
use tokio::time::sleep;

mod deviceid;
mod luhn;
mod protos;

use protos::Announce;

const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();

#[tokio::main]
async fn main() -> Result<()> {
    // let mut rng = SystemRandom::new();
    // let pkcs8 = EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P384_SHA384_ASN1_SIGNING, &mut rng).unwrap();
    // let key_pair = EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P384_SHA384_ASN1_SIGNING, pkcs8).unwrap();
    // key_pair.

    let data = std::fs::read("cert.pem")?;
    let der_data = pem::parse(&data)?;
    println!("Parsed PEM file. Tag = {}", der_data.tag);
    let device_id = deviceid::DeviceId::from_der_cert(&der_data.contents);

    println!("DeviceId = {}", device_id);

    let sock = Arc::new(UdpSocket::bind("0.0.0.0:21027").await?);
    sock.set_broadcast(true)?;
    let handle = tokio::spawn(local_announce(Arc::clone(&sock), device_id));
    let handle2 = tokio::spawn(test_udp(Arc::clone(&sock)));

    match tokio::try_join!(handle, handle2) {
        Ok(_) => Ok(()),
        Err(err) => {
            eprintln!("Error: {}", err);
            Ok(())
        }
    }
}

async fn local_announce(sock: Arc<UdpSocket>, id: DeviceId) -> Result<()> {
    let instance_id: i64 = fastrand::i64(..);
    let duration = Duration::from_secs(30);
    let mut buf = Vec::with_capacity(1024);
    sock.connect("255.255.255.255:21027").await?;
    loop {
        buf.clear();
        buf.write(MAGIC)?;
        sleep(duration).await;
        println!("Sending announcement packet");
        let addresses = vec!["tcp://10.0.1.214".to_string()];
        let mut pkt = Announce::new();
        pkt.set_id(id.to_vec());
        pkt.set_instance_id(instance_id);
        pkt.set_addresses(RepeatedField::from_vec(addresses));
        pkt.write_to_vec(&mut buf)?;
        match sock.send(&buf).await {
            Ok(n) => println!("Sent {} bytes", n),
            Err(e) => eprintln!("Failed to send packet: {}", e),
        }
    }
}

async fn test_udp(sock: Arc<UdpSocket>) -> Result<()> {
    let mut buf = [0u8; 1024];
    println!("Listening to UDP packets");
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;
        println!("{:?} bytes received from {:?}", len, addr);
        if len == 0 {
            println!("Dropping empty packet");
            continue;
        }
        if &buf[0..4] == MAGIC {
            println!("Got an announcement packet!");
            let packet = Announce::parse_from_bytes(&buf[4..len]).unwrap();
            println!("{:?}", packet);
        } else {
            println!("Discarding packet");
        }
    }
}
