use crate::protocol::{Address, DummyUdpStream, ProxyConnector, ProxyTcpStream};
use async_trait::async_trait;
use serde::Deserialize;
use std::{
    io,
    path::Path,
    sync::Arc,
};
use tokio::net::TcpStream;
use tokio_rustls::{
    client::TlsStream,
    rustls::{
        pki_types::ServerName,
        ClientConfig, RootCertStore,
    },
    TlsConnector,
};

use super::{build_provider, load_cert};

#[derive(Deserialize)]
pub struct TrojanTlsConnectorConfig {
    addr: String,
    sni: String,
    cipher: Option<Vec<String>>,
    cert: Option<String>,
}

pub struct TrojanTlsConnector {
    sni: String,
    server_addr: String,
    tls_config: Arc<ClientConfig>,
}

impl ProxyTcpStream for TlsStream<TcpStream> {}

impl TrojanTlsConnector {
    pub fn new(config: &TrojanTlsConnectorConfig) -> io::Result<Self> {
        let mut root_store = RootCertStore::empty();
        if let Some(ref cert_path) = config.cert {
            let certs = load_cert(Path::new(cert_path))?;
            for cert in certs {
                root_store
                    .add(cert)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
            }
        } else {
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }

        let provider = build_provider(config.cipher.clone())?;
        let tls_config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(Self {
            sni: config.sni.clone(),
            server_addr: config.addr.clone(),
            tls_config: Arc::new(tls_config),
        })
    }
}

#[async_trait]
impl ProxyConnector for TrojanTlsConnector {
    type TS = TlsStream<TcpStream>;
    type US = DummyUdpStream;

    async fn connect_tcp(&self, _: &Address) -> io::Result<Self::TS> {
        let stream = TcpStream::connect(&self.server_addr).await?;
        stream.set_nodelay(true)?;

        let server_name = ServerName::try_from(self.sni.clone())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let stream = TlsConnector::from(self.tls_config.clone())
            .connect(server_name, stream)
            .await?;

        log::info!("connected to {}", self.server_addr);
        Ok(stream)
    }

    async fn connect_udp(&self) -> io::Result<Self::US> {
        unimplemented!()
    }
}
