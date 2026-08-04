use crate::{ech, ConnectionPolicy};
use pixiv_client_domain::{
    ConnectionMode, PlatformCapabilities, PolicyError, RoutePlan, RouteRequest, TrafficClass,
};
use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use serde::Serialize;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

const PROBE_USER_AGENT: &str = "PixivClient/0.1 connectivity-probe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRequest {
    pub mode: ConnectionMode,
    pub traffic: TrafficClass,
    pub host: String,
    pub unsafe_acknowledged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeEchStatus {
    NotApplicable,
    Accepted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProbe {
    pub route: RoutePlan,
    pub host: String,
    pub connected_ip: Option<String>,
    pub candidate_address_count: Option<u16>,
    pub http_status: u16,
    pub latency_ms: u64,
    pub dns_source: String,
    pub tls_summary: String,
    pub ech_status: ProbeEchStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProbeError {
    InvalidHost { host: String },
    UnsafeAcknowledgementRequired { host: String },
    EchUnavailable { host: String },
    CompatibleDirectUnavailable { host: String },
    InsecureTransportForbidden { host: String },
    WebViewProxyUnavailable { host: String },
    WebViewTransportUnavailable { host: String },
    DnsQueryFailed { host: String },
    EchConfigUnavailable { host: String },
    EchNotAccepted { host: String },
    ConnectionFailed { host: String },
    HttpProtocolError { host: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (stage, host) = match self {
            Self::InvalidHost { host } => ("invalid host", host),
            Self::UnsafeAcknowledgementRequired { host } => {
                ("unsafe mode acknowledgement required", host)
            }
            Self::EchUnavailable { host } => ("ECH transport unavailable", host),
            Self::CompatibleDirectUnavailable { host } => {
                ("compatible direct transport unavailable", host)
            }
            Self::InsecureTransportForbidden { host } => (
                "insecure transport is forbidden for sensitive traffic",
                host,
            ),
            Self::WebViewProxyUnavailable { host } => ("WebView proxy unavailable", host),
            Self::WebViewTransportUnavailable { host } => (
                "WebView transport cannot be probed by the Rust gateway",
                host,
            ),
            Self::DnsQueryFailed { host } => ("DoH query failed", host),
            Self::EchConfigUnavailable { host } => ("ECH config unavailable", host),
            Self::EchNotAccepted { host } => ("ECH was not accepted", host),
            Self::ConnectionFailed { host } => ("connection failed", host),
            Self::HttpProtocolError { host } => ("invalid HTTP response", host),
        };
        write!(formatter, "{stage}: {host}")
    }
}

impl std::error::Error for ProbeError {}

#[derive(Debug, Clone)]
pub struct NetworkGateway {
    timeout: Duration,
}

impl Default for NetworkGateway {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(12),
        }
    }
}

impl NetworkGateway {
    pub fn capabilities() -> PlatformCapabilities {
        PlatformCapabilities {
            rust_ech: true,
            rust_compatible_direct: true,
            webview_proxy: cfg!(any(
                target_os = "windows",
                target_os = "linux",
                target_os = "android"
            )),
            webview_insecure_bridge: cfg!(target_os = "android"),
        }
    }

    pub fn probe(&self, request: &ProbeRequest) -> Result<ConnectionProbe, ProbeError> {
        if !is_probe_host(&request.host) {
            return Err(ProbeError::InvalidHost {
                host: request.host.clone(),
            });
        }

        let route = ConnectionPolicy
            .evaluate(&RouteRequest {
                mode: request.mode,
                traffic: request.traffic,
                host: request.host.clone(),
                capabilities: Self::capabilities(),
            })
            .map_err(ProbeError::from)?;

        if request.traffic == TrafficClass::LoginWebView {
            return Err(ProbeError::WebViewTransportUnavailable {
                host: request.host.clone(),
            });
        }

        match request.mode {
            ConnectionMode::Standard => self.probe_standard(route, &request.host),
            ConnectionMode::Ech => self.probe_ech(route, &request.host),
            ConnectionMode::Compatible if !request.unsafe_acknowledged => {
                Err(ProbeError::UnsafeAcknowledgementRequired {
                    host: request.host.clone(),
                })
            }
            ConnectionMode::Compatible => self.probe_compatible(route, &request.host),
        }
    }

    pub fn build_client(&self, request: &ProbeRequest) -> Result<Client, ProbeError> {
        if !is_probe_host(&request.host) {
            return Err(ProbeError::InvalidHost {
                host: request.host.clone(),
            });
        }

        ConnectionPolicy
            .evaluate(&RouteRequest {
                mode: request.mode,
                traffic: request.traffic,
                host: request.host.clone(),
                capabilities: Self::capabilities(),
            })
            .map_err(ProbeError::from)?;
        if request.traffic == TrafficClass::LoginWebView {
            return Err(ProbeError::WebViewTransportUnavailable {
                host: request.host.clone(),
            });
        }
        if request.mode == ConnectionMode::Compatible && !request.unsafe_acknowledged {
            return Err(ProbeError::UnsafeAcknowledgementRequired {
                host: request.host.clone(),
            });
        }

        match request.mode {
            ConnectionMode::Standard => Client::builder()
                .tls_backend_rustls()
                .https_only(true)
                .timeout(self.timeout)
                .redirect(Policy::none())
                .build()
                .map_err(|_| ProbeError::ConnectionFailed {
                    host: request.host.clone(),
                }),
            ConnectionMode::Ech => ech::verified_client(&request.host, self.timeout),
            ConnectionMode::Compatible => {
                let address =
                    compatible_address(&request.host).ok_or_else(|| ProbeError::InvalidHost {
                        host: request.host.clone(),
                    })?;
                Client::builder()
                    .tls_backend_rustls()
                    .https_only(true)
                    .no_proxy()
                    .resolve(&request.host, address)
                    .tls_sni(false)
                    .tls_danger_accept_invalid_certs(true)
                    .timeout(self.timeout)
                    .redirect(Policy::none())
                    .build()
                    .map_err(|_| ProbeError::ConnectionFailed {
                        host: request.host.clone(),
                    })
            }
        }
    }

    fn probe_standard(&self, route: RoutePlan, host: &str) -> Result<ConnectionProbe, ProbeError> {
        let client = Client::builder()
            .tls_backend_rustls()
            .timeout(self.timeout)
            .redirect(Policy::none())
            .user_agent(PROBE_USER_AGENT)
            .build()
            .map_err(|_| ProbeError::ConnectionFailed {
                host: host.to_owned(),
            })?;
        let started = Instant::now();
        let response = send_probe(&client, host)?;

        Ok(report_from_response(
            route,
            host,
            response,
            started,
            "系统 DNS / 代理",
            "系统 TLS（证书已验证）",
            None,
        ))
    }

    fn probe_ech(&self, route: RoutePlan, host: &str) -> Result<ConnectionProbe, ProbeError> {
        let started = Instant::now();
        let outcome = ech::probe(host, self.timeout)?;
        Ok(ConnectionProbe {
            route,
            host: host.to_owned(),
            connected_ip: Some(outcome.connected_ip.to_string()),
            candidate_address_count: Some(outcome.candidate_address_count),
            http_status: outcome.http_status,
            latency_ms: elapsed_millis(started),
            dns_source: format!("AliDNS DoH · TTL {}s", outcome.ttl_seconds),
            tls_summary: "TLS 1.3 + ECH（证书已验证）".into(),
            ech_status: ProbeEchStatus::Accepted,
        })
    }

    fn probe_compatible(
        &self,
        route: RoutePlan,
        host: &str,
    ) -> Result<ConnectionProbe, ProbeError> {
        let address = compatible_address(host).ok_or_else(|| ProbeError::InvalidHost {
            host: host.to_owned(),
        })?;
        let client = Client::builder()
            .tls_backend_rustls()
            .no_proxy()
            .resolve(host, address)
            .tls_sni(false)
            .tls_danger_accept_invalid_certs(true)
            .timeout(self.timeout)
            .redirect(Policy::none())
            .user_agent(PROBE_USER_AGENT)
            .build()
            .map_err(|_| ProbeError::ConnectionFailed {
                host: host.to_owned(),
            })?;
        let started = Instant::now();
        let response = send_probe(&client, host)?;

        Ok(report_from_response(
            route,
            host,
            response,
            started,
            "内置 Pixiv IP 白名单",
            "TLS（SNI 与证书验证已关闭）",
            Some(1),
        ))
    }
}

fn send_probe(client: &Client, host: &str) -> Result<Response, ProbeError> {
    client
        .get(format!("https://{host}/"))
        .header("accept", "*/*")
        .send()
        .map_err(|_| ProbeError::ConnectionFailed {
            host: host.to_owned(),
        })
}

fn report_from_response(
    route: RoutePlan,
    host: &str,
    response: Response,
    started: Instant,
    dns_source: &str,
    tls_summary: &str,
    candidate_address_count: Option<u16>,
) -> ConnectionProbe {
    ConnectionProbe {
        route,
        host: host.to_owned(),
        connected_ip: response
            .remote_addr()
            .map(|address| address.ip().to_string()),
        candidate_address_count,
        http_status: response.status().as_u16(),
        latency_ms: elapsed_millis(started),
        dns_source: dns_source.into(),
        tls_summary: tls_summary.into(),
        ech_status: ProbeEchStatus::NotApplicable,
    }
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn is_probe_host(host: &str) -> bool {
    matches!(
        host.trim_end_matches('.').to_ascii_lowercase().as_str(),
        "app-api.pixiv.net"
            | "oauth.secure.pixiv.net"
            | "accounts.pixiv.net"
            | "www.pixiv.net"
            | "i.pximg.net"
            | "s.pximg.net"
    )
}

pub(crate) fn compatible_socket_address(host: &str, port: u16) -> Option<SocketAddr> {
    let octets = match host.trim_end_matches('.').to_ascii_lowercase().as_str() {
        "app-api.pixiv.net" | "oauth.secure.pixiv.net" | "accounts.pixiv.net" | "www.pixiv.net" => {
            [210, 140, 139, 155]
        }
        "i.pximg.net" | "s.pximg.net" => [210, 140, 139, 133],
        _ => return None,
    };
    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), port))
}

fn compatible_address(host: &str) -> Option<SocketAddr> {
    compatible_socket_address(host, 443)
}

impl From<PolicyError> for ProbeError {
    fn from(error: PolicyError) -> Self {
        match error {
            PolicyError::InvalidHost { host } => Self::InvalidHost { host },
            PolicyError::EchUnavailable { host } => Self::EchUnavailable { host },
            PolicyError::CompatibleDirectUnavailable { host } => {
                Self::CompatibleDirectUnavailable { host }
            }
            PolicyError::InsecureTransportForbidden { host } => {
                Self::InsecureTransportForbidden { host }
            }
            PolicyError::WebViewProxyUnavailable { host } => Self::WebViewProxyUnavailable { host },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{compatible_address, NetworkGateway, ProbeError, ProbeRequest};
    use pixiv_client_domain::{ConnectionMode, TrafficClass};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn unsafe_transport_requires_explicit_acknowledgement_before_network_io() {
        let error = NetworkGateway::default()
            .probe(&ProbeRequest {
                mode: ConnectionMode::Compatible,
                traffic: TrafficClass::Api,
                host: "app-api.pixiv.net".into(),
                unsafe_acknowledged: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            ProbeError::UnsafeAcknowledgementRequired {
                host: "app-api.pixiv.net".into()
            }
        );
    }

    #[test]
    fn compatible_ip_table_is_closed_to_unknown_hosts() {
        assert_eq!(
            compatible_address("app-api.pixiv.net").unwrap().ip(),
            IpAddr::V4(Ipv4Addr::new(210, 140, 139, 155))
        );
        assert_eq!(
            compatible_address("i.pximg.net").unwrap().ip(),
            IpAddr::V4(Ipv4Addr::new(210, 140, 139, 133))
        );
        assert!(compatible_address("example.com").is_none());
    }

    #[test]
    fn compatible_oauth_client_requires_explicit_acknowledgement() {
        let gateway = NetworkGateway::default();
        let request = ProbeRequest {
            mode: ConnectionMode::Compatible,
            traffic: TrafficClass::OAuth,
            host: "oauth.secure.pixiv.net".into(),
            unsafe_acknowledged: false,
        };

        assert_eq!(
            gateway.build_client(&request).unwrap_err(),
            ProbeError::UnsafeAcknowledgementRequired {
                host: "oauth.secure.pixiv.net".into(),
            }
        );

        let acknowledged = ProbeRequest {
            unsafe_acknowledged: true,
            ..request
        };
        assert!(gateway.build_client(&acknowledged).is_ok());
    }
}
