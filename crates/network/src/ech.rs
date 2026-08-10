use crate::gateway::ProbeError;
use base64::Engine;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use rustls::client::{EchConfig, EchMode, EchStatus};
use rustls::crypto::aws_lc_rs;
use rustls::pki_types::{EchConfigListBytes, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

const ECH_BOOTSTRAP_HOST: &str = "cloudflare-ech.com";
const ALIDNS_HOST: &str = "dns.alidns.com";
const ALIDNS_URL: &str = "https://dns.alidns.com/resolve?name=cloudflare-ech.com&type=HTTPS";
const ALIDNS_ADDRESSES: [[u8; 4]; 2] = [[223, 5, 5, 5], [223, 6, 6, 6]];
const FALLBACK_ECH_ADDRESSES: [[u8; 4]; 2] = [[104, 18, 10, 118], [104, 18, 11, 118]];

pub(crate) struct EchProbeOutcome {
    pub connected_ip: IpAddr,
    pub http_status: u16,
    pub candidate_address_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EchBundle {
    config: Vec<u8>,
    addresses: Vec<SocketAddr>,
    ttl_seconds: u32,
}

#[derive(Debug, Deserialize)]
struct DnsJsonResponse {
    #[serde(rename = "Status")]
    status: u16,
    #[serde(rename = "Answer", default)]
    answers: Vec<DnsAnswer>,
}

#[derive(Debug, Deserialize)]
struct DnsAnswer {
    #[serde(rename = "TTL")]
    ttl: u32,
    #[serde(rename = "type")]
    record_type: u16,
    data: String,
}

pub(crate) fn probe(host: &str, timeout: Duration) -> Result<EchProbeOutcome, ProbeError> {
    validate_ech_host(host)?;
    let bundle = fetch_bundle(timeout)?;
    probe_with_bundle(host, timeout, &bundle)
}

pub(crate) fn verified_client(host: &str, timeout: Duration) -> Result<Client, ProbeError> {
    validate_ech_host(host)?;
    let bundle = fetch_bundle(timeout)?;
    probe_with_bundle(host, timeout, &bundle)?;
    let mut tls_config = client_config(&bundle.config)?;
    tls_config.alpn_protocols = vec![b"http/1.1".to_vec()];

    Client::builder()
        .tls_backend_preconfigured(tls_config)
        .https_only(true)
        .no_proxy()
        .resolve_to_addrs(host, &bundle.addresses)
        .timeout(timeout)
        .redirect(Policy::none())
        .build()
        .map_err(|_| ProbeError::ConnectionFailed {
            host: host.to_owned(),
        })
}

fn validate_ech_host(host: &str) -> Result<(), ProbeError> {
    if !matches!(
        host.trim_end_matches('.').to_ascii_lowercase().as_str(),
        "app-api.pixiv.net" | "oauth.secure.pixiv.net" | "accounts.pixiv.net"
    ) {
        return Err(ProbeError::EchUnavailable {
            host: host.to_owned(),
        });
    }

    Ok(())
}

fn probe_with_bundle(
    host: &str,
    timeout: Duration,
    bundle: &EchBundle,
) -> Result<EchProbeOutcome, ProbeError> {
    let tls_config = Arc::new(client_config(&bundle.config)?);
    let server_name =
        ServerName::try_from(host.to_owned()).map_err(|_| ProbeError::InvalidHost {
            host: host.to_owned(),
        })?;
    let mut saw_non_accepted_handshake = false;

    for address in &bundle.addresses {
        let Ok(socket) = TcpStream::connect_timeout(address, timeout) else {
            continue;
        };
        let _ = socket.set_read_timeout(Some(timeout));
        let _ = socket.set_write_timeout(Some(timeout));
        let Ok(connection) = ClientConnection::new(tls_config.clone(), server_name.clone()) else {
            continue;
        };
        let mut stream = StreamOwned::new(connection, socket);
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {host}\r\nUser-Agent: PixivClient/0.2 connectivity-probe\r\nAccept: */*\r\nConnection: close\r\n\r\n"
        );
        if stream.write_all(request.as_bytes()).is_err() || stream.flush().is_err() {
            continue;
        }
        if stream.conn.ech_status() != EchStatus::Accepted {
            saw_non_accepted_handshake = true;
            continue;
        }

        let mut response = [0_u8; 1024];
        let Ok(read) = stream.read(&mut response) else {
            continue;
        };
        let status =
            parse_http_status(&response[..read]).ok_or_else(|| ProbeError::HttpProtocolError {
                host: host.to_owned(),
            })?;
        return Ok(EchProbeOutcome {
            connected_ip: address.ip(),
            http_status: status,
            candidate_address_count: bundle.addresses.len().min(usize::from(u16::MAX)) as u16,
        });
    }

    if saw_non_accepted_handshake {
        Err(ProbeError::EchNotAccepted {
            host: host.to_owned(),
        })
    } else {
        Err(ProbeError::ConnectionFailed {
            host: host.to_owned(),
        })
    }
}

fn client_config(encoded_config: &[u8]) -> Result<ClientConfig, ProbeError> {
    let ech_config = EchConfig::new(
        EchConfigListBytes::from(encoded_config.to_vec()),
        aws_lc_rs::hpke::ALL_SUPPORTED_SUITES,
    )
    .map_err(|_| ProbeError::EchConfigUnavailable {
        host: ECH_BOOTSTRAP_HOST.into(),
    })?;
    Ok(
        ClientConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
            .with_ech(EchMode::from(ech_config))
            .map_err(|_| ProbeError::EchConfigUnavailable {
                host: ECH_BOOTSTRAP_HOST.into(),
            })?
            .with_root_certificates(root_store())
            .with_no_client_auth(),
    )
}

fn fetch_bundle(timeout: Duration) -> Result<EchBundle, ProbeError> {
    let tls = ClientConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|_| ProbeError::DnsQueryFailed {
            host: ECH_BOOTSTRAP_HOST.into(),
        })?
        .with_root_certificates(root_store())
        .with_no_client_auth();
    let alidns_addresses =
        ALIDNS_ADDRESSES.map(|octets| SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), 443));
    let client = Client::builder()
        .tls_backend_preconfigured(tls)
        .no_proxy()
        .resolve_to_addrs(ALIDNS_HOST, &alidns_addresses)
        .timeout(timeout)
        .build()
        .map_err(|_| ProbeError::DnsQueryFailed {
            host: ECH_BOOTSTRAP_HOST.into(),
        })?;
    let response = client
        .get(ALIDNS_URL)
        .header("accept", "application/dns-json")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|_| ProbeError::DnsQueryFailed {
            host: ECH_BOOTSTRAP_HOST.into(),
        })?;
    let dns: DnsJsonResponse = response.json().map_err(|_| ProbeError::DnsQueryFailed {
        host: ECH_BOOTSTRAP_HOST.into(),
    })?;
    parse_dns_response(dns)
}

fn parse_dns_response(response: DnsJsonResponse) -> Result<EchBundle, ProbeError> {
    if response.status != 0 {
        return Err(ProbeError::DnsQueryFailed {
            host: ECH_BOOTSTRAP_HOST.into(),
        });
    }

    for answer in response.answers {
        if answer.record_type != 65 {
            continue;
        }
        let Some(encoded_config) = svc_parameter(&answer.data, "ech") else {
            continue;
        };
        let config = base64::engine::general_purpose::STANDARD
            .decode(encoded_config)
            .map_err(|_| ProbeError::EchConfigUnavailable {
                host: ECH_BOOTSTRAP_HOST.into(),
            })?;
        let addresses = svc_parameter(&answer.data, "ipv4hint")
            .map(parse_ipv4_hints)
            .filter(|addresses| !addresses.is_empty())
            .unwrap_or_else(fallback_addresses);
        return Ok(EchBundle {
            config,
            addresses,
            ttl_seconds: answer.ttl,
        });
    }

    Err(ProbeError::EchConfigUnavailable {
        host: ECH_BOOTSTRAP_HOST.into(),
    })
}

fn svc_parameter<'a>(record: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("{name}=");
    let value = record
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&marker))?;
    Some(value.trim_matches('"'))
}

fn parse_ipv4_hints(value: &str) -> Vec<SocketAddr> {
    value
        .split(',')
        .filter_map(|value| value.parse::<Ipv4Addr>().ok())
        .map(|address| SocketAddr::new(IpAddr::V4(address), 443))
        .collect()
}

fn fallback_addresses() -> Vec<SocketAddr> {
    FALLBACK_ECH_ADDRESSES
        .into_iter()
        .map(|octets| SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), 443))
        .collect()
}

fn root_store() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    roots
}

fn parse_http_status(response: &[u8]) -> Option<u16> {
    let first_line = std::str::from_utf8(response).ok()?.lines().next()?;
    let mut parts = first_line.split_whitespace();
    if !parts.next()?.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{parse_dns_response, parse_http_status, DnsAnswer, DnsJsonResponse};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn parses_ech_config_and_cloudflare_ipv4_hints() {
        let response = DnsJsonResponse {
            status: 0,
            answers: vec![DnsAnswer {
                ttl: 221,
                record_type: 65,
                data: "1 . alpn=\"h3,h2\" ipv4hint=\"104.18.10.118,104.18.11.118\" ech=\"AEX+DQBBxwAgACD64SRg36XkWhRQbHIp4lBdtTDCX31oTlf8ZtXx4X7sZwAEAAEAAQASY2xvdWRmbGFyZS1lY2guY29tAAA=\"".into(),
            }],
        };

        let bundle = parse_dns_response(response).unwrap();

        assert!(!bundle.config.is_empty());
        assert_eq!(bundle.ttl_seconds, 221);
        assert_eq!(
            bundle.addresses[0].ip(),
            IpAddr::V4(Ipv4Addr::new(104, 18, 10, 118))
        );
        assert_eq!(
            bundle.addresses[1].ip(),
            IpAddr::V4(Ipv4Addr::new(104, 18, 11, 118))
        );
    }

    #[test]
    fn parses_http_status_line_only() {
        assert_eq!(parse_http_status(b"HTTP/1.1 404 Not Found\r\n"), Some(404));
        assert_eq!(parse_http_status(b"not http"), None);
    }
}
