// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # TLS inside the binary (roadmap F2a)
//!
//! `kshana-pro serve --tls-cert <chain.pem> --tls-key <key.pem>` serves HTTPS directly,
//! with no reverse proxy: TLS 1.3 and 1.2, the rustls defaults (forward-secret cipher
//! suites only), the ring cryptography provider. The chain and its key are PEM
//! files the operator supplies; the server reads them once at start. A plain-HTTP
//! request to an HTTPS server fails the handshake and is closed.

use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

fn other(msg: String) -> std::io::Error {
    std::io::Error::other(msg)
}

/// Build the server configuration from a PEM chain and a PEM private key
/// (PKCS #8, PKCS #1 or SEC1).
pub fn server_config(cert: &Path, key: &Path) -> std::io::Result<Arc<rustls::ServerConfig>> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert)
        .map_err(|e| other(format!("{}: {e}", cert.display())))?
        .collect::<Result<_, _>>()
        .map_err(|e| other(format!("{}: {e}", cert.display())))?;
    if certs.is_empty() {
        return Err(other(format!("{}: no PEM chain found", cert.display())));
    }
    let key =
        PrivateKeyDer::from_pem_file(key).map_err(|e| other(format!("{}: {e}", key.display())))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| other(format!("TLS: {e}")))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| other(format!("TLS chain and key: {e}")))?;
    Ok(Arc::new(cfg))
}
