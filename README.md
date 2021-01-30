# Toy implementation Syncthing in Rust

This is a project purely for educational purposes. The real Syncthing can be found [here](https://syncthing.net). The goal is to implement enough of the protocol to connect to a peer and sync some files, and learn stuff along the way.

So far, it can:
- send and receive `Announce` packets
- Listen for tcp connections:
  - execute TLS handshake
  - present our own certificate
  - retrieve the peer certificate and compute its DeviceID
  - negotiate the bep/1.0 protocol
  - exchange `Hello` message
