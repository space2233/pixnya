use crate::gateway::compatible_socket_address;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{aws_lc_rs, verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream as StdTcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::time::timeout;
use tokio_rustls::{TlsAcceptor, TlsConnector};

const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(12);
const LOGIN_CERTIFICATE_HOSTS: [&str; 10] = [
    "pixiv.net",
    "*.pixiv.net",
    "app-api.pixiv.net",
    "oauth.secure.pixiv.net",
    "accounts.pixiv.net",
    "www.pixiv.net",
    "pximg.net",
    "*.pximg.net",
    "i.pximg.net",
    "s.pximg.net",
];

/// Chooses the TLS boundary used by the loopback login proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginProxyMode {
    /// Connect Pixiv hosts by fixed IP while leaving TLS end-to-end in the WebView.
    EndToEndFixedIp,
    /// Terminate WebView TLS locally, then connect to a fixed Pixiv IP without
    /// upstream SNI or certificate-chain verification.
    InsecureTlsBridge,
}

/// A short-lived, loopback-only HTTP CONNECT proxy for the login WebView.
///
/// The insecure bridge is deliberately isolated here. It only activates for
/// hosts in `compatible_socket_address`; every other HTTPS target remains a raw
/// end-to-end tunnel. No HTTP payload is parsed or logged.
pub struct LoginProxy {
    address: SocketAddr,
    certificate_sha256: Option<String>,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LoginProxy {
    pub fn start(mode: LoginProxyMode) -> Result<Self, LoginProxyError> {
        let listener =
            TcpListener::bind(("127.0.0.1", 0)).map_err(|_| LoginProxyError::BindFailed)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| LoginProxyError::BindFailed)?;
        let address = listener
            .local_addr()
            .map_err(|_| LoginProxyError::BindFailed)?;
        let (bridge, certificate_sha256) = match mode {
            LoginProxyMode::EndToEndFixedIp => (None, None),
            LoginProxyMode::InsecureTlsBridge => {
                let material = InsecureBridge::new()?;
                let fingerprint = material.certificate_sha256.clone();
                (Some(material), Some(fingerprint))
            }
        };
        let configuration = ProxyConfiguration { mode, bridge };
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|_| LoginProxyError::RuntimeUnavailable)?;
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = running.clone();
        let worker = thread::Builder::new()
            .name("pixiv-login-proxy".into())
            .spawn(move || run_proxy(runtime, listener, configuration, worker_running))
            .map_err(|_| LoginProxyError::ThreadUnavailable)?;

        Ok(Self {
            address,
            certificate_sha256,
            running,
            worker: Some(worker),
        })
    }

    pub fn port(&self) -> u16 {
        self.address.port()
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// SHA-256 of the one-time leaf certificate used by the local TLS bridge.
    pub fn certificate_sha256(&self) -> Option<&str> {
        self.certificate_sha256.as_deref()
    }
}

impl Drop for LoginProxy {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let _ = StdTcpStream::connect_timeout(&self.address, Duration::from_millis(100));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginProxyError {
    BindFailed,
    ThreadUnavailable,
    RuntimeUnavailable,
    CertificateGenerationFailed,
    TlsConfigurationFailed,
}

impl fmt::Display for LoginProxyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BindFailed => "failed to bind the loopback login proxy",
            Self::ThreadUnavailable => "failed to start the login proxy worker",
            Self::RuntimeUnavailable => "failed to create the login proxy runtime",
            Self::CertificateGenerationFailed => {
                "failed to generate the one-time login bridge certificate"
            }
            Self::TlsConfigurationFailed => "failed to configure the login TLS bridge",
        })
    }
}

impl std::error::Error for LoginProxyError {}

#[derive(Clone)]
struct ProxyConfiguration {
    mode: LoginProxyMode,
    bridge: Option<InsecureBridge>,
}

#[derive(Clone)]
struct InsecureBridge {
    acceptor: TlsAcceptor,
    connector: TlsConnector,
    certificate_sha256: String,
}

impl InsecureBridge {
    fn new() -> Result<Self, LoginProxyError> {
        let CertifiedKey { cert, signing_key } = generate_simple_self_signed(
            LOGIN_CERTIFICATE_HOSTS
                .iter()
                .map(|host| (*host).to_owned())
                .collect::<Vec<_>>(),
        )
        .map_err(|_| LoginProxyError::CertificateGenerationFailed)?;
        let certificate = cert.der().clone();
        let certificate_sha256 = sha256_hex(certificate.as_ref());
        let private_key =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
        let provider = Arc::new(aws_lc_rs::default_provider());
        let mut server = ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|_| LoginProxyError::TlsConfigurationFailed)?
            .with_no_client_auth()
            .with_single_cert(vec![certificate], private_key)
            .map_err(|_| LoginProxyError::TlsConfigurationFailed)?;
        server.alpn_protocols = vec![b"http/1.1".to_vec()];

        let mut client = ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|_| LoginProxyError::TlsConfigurationFailed)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoServerCertificateVerification {
                provider,
            }))
            .with_no_client_auth();
        client.enable_sni = false;
        client.alpn_protocols = vec![b"http/1.1".to_vec()];

        Ok(Self {
            acceptor: TlsAcceptor::from(Arc::new(server)),
            connector: TlsConnector::from(Arc::new(client)),
            certificate_sha256,
        })
    }
}

#[derive(Debug)]
struct NoServerCertificateVerification {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for NoServerCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            certificate,
            signed,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            certificate,
            signed,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn run_proxy(
    runtime: Runtime,
    listener: TcpListener,
    configuration: ProxyConfiguration,
    running: Arc<AtomicBool>,
) {
    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };
        while running.load(Ordering::Acquire) {
            match listener.accept().await {
                Ok((client, _)) if running.load(Ordering::Acquire) => {
                    let configuration = configuration.clone();
                    tokio::spawn(async move {
                        let _ = handle_client(client, configuration).await;
                    });
                }
                Ok(_) => break,
                Err(_) => break,
            }
        }
    });
}

async fn handle_client(mut client: TcpStream, configuration: ProxyConfiguration) -> io::Result<()> {
    let request_head = timeout(CONNECT_TIMEOUT, read_request_head(&mut client))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "proxy request timed out"))??;
    let request = std::str::from_utf8(&request_head)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid proxy request"))?;
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let authority = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();

    if method != "CONNECT" || !version.starts_with("HTTP/1.") {
        write_proxy_error(&mut client, 405, "CONNECT required").await?;
        return Ok(());
    }

    let Some((host, port)) = parse_authority(authority) else {
        write_proxy_error(&mut client, 400, "Invalid CONNECT target").await?;
        return Ok(());
    };
    if port != 443 {
        write_proxy_error(&mut client, 403, "Only HTTPS tunnels are allowed").await?;
        return Ok(());
    }

    let fixed_address = compatible_socket_address(&host, port);
    match (configuration.mode, fixed_address, configuration.bridge) {
        (LoginProxyMode::InsecureTlsBridge, Some(address), Some(bridge)) => {
            bridge_fixed_pixiv_host(client, address, bridge).await
        }
        (_, address, _) => tunnel_end_to_end(client, &host, port, address).await,
    }
}

async fn bridge_fixed_pixiv_host(
    mut client: TcpStream,
    address: SocketAddr,
    bridge: InsecureBridge,
) -> io::Result<()> {
    let upstream = connect_address(address).await?;
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    let mut client_tls = timeout(CONNECT_TIMEOUT, bridge.acceptor.accept(client))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "local TLS handshake timed out"))??;
    let server_name = ServerName::IpAddress(ip_server_name(address.ip()));
    let mut upstream_tls = timeout(
        CONNECT_TIMEOUT,
        bridge.connector.connect(server_name, upstream),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "upstream TLS handshake timed out"))??;

    copy_bidirectional(&mut client_tls, &mut upstream_tls)
        .await
        .map(|_| ())
}

async fn tunnel_end_to_end(
    mut client: TcpStream,
    host: &str,
    port: u16,
    fixed_address: Option<SocketAddr>,
) -> io::Result<()> {
    let mut upstream = if let Some(address) = fixed_address {
        connect_address(address).await?
    } else {
        timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "upstream connect timed out"))??
    };
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    copy_bidirectional(&mut client, &mut upstream)
        .await
        .map(|_| ())
}

async fn connect_address(address: SocketAddr) -> io::Result<TcpStream> {
    timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "upstream connect timed out"))?
}

async fn read_request_head(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut received = Vec::with_capacity(1024);
    let mut byte = [0_u8; 1];

    loop {
        stream.read_exact(&mut byte).await?;
        received.push(byte[0]);
        if received.ends_with(b"\r\n\r\n") {
            return Ok(received);
        }
        if received.len() >= MAX_REQUEST_HEAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "proxy request headers are too large",
            ));
        }
    }
}

fn parse_authority(authority: &str) -> Option<(String, u16)> {
    if authority.is_empty()
        || authority.contains(['/', '\\', '@'])
        || authority.chars().any(char::is_whitespace)
    {
        return None;
    }

    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let closing = rest.find(']')?;
        let host = &rest[..closing];
        let port = rest[closing + 1..].strip_prefix(':')?;
        (host, port)
    } else {
        authority.rsplit_once(':')?
    };
    let host = host.trim_end_matches('.');
    let port = port.parse::<u16>().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }

    Some((host.to_ascii_lowercase(), port))
}

async fn write_proxy_error(stream: &mut TcpStream, status: u16, reason: &str) -> io::Result<()> {
    stream
        .write_all(
            format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
                .as_bytes(),
        )
        .await
}

fn ip_server_name(address: IpAddr) -> rustls::pki_types::IpAddr {
    address.into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_authority, InsecureBridge, LoginProxy, LoginProxyMode};

    #[test]
    fn parses_https_connect_authorities() {
        assert_eq!(
            parse_authority("accounts.pixiv.net:443"),
            Some(("accounts.pixiv.net".into(), 443))
        );
        assert_eq!(
            parse_authority("[2001:db8::1]:443"),
            Some(("2001:db8::1".into(), 443))
        );
    }

    #[test]
    fn rejects_ambiguous_or_injected_authorities() {
        for authority in [
            "accounts.pixiv.net",
            "accounts.pixiv.net:0",
            "user@accounts.pixiv.net:443",
            "accounts.pixiv.net:443/path",
            "accounts.pixiv.net:443\r\nInjected: true",
        ] {
            assert_eq!(parse_authority(authority), None, "{authority}");
        }
    }

    #[test]
    fn insecure_bridge_uses_a_new_pinned_certificate_for_each_session() {
        let first = InsecureBridge::new().unwrap();
        let second = InsecureBridge::new().unwrap();

        assert_eq!(first.certificate_sha256.len(), 64);
        assert_ne!(first.certificate_sha256, second.certificate_sha256);
    }

    #[test]
    fn end_to_end_proxy_has_no_local_certificate_pin() {
        let proxy = LoginProxy::start(LoginProxyMode::EndToEndFixedIp).unwrap();
        assert_eq!(proxy.certificate_sha256(), None);
    }
}
