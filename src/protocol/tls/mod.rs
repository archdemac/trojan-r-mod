use rustls_pemfile::{certs, private_key};
use std::{
    fs::File,
    io::{self, BufReader},
    path::Path,
    sync::Arc,
};
use tokio_rustls::rustls::{
    crypto::{ring::ALL_CIPHER_SUITES, CryptoProvider},
    pki_types::{CertificateDer, PrivateKeyDer},
    CipherSuite, SupportedCipherSuite,
};

use crate::error::Error;

#[cfg(feature = "server")]
pub mod acceptor;
#[cfg(any(feature = "client", feature = "forward"))]
pub mod connector;

fn new_error<T: ToString>(message: T) -> io::Error {
    Error::new(format!("tls: {}", message.to_string())).into()
}

fn load_cert(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    let mut reader = BufReader::new(File::open(path)?);
    let certs = certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
fn load_key(path: &Path) -> io::Result<Vec<PrivateKeyDer<'static>>> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut keys = Vec::new();
    while let Some(key) = private_key(&mut reader)? {
        keys.push(key);
    }
    if keys.is_empty() {
        return Err(new_error("no valid key found"));
    }
    Ok(keys)
}

fn get_cipher_name(cipher: &SupportedCipherSuite) -> &'static str {
    match cipher.suite() {
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256 => "TLS13_CHACHA20_POLY1305_SHA256",
        CipherSuite::TLS13_AES_256_GCM_SHA384 => "TLS13_AES_256_GCM_SHA384",
        CipherSuite::TLS13_AES_128_GCM_SHA256 => "TLS13_AES_128_GCM_SHA256",
        CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256 => {
            "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256"
        }
        CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256 => {
            "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256"
        }
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384 => {
            "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384"
        }
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 => {
            "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256"
        }
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384 => {
            "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384"
        }
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 => {
            "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"
        }
        _ => "???",
    }
}

fn get_cipher_suite(cipher: Option<Vec<String>>) -> io::Result<Vec<SupportedCipherSuite>> {
    if cipher.is_none() {
        return Ok(ALL_CIPHER_SUITES.to_vec());
    }
    let cipher = cipher.unwrap();
    let mut result = Vec::new();

    for name in cipher {
        let mut found = false;
        for i in ALL_CIPHER_SUITES {
            if name == get_cipher_name(i) {
                result.push(*i);
                found = true;
                log::debug!("cipher: {} applied", name);
                break;
            }
        }
        if !found {
            return Err(new_error(format!("bad cipher: {}", name)));
        }
    }
    Ok(result)
}

fn build_provider(cipher: Option<Vec<String>>) -> io::Result<Arc<CryptoProvider>> {
    let mut provider = tokio_rustls::rustls::crypto::ring::default_provider();
    provider.cipher_suites = get_cipher_suite(cipher)?;
    Ok(Arc::new(provider))
}
