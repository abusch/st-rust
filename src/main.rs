use anyhow::Result;
use tokio::net::UdpSocket;

mod protos;

use protos::Announce;

const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();

#[tokio::main]
async fn main() -> Result<()> {
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
