use pixiv_client_domain::{
    ConnectionMode, EchRequirement, PolicyError, RoutePlan, RouteRequest, TrafficClass,
    TransportRoute, TransportSecurity,
};

mod diagnostics;
mod ech;
mod gateway;
mod login_proxy;

pub use diagnostics::{
    ConnectionDiagnosticReport, DiagnosticCheck, DiagnosticContext, DiagnosticFailureKind,
    DiagnosticStatus, DiagnosticTarget,
};
pub use gateway::{ConnectionProbe, NetworkGateway, ProbeEchStatus, ProbeError, ProbeRequest};
pub use login_proxy::{LoginProxy, LoginProxyError, LoginProxyMode};

#[derive(Debug, Default)]
pub struct ConnectionPolicy;

impl ConnectionPolicy {
    pub fn evaluate(&self, request: &RouteRequest) -> Result<RoutePlan, PolicyError> {
        match request.mode {
            ConnectionMode::Standard if request.traffic == TrafficClass::LoginWebView => {
                Ok(RoutePlan {
                    transport: TransportRoute::WebViewSystem,
                    certificate_host: request.host.clone(),
                    ech_requirement: EchRequirement::NotApplicable,
                    security: TransportSecurity::SystemTls,
                    requires_user_acknowledgement: false,
                })
            }
            ConnectionMode::Standard => Ok(RoutePlan {
                transport: TransportRoute::System,
                certificate_host: request.host.clone(),
                ech_requirement: EchRequirement::NotApplicable,
                security: TransportSecurity::SystemTls,
                requires_user_acknowledgement: false,
            }),
            ConnectionMode::Ech if request.traffic == TrafficClass::LoginWebView => Ok(RoutePlan {
                transport: TransportRoute::WebViewSystem,
                certificate_host: request.host.clone(),
                ech_requirement: EchRequirement::PlatformManaged,
                security: TransportSecurity::SystemTls,
                requires_user_acknowledgement: false,
            }),
            ConnectionMode::Ech
                if request.traffic != TrafficClass::LoginWebView
                    && request.capabilities.rust_ech =>
            {
                Ok(RoutePlan {
                    transport: TransportRoute::Ech,
                    certificate_host: request.host.clone(),
                    ech_requirement: EchRequirement::Accepted,
                    security: TransportSecurity::EchVerified,
                    requires_user_acknowledgement: false,
                })
            }
            ConnectionMode::Ech => Err(PolicyError::EchUnavailable {
                host: request.host.clone(),
            }),
            ConnectionMode::Compatible
                if request.traffic != TrafficClass::LoginWebView
                    && request.capabilities.rust_compatible_direct
                    && is_pixiv_host(&request.host) =>
            {
                Ok(RoutePlan {
                    transport: TransportRoute::CompatibleDirect,
                    certificate_host: request.host.clone(),
                    ech_requirement: EchRequirement::NotApplicable,
                    security: TransportSecurity::Insecure,
                    requires_user_acknowledgement: true,
                })
            }
            ConnectionMode::Compatible
                if request.traffic == TrafficClass::LoginWebView
                    && !is_pixiv_host(&request.host) =>
            {
                Ok(RoutePlan {
                    transport: TransportRoute::WebViewSystem,
                    certificate_host: request.host.clone(),
                    ech_requirement: EchRequirement::NotApplicable,
                    security: TransportSecurity::SystemTls,
                    requires_user_acknowledgement: false,
                })
            }
            ConnectionMode::Compatible
                if request.traffic == TrafficClass::LoginWebView
                    && request.capabilities.webview_insecure_bridge
                    && is_pixiv_host(&request.host) =>
            {
                Ok(RoutePlan {
                    transport: TransportRoute::WebViewInsecureBridge,
                    certificate_host: request.host.clone(),
                    ech_requirement: EchRequirement::NotApplicable,
                    security: TransportSecurity::Insecure,
                    requires_user_acknowledgement: true,
                })
            }
            ConnectionMode::Compatible
                if request.traffic == TrafficClass::LoginWebView
                    && request.capabilities.webview_proxy
                    && is_pixiv_host(&request.host) =>
            {
                Ok(RoutePlan {
                    transport: TransportRoute::WebViewProxy,
                    certificate_host: request.host.clone(),
                    ech_requirement: EchRequirement::NotApplicable,
                    security: TransportSecurity::SystemTls,
                    requires_user_acknowledgement: false,
                })
            }
            ConnectionMode::Compatible
                if request.traffic == TrafficClass::LoginWebView
                    && is_pixiv_host(&request.host) =>
            {
                Err(PolicyError::WebViewProxyUnavailable {
                    host: request.host.clone(),
                })
            }
            ConnectionMode::Compatible if is_pixiv_host(&request.host) => {
                Err(PolicyError::CompatibleDirectUnavailable {
                    host: request.host.clone(),
                })
            }
            ConnectionMode::Compatible => Ok(RoutePlan {
                transport: TransportRoute::System,
                certificate_host: request.host.clone(),
                ech_requirement: EchRequirement::NotApplicable,
                security: TransportSecurity::SystemTls,
                requires_user_acknowledgement: false,
            }),
        }
    }
}

pub fn is_pixiv_host(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    ["pixiv.net", "pximg.net"]
        .iter()
        .any(|root| host.eq_ignore_ascii_case(root) || host.ends_with(&format!(".{root}")))
}

#[cfg(test)]
mod diagnostic_contract_tests {
    use super::{
        ConnectionDiagnosticReport, DiagnosticContext, DiagnosticTarget, NetworkGateway, ProbeError,
    };
    use pixiv_client_domain::ConnectionMode;

    #[test]
    fn diagnostics_cover_api_media_and_login_without_accepting_sensitive_text() {
        let report = NetworkGateway::default().diagnose_with(
            ConnectionMode::Standard,
            false,
            DiagnosticContext {
                application_version: "0.9.0\nAuthorization: Bearer secret",
                platform: "windows\nCookie: session=secret",
                architecture: "x86_64",
                webview_proxy_active: false,
            },
            |request| {
                Err(ProbeError::ConnectionFailed {
                    host: request.host.clone(),
                })
            },
        );

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.checks.len(), 3);
        assert_eq!(report.checks[0].target, DiagnosticTarget::Api);
        assert_eq!(report.checks[1].target, DiagnosticTarget::Media);
        assert_eq!(report.checks[2].target, DiagnosticTarget::Login);
        assert_eq!(report.application_version, "unavailable");
        assert_eq!(report.platform, "unavailable");
    }

    #[test]
    fn report_contains_only_safe_structured_fields() {
        let report =
            ConnectionDiagnosticReport::for_test(ConnectionMode::Ech, "android", "aarch64", false);

        assert_eq!(report.application_version, "0.9.0");
        assert_eq!(report.platform, "android");
        assert_eq!(report.architecture, "aarch64");
        assert_eq!(report.checks.len(), 3);
    }
}
