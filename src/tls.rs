use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use anyhow::{anyhow, Result};
use rustls::{
    internal::pemfile::{certs, pkcs8_private_keys},
    Certificate, ClientCertVerifier, PrivateKey, ServerConfig,
};

/// Load the given certificate(s). Expects a PEM a file with DER-encoded X.509 certificate.
pub fn load_certs<P: AsRef<Path>>(path: P) -> Result<Vec<Certificate>> {
    let path = path.as_ref();
    certs(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("Failed to load certificate file: {}", path.display()))
}

/// Loads the given private key. Expects a PEM file with a PKCS8 key.
pub fn load_keys<P: AsRef<Path>>(path: P) -> Result<Vec<PrivateKey>> {
    let path = path.as_ref();
    pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("Failed to load key file: {}", path.display()))
}

/// Build configuration for accepting TLS connections.
///
/// The returned configuration will present the given certificate/key as part as
/// the TLS handshake, and will request the peer certificate but won't validate
/// it. It will also negotiate the "bep/1.0" protocol.
pub fn tls_config(certs: Vec<Certificate>, key: PrivateKey) -> Result<ServerConfig> {
    // TLS config
    let mut config = ServerConfig::new(Arc::new(RequestCertificate));
    // Load up our certificate and key to present during TLS handshake
    config.set_single_cert(certs, key)?;
    // Set up application level protocol negotation and indicate we want bep/1.0
    config.set_protocols(&[b"bep/1.0".to_vec()]);

    Ok(config)
}

/// A [ClientCertVerifier] that simply requests the peer certificate without
/// authenticating it.
pub struct RequestCertificate;

impl ClientCertVerifier for RequestCertificate {
    fn client_auth_root_subjects(
        &self,
        _sni: Option<&tokio_rustls::webpki::DNSName>,
    ) -> Option<rustls::DistinguishedNames> {
        Some(rustls::DistinguishedNames::new())
    }

    fn verify_client_cert(
        &self,
        _presented_certs: &[Certificate],
        _sni: Option<&tokio_rustls::webpki::DNSName>,
    ) -> Result<rustls::ClientCertVerified, rustls::TLSError> {
        Ok(rustls::ClientCertVerified::assertion())
    }

    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self, _sni: Option<&tokio_rustls::webpki::DNSName>) -> Option<bool> {
        Some(false)
    }
}
