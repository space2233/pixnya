use pixiv_client_domain::{
    ConnectionMode, EchRequirement, PlatformCapabilities, PolicyError, RouteRequest, TrafficClass,
    TransportRoute, TransportSecurity,
};
use pixiv_client_network::ConnectionPolicy;

#[test]
fn standard_mode_uses_system_transport_and_keeps_certificate_host() {
    let request = RouteRequest {
        mode: ConnectionMode::Standard,
        traffic: TrafficClass::Api,
        host: "app-api.pixiv.net".into(),
        capabilities: PlatformCapabilities::default(),
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::System);
    assert_eq!(plan.certificate_host, "app-api.pixiv.net");
    assert_eq!(plan.ech_requirement, EchRequirement::NotApplicable);
    assert_eq!(plan.security, TransportSecurity::SystemTls);
    assert!(!plan.requires_user_acknowledgement);
}

#[test]
fn ech_mode_requires_an_accepted_ech_handshake_for_rust_api_requests() {
    let request = RouteRequest {
        mode: ConnectionMode::Ech,
        traffic: TrafficClass::Api,
        host: "app-api.pixiv.net".into(),
        capabilities: PlatformCapabilities {
            rust_ech: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::Ech);
    assert_eq!(plan.certificate_host, "app-api.pixiv.net");
    assert_eq!(plan.ech_requirement, EchRequirement::Accepted);
    assert_eq!(plan.security, TransportSecurity::EchVerified);
    assert!(!plan.requires_user_acknowledgement);
}

#[test]
fn ech_mode_refuses_to_silently_fall_back_for_rust_api_requests() {
    let request = RouteRequest {
        mode: ConnectionMode::Ech,
        traffic: TrafficClass::Api,
        host: "app-api.pixiv.net".into(),
        capabilities: PlatformCapabilities::default(),
    };

    let error = ConnectionPolicy.evaluate(&request).unwrap_err();

    assert_eq!(
        error,
        PolicyError::EchUnavailable {
            host: "app-api.pixiv.net".into(),
        }
    );
}

#[test]
fn ech_mode_keeps_pixiv_media_on_the_verified_ech_route() {
    let route = ConnectionPolicy
        .evaluate(&RouteRequest {
            mode: ConnectionMode::Ech,
            traffic: TrafficClass::Media,
            host: "i.pximg.net".into(),
            capabilities: PlatformCapabilities {
                rust_ech: true,
                ..PlatformCapabilities::default()
            },
        })
        .unwrap();
    assert_eq!(route.transport, TransportRoute::Ech);
    assert_eq!(route.ech_requirement, EchRequirement::Accepted);
    assert_eq!(route.security, TransportSecurity::EchVerified);
}

#[test]
fn compatible_mode_marks_pixiv_media_as_insecure_and_requires_acknowledgement() {
    let request = RouteRequest {
        mode: ConnectionMode::Compatible,
        traffic: TrafficClass::Media,
        host: "i.pximg.net".into(),
        capabilities: PlatformCapabilities {
            rust_compatible_direct: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::CompatibleDirect);
    assert_eq!(plan.certificate_host, "i.pximg.net");
    assert_eq!(plan.ech_requirement, EchRequirement::NotApplicable);
    assert_eq!(plan.security, TransportSecurity::Insecure);
    assert!(plan.requires_user_acknowledgement);
}

#[test]
fn compatible_oauth_is_explicitly_marked_insecure() {
    let request = RouteRequest {
        mode: ConnectionMode::Compatible,
        traffic: TrafficClass::OAuth,
        host: "oauth.secure.pixiv.net".into(),
        capabilities: PlatformCapabilities {
            rust_compatible_direct: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::CompatibleDirect);
    assert_eq!(plan.security, TransportSecurity::Insecure);
    assert!(plan.requires_user_acknowledgement);
}

#[test]
fn third_party_login_hosts_never_inherit_pixiv_compatible_routing() {
    let request = RouteRequest {
        mode: ConnectionMode::Compatible,
        traffic: TrafficClass::LoginWebView,
        host: "accounts.google.com".into(),
        capabilities: PlatformCapabilities {
            rust_compatible_direct: true,
            webview_proxy: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::WebViewSystem);
    assert_eq!(plan.certificate_host, "accounts.google.com");
    assert_eq!(plan.ech_requirement, EchRequirement::NotApplicable);
    assert_eq!(plan.security, TransportSecurity::SystemTls);
    assert!(!plan.requires_user_acknowledgement);
}

#[test]
fn compatible_pixiv_login_uses_the_platform_webview_proxy() {
    let request = RouteRequest {
        mode: ConnectionMode::Compatible,
        traffic: TrafficClass::LoginWebView,
        host: "accounts.pixiv.net".into(),
        capabilities: PlatformCapabilities {
            webview_proxy: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::WebViewProxy);
    assert_eq!(plan.certificate_host, "accounts.pixiv.net");
    assert_eq!(plan.ech_requirement, EchRequirement::NotApplicable);
}

#[test]
fn android_ech_login_never_uses_the_insecure_bridge() {
    let request = RouteRequest {
        mode: ConnectionMode::Ech,
        traffic: TrafficClass::LoginWebView,
        host: "app-api.pixiv.net".into(),
        capabilities: PlatformCapabilities {
            rust_ech: true,
            webview_proxy: true,
            webview_insecure_bridge: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::WebViewSystem);
    assert_eq!(plan.ech_requirement, EchRequirement::PlatformManaged);
    assert_eq!(plan.security, TransportSecurity::SystemTls);
    assert!(!plan.requires_user_acknowledgement);
}

#[test]
fn android_compatible_login_remains_an_explicit_insecure_bridge() {
    let request = RouteRequest {
        mode: ConnectionMode::Compatible,
        traffic: TrafficClass::LoginWebView,
        host: "app-api.pixiv.net".into(),
        capabilities: PlatformCapabilities {
            rust_ech: true,
            webview_proxy: true,
            webview_insecure_bridge: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::WebViewInsecureBridge);
    assert_eq!(plan.ech_requirement, EchRequirement::NotApplicable);
    assert_eq!(plan.security, TransportSecurity::Insecure);
    assert!(plan.requires_user_acknowledgement);
}

#[test]
fn compatible_mode_reports_missing_platform_adapters() {
    let cases = [
        (
            TrafficClass::Media,
            "i.pximg.net",
            PolicyError::CompatibleDirectUnavailable {
                host: "i.pximg.net".into(),
            },
        ),
        (
            TrafficClass::LoginWebView,
            "accounts.pixiv.net",
            PolicyError::WebViewProxyUnavailable {
                host: "accounts.pixiv.net".into(),
            },
        ),
    ];

    for (traffic, host, expected) in cases {
        let request = RouteRequest {
            mode: ConnectionMode::Compatible,
            traffic,
            host: host.into(),
            capabilities: PlatformCapabilities::default(),
        };

        assert_eq!(ConnectionPolicy.evaluate(&request), Err(expected));
    }
}

#[test]
fn platform_webview_owns_standard_and_ech_login_tls() {
    let cases = [
        (ConnectionMode::Standard, EchRequirement::NotApplicable),
        (ConnectionMode::Ech, EchRequirement::PlatformManaged),
    ];

    for (mode, expected_ech_requirement) in cases {
        let request = RouteRequest {
            mode,
            traffic: TrafficClass::LoginWebView,
            host: "accounts.pixiv.net".into(),
            capabilities: PlatformCapabilities {
                rust_ech: true,
                ..PlatformCapabilities::default()
            },
        };

        let plan = ConnectionPolicy.evaluate(&request).unwrap();

        assert_eq!(plan.transport, TransportRoute::WebViewSystem);
        assert_eq!(plan.ech_requirement, expected_ech_requirement);
    }
}

#[test]
fn unknown_hosts_do_not_inherit_pixiv_compatible_routes() {
    let request = RouteRequest {
        mode: ConnectionMode::Compatible,
        traffic: TrafficClass::Api,
        host: "example.com".into(),
        capabilities: PlatformCapabilities {
            rust_compatible_direct: true,
            ..PlatformCapabilities::default()
        },
    };

    let plan = ConnectionPolicy.evaluate(&request).unwrap();

    assert_eq!(plan.transport, TransportRoute::System);
    assert_eq!(plan.certificate_host, "example.com");
    assert_eq!(plan.ech_requirement, EchRequirement::NotApplicable);
    assert_eq!(plan.security, TransportSecurity::SystemTls);
    assert!(!plan.requires_user_acknowledgement);
}
