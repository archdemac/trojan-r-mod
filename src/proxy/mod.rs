use std::{
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use log::LevelFilter;
use serde::Deserialize;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::Error;
use crate::protocol::{
    AcceptResult, ProxyAcceptor, ProxyConnector, ProxyTcpStream, ProxyUdpStream, UdpRead, UdpWrite,
};
#[cfg(feature = "server")]
use crate::protocol::direct::connector::DirectConnector;
#[cfg(feature = "forward")]
use crate::protocol::dokodemo::acceptor::{DokodemoAcceptor, DokodemoAcceptorConfig};
#[cfg(feature = "server")]
use crate::protocol::mux::acceptor::{MuxAcceptor, MuxAcceptorConfig};
#[cfg(any(feature = "client", feature = "forward"))]
use crate::protocol::mux::connector::{MuxConnector, MuxConnectorConfig};
#[cfg(feature = "server")]
use crate::protocol::plaintext::acceptor::{PlaintextAcceptor, PlaintextAcceptorConfig};
#[cfg(feature = "client")]
use crate::protocol::socks5::acceptor::{Socks5Acceptor, Socks5AcceptorConfig};
#[cfg(feature = "server")]
use crate::protocol::tls::acceptor::{TrojanTlsAcceptor, TrojanTlsAcceptorConfig};
#[cfg(any(feature = "client", feature = "forward"))]
use crate::protocol::tls::connector::{TrojanTlsConnector, TrojanTlsConnectorConfig};
#[cfg(feature = "server")]
use crate::protocol::trojan::acceptor::{TrojanAcceptor, TrojanAcceptorConfig};
#[cfg(any(feature = "client", feature = "forward"))]
use crate::protocol::trojan::connector::{TrojanConnector, TrojanConnectorConfig};
#[cfg(feature = "server")]
use crate::protocol::websocket::acceptor::{WebSocketAcceptor, WebSocketAcceptorConfig};
#[cfg(any(feature = "client", feature = "forward"))]
use crate::protocol::websocket::connector::{WebSocketConnector, WebSocketConnectorConfig};

const RELAY_BUFFER_SIZE: usize = 0x2000;

async fn copy_udp<R: UdpRead, W: UdpWrite>(r: &mut R, w: &mut W) -> io::Result<()> {
    let mut buf = [0u8; RELAY_BUFFER_SIZE];
    loop {
        let (len, addr) = r.read_from(&mut buf).await?;
        log::debug!("udp packet addr={} len={}", addr, len);
        if len == 0 {
            break;
        }
        w.write_to(&buf[..len], &addr).await?;
    }
    Ok(())
}

async fn copy_tcp<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    r: &mut R,
    w: &mut W,
) -> io::Result<()> {
    let mut buf = [0u8; RELAY_BUFFER_SIZE];
    loop {
        let len = r.read(&mut buf).await?;
        if len == 0 {
            break;
        }
        w.write_all(&buf[..len]).await?;
        w.flush().await?;
    }
    Ok(())
}

pub async fn relay_udp<T: ProxyUdpStream, U: ProxyUdpStream>(a: T, b: U) {
    let (mut a_rx, mut a_tx) = a.split();
    let (mut b_rx, mut b_tx) = b.split();
    let t1 = copy_udp(&mut a_rx, &mut b_tx);
    let t2 = copy_udp(&mut b_rx, &mut a_tx);
    let e = tokio::select! {
        e = t1 => {e}
        e = t2 => {e}
    };
    if let Err(e) = e {
        log::debug!("udp_relay err: {}", e)
    }
    let _ = T::reunite(a_rx, a_tx).close().await;
    let _ = U::reunite(b_rx, b_tx).close().await;
    log::info!("udp session ends");
}

pub async fn relay_tcp<T: ProxyTcpStream, U: ProxyTcpStream>(a: T, b: U) {
    let (mut a_rx, mut a_tx) = split(a);
    let (mut b_rx, mut b_tx) = split(b);
    let t1 = copy_tcp(&mut a_rx, &mut b_tx);
    let t2 = copy_tcp(&mut b_rx, &mut a_tx);
    let e = tokio::select! {
        e = t1 => {e}
        e = t2 => {e}
    };
    if let Err(e) = e {
        log::debug!("relay_tcp err: {}", e)
    }
    let mut a = a_rx.unsplit(a_tx);
    let mut b = b_rx.unsplit(b_tx);
    let _ = a.shutdown().await;
    let _ = b.shutdown().await;
    log::info!("tcp session ends");
}

#[derive(Deserialize)]
struct GlobalConfig {
    mode: String,
    log_level: Option<String>,
}

#[cfg(feature = "client")]
#[derive(Deserialize)]
struct ClientConfig {
    socks5: Socks5AcceptorConfig,
    trojan: TrojanConnectorConfig,
    tls: TrojanTlsConnectorConfig,
    websocket: Option<WebSocketConnectorConfig>,
    mux: Option<MuxConnectorConfig>,
}

#[cfg(feature = "server")]
#[derive(Deserialize)]
struct ServerConfig {
    trojan: TrojanAcceptorConfig,
    tls: Option<TrojanTlsAcceptorConfig>,
    plaintext: Option<PlaintextAcceptorConfig>,
    websocket: Option<WebSocketAcceptorConfig>,
    mux: Option<MuxAcceptorConfig>,
}

#[cfg(feature = "forward")]
#[derive(Deserialize)]
struct ForwardConfig {
    dokodemo: DokodemoAcceptorConfig,
    trojan: TrojanConnectorConfig,
    tls: TrojanTlsConnectorConfig,
    websocket: Option<WebSocketConnectorConfig>,
    mux: Option<MuxConnectorConfig>,
}

async fn run_proxy<I: ProxyAcceptor, O: ProxyConnector + 'static>(
    acceptor: I,
    connector: O,
) -> io::Result<()> {
    let connector = Arc::new(connector);
    loop {
        match acceptor.accept().await {
            Ok(AcceptResult::Tcp((inbound, addr))) => {
                let connector = connector.clone();
                tokio::spawn(async move {
                    match connector.connect_tcp(&addr).await {
                        Ok(outbound) => {
                            log::info!("relaying tcp stream to {}", addr);
                            relay_tcp(inbound, outbound).await;
                        }
                        Err(e) => {
                            log::error!("failed to relay tcp stream to {}: {}", addr, e);
                        }
                    }
                });
            }
            Ok(AcceptResult::Udp(inbound)) => {
                let connector = connector.clone();
                tokio::spawn(async move {
                    match connector.connect_udp().await {
                        Ok(outbound) => {
                            log::info!("relaying udp stream..");
                            relay_udp(inbound, outbound).await;
                        }
                        Err(e) => {
                            log::error!("failed to relay tcp stream: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                log::error!("accept failed: {}", e);
            }
        }
    }
}

pub async fn launch_from_config_filename(filename: String) -> io::Result<()> {
    let mut file = File::open(filename)?;
    let mut config_string = String::new();
    file.read_to_string(&mut config_string)?;
    launch_from_config_string(config_string).await
}

pub async fn launch_from_config_string(config_string: String) -> io::Result<()> {
    let config: GlobalConfig = toml::from_str(&config_string).map_err(|e| Error::new(e.to_string()))?;
    if let Some(log_level) = config.log_level {
        let level = match log_level.as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => {
                return Err(Error::new("invalid log_level").into());
            }
        };
        let _ = env_logger::builder().filter_level(level).try_init();
    } else {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Debug)
            .try_init();
    }
    match config.mode.as_str() {
        #[cfg(feature = "server")]
        "server" => {
            log::debug!("server mode");
            let config: ServerConfig =
                toml::from_str(&config_string).map_err(|e| Error::new(e.to_string()))?;
            let direct_connector = DirectConnector {};

            if let Some(tls_config) = config.tls {
                let tls_acceptor = TrojanTlsAcceptor::new(&tls_config).await?;
                if let Some(ws_config) = config.websocket {
                    let ws_acceptor = WebSocketAcceptor::new(&ws_config, tls_acceptor)?;
                    let trojan_acceptor = TrojanAcceptor::new(&config.trojan, ws_acceptor)?;
                    if let Some(mux_config) = config.mux {
                        let mux_acceptor = MuxAcceptor::new(trojan_acceptor, &mux_config)?;
                        run_proxy(mux_acceptor, direct_connector).await?;
                    } else {
                        run_proxy(trojan_acceptor, direct_connector).await?;
                    }
                } else {
                    let trojan_acceptor = TrojanAcceptor::new(&config.trojan, tls_acceptor)?;
                    if let Some(mux_config) = config.mux {
                        let mux_acceptor = MuxAcceptor::new(trojan_acceptor, &mux_config)?;
                        run_proxy(mux_acceptor, direct_connector).await?;
                    } else {
                        run_proxy(trojan_acceptor, direct_connector).await?;
                    }
                }
            } else if let Some(plaintext_config) = config.plaintext {
                let direct_acceptor = PlaintextAcceptor::new(&plaintext_config).await?;
                if let Some(ws_config) = config.websocket {
                    let ws_acceptor = WebSocketAcceptor::new(&ws_config, direct_acceptor)?;
                    let trojan_acceptor = TrojanAcceptor::new(&config.trojan, ws_acceptor)?;
                    if let Some(mux_config) = config.mux {
                        let mux_acceptor = MuxAcceptor::new(trojan_acceptor, &mux_config)?;
                        run_proxy(mux_acceptor, direct_connector).await?;
                    } else {
                        run_proxy(trojan_acceptor, direct_connector).await?;
                    }
                } else {
                    let trojan_acceptor = TrojanAcceptor::new(&config.trojan, direct_acceptor)?;
                    if let Some(mux_config) = config.mux {
                        let mux_acceptor = MuxAcceptor::new(trojan_acceptor, &mux_config)?;
                        run_proxy(mux_acceptor, direct_connector).await?;
                    } else {
                        run_proxy(trojan_acceptor, direct_connector).await?;
                    }
                }
            } else {
                return Err(Error::new("plaintext/tls section not found").into());
            }
        }
        #[cfg(feature = "client")]
        "client" => {
            log::debug!("client mode");
            let config: ClientConfig =
                toml::from_str(&config_string).map_err(|e| Error::new(e.to_string()))?;
            let socks5_acceptor = Socks5Acceptor::new(&config.socks5).await?;
            let tls_connector = TrojanTlsConnector::new(&config.tls)?;
            if let Some(ws_config) = config.websocket {
                let ws_connector = WebSocketConnector::new(&ws_config, tls_connector)?;
                let trojan_connector = TrojanConnector::new(&config.trojan, ws_connector)?;
                if let Some(mux_config) = config.mux {
                    let mux_connector = MuxConnector::new(&mux_config, trojan_connector).unwrap();
                    run_proxy(socks5_acceptor, mux_connector).await?;
                } else {
                    run_proxy(socks5_acceptor, trojan_connector).await?;
                }
            } else {
                let trojan_connector = TrojanConnector::new(&config.trojan, tls_connector)?;
                if let Some(mux_config) = config.mux {
                    let mux_connector = MuxConnector::new(&mux_config, trojan_connector).unwrap();
                    run_proxy(socks5_acceptor, mux_connector).await?;
                } else {
                    run_proxy(socks5_acceptor, trojan_connector).await?;
                }
            }
        }
        #[cfg(feature = "forward")]
        "forward" => {
            log::debug!("forward mode");
            let config: ForwardConfig =
                toml::from_str(&config_string).map_err(|e| Error::new(e.to_string()))?;
            let dokodemo_acceptor = DokodemoAcceptor::new(&config.dokodemo).await?;
            let tls_connector = TrojanTlsConnector::new(&config.tls)?;
            if let Some(ws_config) = config.websocket {
                let ws_connector = WebSocketConnector::new(&ws_config, tls_connector)?;
                let trojan_connector = TrojanConnector::new(&config.trojan, ws_connector)?;
                if let Some(mux_config) = config.mux {
                    let mux_connector = MuxConnector::new(&mux_config, trojan_connector).unwrap();
                    run_proxy(dokodemo_acceptor, mux_connector).await?;
                } else {
                    run_proxy(dokodemo_acceptor, trojan_connector).await?;
                }
            } else {
                let trojan_connector = TrojanConnector::new(&config.trojan, tls_connector)?;
                if let Some(mux_config) = config.mux {
                    let mux_connector = MuxConnector::new(&mux_config, trojan_connector).unwrap();
                    run_proxy(dokodemo_acceptor, mux_connector).await?;
                } else {
                    run_proxy(dokodemo_acceptor, trojan_connector).await?;
                }
            }
        }
        _ => {
            log::error!("invalid mode: {}", config.mode.as_str());
        }
    }
    Ok(())
}
