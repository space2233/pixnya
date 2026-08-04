use pixiv_client_domain::{
    ConnectionMode, PlatformCapabilities, PolicyError, RoutePlan, RouteRequest, TrafficClass,
};
use pixiv_client_network::ConnectionPolicy;

#[derive(Debug)]
pub(crate) enum LoginRouteError {
    Policy(PolicyError),
    UnsafeAcknowledgementRequired { host: String },
}

pub(crate) fn evaluate_login_route(
    mode: ConnectionMode,
    capabilities: PlatformCapabilities,
    unsafe_acknowledged: bool,
    host: &str,
) -> Result<RoutePlan, LoginRouteError> {
    if mode == ConnectionMode::Compatible && !unsafe_acknowledged {
        return Err(LoginRouteError::UnsafeAcknowledgementRequired {
            host: host.to_owned(),
        });
    }

    let route = ConnectionPolicy
        .evaluate(&RouteRequest {
            mode,
            traffic: TrafficClass::LoginWebView,
            host: host.to_owned(),
            capabilities,
        })
        .map_err(LoginRouteError::Policy)?;

    if route.requires_user_acknowledgement && !unsafe_acknowledged {
        return Err(LoginRouteError::UnsafeAcknowledgementRequired {
            host: host.to_owned(),
        });
    }
    Ok(route)
}

#[cfg(test)]
mod tests {
    use super::{evaluate_login_route, LoginRouteError};
    use pixiv_client_domain::{ConnectionMode, PlatformCapabilities, TransportRoute};

    const LOGIN_HOST: &str = "app-api.pixiv.net";

    #[test]
    fn compatible_mode_requires_acknowledgement_before_policy_evaluation() {
        let result = evaluate_login_route(
            ConnectionMode::Compatible,
            PlatformCapabilities::default(),
            false,
            LOGIN_HOST,
        );

        assert!(matches!(
            result,
            Err(LoginRouteError::UnsafeAcknowledgementRequired { host })
                if host == LOGIN_HOST
        ));
    }

    #[test]
    fn standard_mode_returns_the_system_webview_route() {
        let route = evaluate_login_route(
            ConnectionMode::Standard,
            PlatformCapabilities::default(),
            false,
            LOGIN_HOST,
        )
        .unwrap();

        assert_eq!(route.transport, TransportRoute::WebViewSystem);
    }
}
