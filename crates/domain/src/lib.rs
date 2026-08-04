use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode {
    Standard,
    Ech,
    Compatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficClass {
    OAuth,
    Api,
    Media,
    LoginWebView,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub rust_ech: bool,
    pub rust_compatible_direct: bool,
    pub webview_proxy: bool,
    pub webview_insecure_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRequest {
    pub mode: ConnectionMode,
    pub traffic: TrafficClass,
    pub host: String,
    pub capabilities: PlatformCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportRoute {
    System,
    Ech,
    CompatibleDirect,
    WebViewSystem,
    WebViewProxy,
    WebViewInsecureBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EchRequirement {
    NotApplicable,
    Accepted,
    PlatformManaged,
    PreflightOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportSecurity {
    SystemTls,
    EchVerified,
    Insecure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlan {
    pub transport: TransportRoute,
    pub certificate_host: String,
    pub ech_requirement: EchRequirement,
    pub security: TransportSecurity,
    pub requires_user_acknowledgement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyError {
    InvalidHost { host: String },
    EchUnavailable { host: String },
    CompatibleDirectUnavailable { host: String },
    InsecureTransportForbidden { host: String },
    WebViewProxyUnavailable { host: String },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHost { host } => write!(formatter, "invalid request host: {host}"),
            Self::EchUnavailable { host } => {
                write!(formatter, "ECH is required but unavailable for {host}")
            }
            Self::CompatibleDirectUnavailable { host } => write!(
                formatter,
                "compatible direct transport is unavailable for {host}"
            ),
            Self::InsecureTransportForbidden { host } => write!(
                formatter,
                "insecure direct transport is forbidden for sensitive traffic to {host}"
            ),
            Self::WebViewProxyUnavailable { host } => {
                write!(
                    formatter,
                    "WebView proxy override is unavailable for {host}"
                )
            }
        }
    }
}

impl std::error::Error for PolicyError {}
