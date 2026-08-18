use pixiv_client_domain::{ConnectionMode, TrafficClass};
use pixiv_client_network::{
    LoginProxy, LoginProxyMode, NetworkGateway, ProbeEchStatus, ProbeRequest,
};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Proxy;
use std::time::Duration;

#[test]
#[ignore = "requires live network access"]
fn live_ech_probe_requires_an_accepted_handshake() {
    let report = NetworkGateway::default()
        .probe(&ProbeRequest {
            mode: ConnectionMode::Ech,
            traffic: TrafficClass::Api,
            host: "app-api.pixiv.net".into(),
            unsafe_acknowledged: false,
        })
        .unwrap();

    assert_eq!(report.ech_status, ProbeEchStatus::Accepted);
}

#[test]
#[ignore = "requires live network access"]
fn live_ech_probe_reaches_both_api_and_media_hosts() {
    let gateway = NetworkGateway::default();
    for (traffic, host) in [
        (TrafficClass::Api, "app-api.pixiv.net"),
        (TrafficClass::Media, "i.pximg.net"),
        (TrafficClass::Media, "s.pximg.net"),
    ] {
        let report = gateway
            .probe(&ProbeRequest {
                mode: ConnectionMode::Ech,
                traffic,
                host: host.into(),
                unsafe_acknowledged: false,
            })
            .unwrap_or_else(|error| panic!("ECH {traffic:?} probe failed for {host}: {error}"));

        assert_eq!(report.ech_status, ProbeEchStatus::Accepted);
    }
}

#[test]
#[ignore = "requires live network access"]
fn live_ech_reqwest_client_reaches_the_oauth_host_after_accepted_preflight() {
    let request = ProbeRequest {
        mode: ConnectionMode::Ech,
        traffic: TrafficClass::OAuth,
        host: "oauth.secure.pixiv.net".into(),
        unsafe_acknowledged: false,
    };
    let client = NetworkGateway::default().build_client(&request).unwrap();
    let response = client
        .get("https://oauth.secure.pixiv.net/")
        .send()
        .unwrap();

    assert!(response.status().as_u16() >= 100);
}

#[test]
#[ignore = "requires live network access and explicitly unsafe TLS"]
fn live_compatible_probe_runs_only_after_acknowledgement() {
    let report = NetworkGateway::default()
        .probe(&ProbeRequest {
            mode: ConnectionMode::Compatible,
            traffic: TrafficClass::Api,
            host: "app-api.pixiv.net".into(),
            unsafe_acknowledged: true,
        })
        .unwrap();

    assert!(report.http_status >= 100);
}

#[test]
#[ignore = "requires live network access and explicitly unsafe login TLS"]
fn live_insecure_login_bridge_reaches_the_official_page() {
    let proxy = LoginProxy::start(LoginProxyMode::InsecureTlsBridge).unwrap();
    let client = Client::builder()
        .proxy(Proxy::all(proxy.url()).unwrap())
        .tls_danger_accept_invalid_certs(true)
        .redirect(Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();

    let response = client
        .get("https://app-api.pixiv.net/web/v1/login")
        .send()
        .unwrap();

    assert!(response.status().as_u16() < 500);
    assert_eq!(proxy.certificate_sha256().unwrap().len(), 64);
}
