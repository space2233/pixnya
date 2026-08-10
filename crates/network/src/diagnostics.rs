use crate::{ConnectionPolicy, ConnectionProbe, NetworkGateway, ProbeError, ProbeRequest};
use pixiv_client_domain::{
    ConnectionMode, PlatformCapabilities, RoutePlan, RouteRequest, TrafficClass,
};
use serde::Serialize;
use std::net::IpAddr;

const API_HOST: &str = "app-api.pixiv.net";
const MEDIA_HOST: &str = "i.pximg.net";
const LOGIN_HOST: &str = "app-api.pixiv.net";

#[derive(Debug, Clone, Copy)]
pub struct DiagnosticContext<'a> {
    pub application_version: &'a str,
    pub platform: &'a str,
    pub architecture: &'a str,
    pub webview_proxy_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticTarget {
    Api,
    Media,
    Login,
}

impl DiagnosticTarget {
    fn host(self) -> &'static str {
        match self {
            Self::Api | Self::Login => API_HOST,
            Self::Media => MEDIA_HOST,
        }
    }

    fn traffic(self) -> TrafficClass {
        match self {
            Self::Api => TrafficClass::Api,
            Self::Media => TrafficClass::Media,
            Self::Login => TrafficClass::LoginWebView,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Reachable,
    Unreachable,
    PlatformRouteReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticFailureKind {
    InvalidHost,
    UnsafeAcknowledgementRequired,
    EchUnavailable,
    CompatibleDirectUnavailable,
    InsecureTransportForbidden,
    WebViewProxyUnavailable,
    WebViewTransportUnavailable,
    DnsQueryFailed,
    EchConfigUnavailable,
    EchNotAccepted,
    ConnectionFailed,
    HttpProtocolError,
}

impl From<&ProbeError> for DiagnosticFailureKind {
    fn from(error: &ProbeError) -> Self {
        match error {
            ProbeError::InvalidHost { .. } => Self::InvalidHost,
            ProbeError::UnsafeAcknowledgementRequired { .. } => Self::UnsafeAcknowledgementRequired,
            ProbeError::EchUnavailable { .. } => Self::EchUnavailable,
            ProbeError::CompatibleDirectUnavailable { .. } => Self::CompatibleDirectUnavailable,
            ProbeError::InsecureTransportForbidden { .. } => Self::InsecureTransportForbidden,
            ProbeError::WebViewProxyUnavailable { .. } => Self::WebViewProxyUnavailable,
            ProbeError::WebViewTransportUnavailable { .. } => Self::WebViewTransportUnavailable,
            ProbeError::DnsQueryFailed { .. } => Self::DnsQueryFailed,
            ProbeError::EchConfigUnavailable { .. } => Self::EchConfigUnavailable,
            ProbeError::EchNotAccepted { .. } => Self::EchNotAccepted,
            ProbeError::ConnectionFailed { .. } => Self::ConnectionFailed,
            ProbeError::HttpProtocolError { .. } => Self::HttpProtocolError,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub target: DiagnosticTarget,
    pub host: &'static str,
    pub status: DiagnosticStatus,
    pub route: Option<RoutePlan>,
    pub connected_ip: Option<String>,
    pub candidate_address_count: Option<u16>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub failure: Option<DiagnosticFailureKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDiagnosticReport {
    pub schema_version: u8,
    pub application_version: String,
    pub platform: String,
    pub architecture: String,
    pub mode: ConnectionMode,
    pub capabilities: PlatformCapabilities,
    pub webview_proxy_active: bool,
    pub checks: Vec<DiagnosticCheck>,
}

impl NetworkGateway {
    pub fn diagnose(
        &self,
        mode: ConnectionMode,
        unsafe_acknowledged: bool,
        context: DiagnosticContext<'_>,
    ) -> ConnectionDiagnosticReport {
        let capabilities = Self::capabilities();
        let api_request = diagnostic_request(DiagnosticTarget::Api, mode, unsafe_acknowledged);
        let media_request = diagnostic_request(DiagnosticTarget::Media, mode, unsafe_acknowledged);
        let (api_result, media_result) = std::thread::scope(|scope| {
            let api = scope.spawn(|| self.probe(&api_request));
            let media = scope.spawn(|| self.probe(&media_request));
            (
                api.join().unwrap_or_else(|_| {
                    Err(ProbeError::ConnectionFailed {
                        host: API_HOST.into(),
                    })
                }),
                media.join().unwrap_or_else(|_| {
                    Err(ProbeError::ConnectionFailed {
                        host: MEDIA_HOST.into(),
                    })
                }),
            )
        });
        let api = diagnostic_probe_result(DiagnosticTarget::Api, mode, capabilities, api_result);
        let media =
            diagnostic_probe_result(DiagnosticTarget::Media, mode, capabilities, media_result);
        let login = diagnostic_login(mode, capabilities);

        ConnectionDiagnosticReport::new(context, mode, capabilities, vec![api, media, login])
    }

    pub fn diagnose_with<F>(
        &self,
        mode: ConnectionMode,
        unsafe_acknowledged: bool,
        context: DiagnosticContext<'_>,
        mut probe: F,
    ) -> ConnectionDiagnosticReport
    where
        F: FnMut(&ProbeRequest) -> Result<ConnectionProbe, ProbeError>,
    {
        let capabilities = Self::capabilities();
        let api = diagnostic_probe(
            DiagnosticTarget::Api,
            mode,
            unsafe_acknowledged,
            capabilities,
            &mut probe,
        );
        let media = diagnostic_probe(
            DiagnosticTarget::Media,
            mode,
            unsafe_acknowledged,
            capabilities,
            &mut probe,
        );
        let login = diagnostic_login(mode, capabilities);

        ConnectionDiagnosticReport::new(context, mode, capabilities, vec![api, media, login])
    }
}

impl ConnectionDiagnosticReport {
    fn new(
        context: DiagnosticContext<'_>,
        mode: ConnectionMode,
        capabilities: PlatformCapabilities,
        checks: Vec<DiagnosticCheck>,
    ) -> Self {
        Self {
            schema_version: 1,
            application_version: safe_metadata(context.application_version),
            platform: safe_metadata(context.platform),
            architecture: safe_metadata(context.architecture),
            mode,
            capabilities,
            webview_proxy_active: context.webview_proxy_active,
            checks,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        mode: ConnectionMode,
        platform: &str,
        architecture: &str,
        webview_proxy_active: bool,
    ) -> Self {
        let capabilities = NetworkGateway::capabilities();
        let checks = [
            DiagnosticTarget::Api,
            DiagnosticTarget::Media,
            DiagnosticTarget::Login,
        ]
        .into_iter()
        .map(|target| DiagnosticCheck {
            target,
            host: target.host(),
            status: if target == DiagnosticTarget::Login {
                DiagnosticStatus::PlatformRouteReady
            } else {
                DiagnosticStatus::Unreachable
            },
            route: None,
            connected_ip: None,
            candidate_address_count: None,
            http_status: None,
            latency_ms: None,
            failure: None,
        })
        .collect();
        Self::new(
            DiagnosticContext {
                application_version: "0.9.0",
                platform,
                architecture,
                webview_proxy_active,
            },
            mode,
            capabilities,
            checks,
        )
    }
}

fn diagnostic_probe<F>(
    target: DiagnosticTarget,
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
    capabilities: PlatformCapabilities,
    probe: &mut F,
) -> DiagnosticCheck
where
    F: FnMut(&ProbeRequest) -> Result<ConnectionProbe, ProbeError>,
{
    let request = diagnostic_request(target, mode, unsafe_acknowledged);
    let result = probe(&request);
    diagnostic_probe_result(target, mode, capabilities, result)
}

fn diagnostic_request(
    target: DiagnosticTarget,
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
) -> ProbeRequest {
    ProbeRequest {
        mode,
        traffic: target.traffic(),
        host: target.host().to_owned(),
        unsafe_acknowledged,
    }
}

fn diagnostic_probe_result(
    target: DiagnosticTarget,
    mode: ConnectionMode,
    capabilities: PlatformCapabilities,
    result: Result<ConnectionProbe, ProbeError>,
) -> DiagnosticCheck {
    let planned_route = ConnectionPolicy
        .evaluate(&RouteRequest {
            mode,
            traffic: target.traffic(),
            host: target.host().to_owned(),
            capabilities,
        })
        .ok();

    match result {
        Ok(result) => DiagnosticCheck {
            target,
            host: target.host(),
            status: DiagnosticStatus::Reachable,
            route: Some(result.route),
            connected_ip: safe_ip(result.connected_ip),
            candidate_address_count: result.candidate_address_count,
            http_status: Some(result.http_status),
            latency_ms: Some(result.latency_ms),
            failure: None,
        },
        Err(error) => DiagnosticCheck {
            target,
            host: target.host(),
            status: DiagnosticStatus::Unreachable,
            route: planned_route,
            connected_ip: None,
            candidate_address_count: None,
            http_status: None,
            latency_ms: None,
            failure: Some((&error).into()),
        },
    }
}

fn diagnostic_login(mode: ConnectionMode, capabilities: PlatformCapabilities) -> DiagnosticCheck {
    match ConnectionPolicy.evaluate(&RouteRequest {
        mode,
        traffic: TrafficClass::LoginWebView,
        host: LOGIN_HOST.to_owned(),
        capabilities,
    }) {
        Ok(route) => DiagnosticCheck {
            target: DiagnosticTarget::Login,
            host: LOGIN_HOST,
            status: DiagnosticStatus::PlatformRouteReady,
            route: Some(route),
            connected_ip: None,
            candidate_address_count: None,
            http_status: None,
            latency_ms: None,
            failure: None,
        },
        Err(error) => DiagnosticCheck {
            target: DiagnosticTarget::Login,
            host: LOGIN_HOST,
            status: DiagnosticStatus::Unreachable,
            route: None,
            connected_ip: None,
            candidate_address_count: None,
            http_status: None,
            latency_ms: None,
            failure: Some(match error {
                pixiv_client_domain::PolicyError::InvalidHost { .. } => {
                    DiagnosticFailureKind::InvalidHost
                }
                pixiv_client_domain::PolicyError::EchUnavailable { .. } => {
                    DiagnosticFailureKind::EchUnavailable
                }
                pixiv_client_domain::PolicyError::CompatibleDirectUnavailable { .. } => {
                    DiagnosticFailureKind::CompatibleDirectUnavailable
                }
                pixiv_client_domain::PolicyError::InsecureTransportForbidden { .. } => {
                    DiagnosticFailureKind::InsecureTransportForbidden
                }
                pixiv_client_domain::PolicyError::WebViewProxyUnavailable { .. } => {
                    DiagnosticFailureKind::WebViewProxyUnavailable
                }
            }),
        },
    }
}

fn safe_metadata(value: &str) -> String {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        "unavailable".to_owned()
    } else {
        value.to_owned()
    }
}

fn safe_ip(value: Option<String>) -> Option<String> {
    value?
        .parse::<IpAddr>()
        .ok()
        .map(|address| address.to_string())
}
