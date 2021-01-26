use std::{fs::File, io::BufReader, path::Path, sync::Arc, time::Duration};
use std::{io::Write, net::SocketAddr};

use anyhow::{anyhow, Result};
use deviceid::DeviceId;
use protobuf::{Message, RepeatedField};
use rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig, internal::pemfile::{certs, pkcs8_private_keys}};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::sleep;

mod deviceid;
mod luhn;
mod protos;

use protos::{Announce, Hello};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();

fn load_certs<P: AsRef<Path>>(path: P) -> Result<Vec<Certificate>> {
    let path = path.as_ref();
    certs(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("Failed to load certificate file: {}", path.display()))
}

fn load_keys<P: AsRef<Path>>(path: P) -> Result<Vec<PrivateKey>> {
    let path = path.as_ref();
    pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("Failed to load key file: {}", path.display()))
}


#[tokio::main]
async fn main() -> Result<()> {
    let certs = load_certs("cert.pem")?;
    println!("Loaded {} certificates", certs.len());
    let mut keys = load_keys("key.pem")?;
    println!("Loaded {} keys", keys.len());
    let device_id = deviceid::DeviceId::from_der_cert(certs[0].0.as_slice());

    println!("DeviceId = {}", device_id);

    let mut config = ServerConfig::new(NoClientAuth::new());
    config.set_single_cert(certs, keys.remove(0))?;

    let handle = tokio::spawn(async move { local_announce(device_id).await.unwrap() });
    let handle2 = tokio::spawn(async move { test_udp().await.unwrap() });
    let handle3 = tokio::spawn(async move { tcp_listener(config).await.unwrap() });

    match tokio::try_join!(handle, handle2, handle3) {
        Ok(_) => Ok(()),
        Err(err) => {
            eprintln!("Error: {}", err);
            Ok(())
        }
    }
}

async fn tcp_listener(config: ServerConfig) -> Result<()> {
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let socket = TcpListener::bind("0.0.0.0:22000").await?;
    println!("Listening for TCP requests on port 22000");
    loop {
        match socket.accept().await {
            Ok((stream, addr)) => {
                let stream = acceptor.accept(stream).await?;
                tokio::spawn(async move {
                    match process_tcp_request(stream, addr).await {
                        Ok(_) => println!("TCP connection successfully handled"),
                        Err(e) => eprintln!("Failed to handle TCP connection: {}", e),
                    }
                });
            }
            Err(e) => eprintln!("Error while accepting TCP connection: {}", e),
        }
    }
}

async fn process_tcp_request(mut stream: TlsStream<TcpStream>, peer_addr: SocketAddr) -> Result<()> {
    println!("Got a TCP connection from {}", peer_addr);
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    if &header[..] == MAGIC {
        let len = stream.read_u16().await?;
        if len > 32767 {
            anyhow!("Hello message too big: {}", len);
        }
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;
        let hello = Hello::parse_from_bytes(&buf)?;
        println!("Got a Hello packet! {:?}", hello);
    } else {
        println!("Invalid magic number in header: {:?}", header);
    }

    Ok(())
}

async fn local_announce(id: DeviceId) -> Result<()> {
    let instance_id: i64 = fastrand::i64(..);
    let duration = Duration::from_secs(30);
    let mut buf = Vec::with_capacity(1024);
    println!("Creating UDP socket");
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    println!("UDP socket created");
    println!("Setting broadcast mode");
    sock.set_broadcast(true)?;
    println!("Connecting to broadcast address");
    sock.connect("255.255.255.255:21027").await?;
    println!("Ready to send announcement packets");
    loop {
        buf.clear();
        buf.write(MAGIC)?;
        sleep(duration).await;
        println!("Sending announcement packet");
        let addresses = vec!["tcp://10.0.1.214:22000".to_string()];
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

async fn test_udp() -> Result<()> {
    let mut buf = [0u8; 1024];
    let sock = UdpSocket::bind("0.0.0.0:21027").await?;
    println!("Listening to UDP packets");
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                println!("{:?} bytes received from {:?}", len, addr);
                if len == 0 {
                    println!("Dropping empty packet");
                    continue;
                }
                if &buf[0..4] == MAGIC {
                    let packet = Announce::parse_from_bytes(&buf[4..len]).unwrap();
                    let device_id = DeviceId::new(packet.get_id());
                    println!("Go an announcement packet from {}", device_id);
                // println!("{:?}", packet);
                } else {
                    println!("Discarding packet");
                }
            }
            Err(e) => eprintln!("Failed to read from UDP socket: {}", e),
        }
    }
}
