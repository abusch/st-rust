use anyhow::Result;
use tokio::net::UdpSocket;

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

    let data = std::fs::read("/home/abusch/.config/syncthing/cert.pem")?;
    let der_data = pem::parse(&data)?;
    println!("Parsed PEM file. Tag = {}", der_data.tag);
    let device_id = deviceid::DeviceId::from_der_cert(&der_data.contents);

    println!("DeviceId = {}", device_id);

    test_udp().await
}

async fn test_udp() -> Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:21027").await?;

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
            let packet = ::protobuf::parse_from_bytes::<Announce>(&buf[4..len]).unwrap();
            println!("{:?}", packet);
        } else {
            println!("Discarding packet");
        }
    }
}
