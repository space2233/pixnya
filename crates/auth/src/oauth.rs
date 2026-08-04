use crate::AuthorizationGrant;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use time::macros::format_description;
use time::OffsetDateTime;
use zeroize::Zeroizing;

const TOKEN_URL: &str = "https://oauth.secure.pixiv.net/auth/token";
const TOKEN_ORIGIN_URL: &str = "https://oauth.secure.pixiv.net/";
const REDIRECT_URI: &str = "https://app-api.pixiv.net/web/v1/users/auth/pixiv/callback";
const APP_VERSION: &str = "5.0.166";
const USER_AGENT: &str = "PixivAndroidApp/5.0.166 (Android 13; PixivClient)";

pub struct OAuthClientConfig {
    client_id: Zeroizing<String>,
    client_secret: Zeroizing<String>,
    hash_salt: Zeroizing<String>,
}

impl OAuthClientConfig {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        hash_salt: impl Into<String>,
    ) -> Result<Self, OAuthError> {
        let client_id = Zeroizing::new(client_id.into());
        let client_secret = Zeroizing::new(client_secret.into());
        let hash_salt = Zeroizing::new(hash_salt.into());
        if client_id.trim().is_empty()
            || client_secret.trim().is_empty()
            || hash_salt.trim().is_empty()
        {
            return Err(OAuthError::ConfigurationUnavailable);
        }
        Ok(Self {
            client_id,
            client_secret,
            hash_salt,
        })
    }

    pub fn client_request_signature(&self) -> Result<ClientRequestSignature, OAuthError> {
        let client_time = current_client_time()?;
        let client_hash = Zeroizing::new(format!(
            "{:x}",
            md5::compute(format!("{}{}", client_time, self.hash_salt.as_str()))
        ));
        Ok(ClientRequestSignature {
            client_time,
            client_hash,
        })
    }
}

pub struct ClientRequestSignature {
    client_time: String,
    client_hash: Zeroizing<String>,
}

impl ClientRequestSignature {
    pub fn client_time(&self) -> &str {
        &self.client_time
    }

    pub fn client_hash(&self) -> &str {
        self.client_hash.as_str()
    }
}

pub struct OAuthClient {
    http: Client,
    config: OAuthClientConfig,
}

impl OAuthClient {
    pub fn new(config: OAuthClientConfig) -> Result<Self, OAuthError> {
        let http = Client::builder()
            .tls_backend_rustls()
            .https_only(true)
            .timeout(Duration::from_secs(20))
            .redirect(Policy::none())
            .build()
            .map_err(|_| OAuthError::ClientUnavailable)?;
        Ok(Self { http, config })
    }

    pub fn with_http(config: OAuthClientConfig, http: Client) -> Self {
        Self { http, config }
    }

    /// Opens an anonymous keep-alive connection while the user is still authorizing.
    /// Failure is intentionally ignored by the caller because the real token request
    /// remains the source of truth and can retry a transient network failure.
    pub fn warm_transport(&self) {
        if let Ok(response) = self
            .http
            .get(TOKEN_ORIGIN_URL)
            .header("User-Agent", USER_AGENT)
            .send()
        {
            let _ = response.bytes();
        }
    }

    pub fn exchange_authorization_code(
        &self,
        grant: &AuthorizationGrant,
    ) -> Result<TokenSet, OAuthError> {
        self.request_token(&[
            ("code", grant.code()),
            ("redirect_uri", REDIRECT_URI),
            ("grant_type", "authorization_code"),
            ("include_policy", "true"),
            ("client_id", self.config.client_id.as_str()),
            ("code_verifier", grant.code_verifier()),
            ("client_secret", self.config.client_secret.as_str()),
        ])
    }

    pub fn refresh(&self, refresh_token: &str) -> Result<TokenSet, OAuthError> {
        if refresh_token.is_empty() {
            return Err(OAuthError::InvalidResponse);
        }
        self.request_token(&[
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("include_policy", "true"),
        ])
    }

    fn request_token(&self, form: &[(&str, &str)]) -> Result<TokenSet, OAuthError> {
        let signature = self.config.client_request_signature()?;
        let response = self
            .http
            .post(TOKEN_URL)
            .header("X-Client-Time", signature.client_time())
            .header("X-Client-Hash", signature.client_hash())
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "zh-CN")
            .header("App-OS", "Android")
            .header("App-OS-Version", "Android 13")
            .header("App-Version", APP_VERSION)
            .form(form)
            .send()
            .map_err(|_| OAuthError::RequestFailed)?;
        let status = response.status();
        if !status.is_success() {
            return Err(OAuthError::Rejected {
                http_status: status.as_u16(),
            });
        }
        let envelope: TokenEnvelope = response.json().map_err(|_| OAuthError::InvalidResponse)?;
        TokenSet::try_from(envelope.into_payload())
    }
}

fn current_client_time() -> Result<String, OAuthError> {
    OffsetDateTime::now_utc()
        .format(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]+00:00"
        ))
        .map_err(|_| OAuthError::ClockUnavailable)
}

pub struct TokenSet {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_in: u64,
    user: AuthenticatedUser,
}

impl TokenSet {
    fn try_from(payload: TokenPayload) -> Result<Self, OAuthError> {
        if payload.access_token.is_empty()
            || payload.refresh_token.is_empty()
            || payload.expires_in == 0
            || payload.user.id.is_empty()
        {
            return Err(OAuthError::InvalidResponse);
        }
        Ok(Self {
            access_token: Zeroizing::new(payload.access_token),
            refresh_token: Zeroizing::new(payload.refresh_token),
            expires_in: payload.expires_in,
            user: payload.user.into(),
        })
    }

    pub fn into_parts(self) -> (Zeroizing<String>, Zeroizing<String>, u64, AuthenticatedUser) {
        (
            self.access_token,
            self.refresh_token,
            self.expires_in,
            self.user,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    pub id: String,
    pub name: String,
    pub account: String,
    pub avatar_url: Option<String>,
    pub is_premium: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProfileImageUrls {
    #[serde(default)]
    pub px_16x16: Option<String>,
    #[serde(default)]
    pub px_50x50: Option<String>,
    #[serde(default)]
    pub px_170x170: Option<String>,
    #[serde(default)]
    pub medium: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TokenEnvelope {
    Wrapped { response: TokenPayload },
    Direct(TokenPayload),
}

impl TokenEnvelope {
    fn into_payload(self) -> TokenPayload {
        match self {
            Self::Wrapped { response } => response,
            Self::Direct(payload) => payload,
        }
    }
}

#[derive(Deserialize)]
struct TokenPayload {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    user: UserPayload,
}

#[derive(Deserialize)]
struct UserPayload {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    account: String,
    #[serde(default)]
    profile_image_urls: ProfileImageUrls,
    #[serde(default)]
    is_premium: bool,
}

impl From<UserPayload> for AuthenticatedUser {
    fn from(user: UserPayload) -> Self {
        let avatar_url = user
            .profile_image_urls
            .px_170x170
            .or(user.profile_image_urls.medium)
            .or(user.profile_image_urls.px_50x50)
            .or(user.profile_image_urls.px_16x16);
        Self {
            id: user.id,
            name: user.name,
            account: user.account,
            avatar_url,
            is_premium: user.is_premium,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthError {
    ConfigurationUnavailable,
    ClientUnavailable,
    ClockUnavailable,
    RequestFailed,
    Rejected { http_status: u16 },
    InvalidResponse,
}

impl fmt::Display for OAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigurationUnavailable => {
                formatter.write_str("OAuth configuration unavailable")
            }
            Self::ClientUnavailable => formatter.write_str("verified TLS client unavailable"),
            Self::ClockUnavailable => formatter.write_str("system clock unavailable"),
            Self::RequestFailed => formatter.write_str("token request failed"),
            Self::Rejected { http_status } => {
                write!(formatter, "token request rejected ({http_status})")
            }
            Self::InvalidResponse => formatter.write_str("invalid token response"),
        }
    }
}

impl std::error::Error for OAuthError {}

#[cfg(test)]
mod tests {
    use super::{AuthenticatedUser, OAuthClientConfig, OAuthError, TokenEnvelope, TokenSet};

    #[test]
    fn client_request_signature_is_complete_without_exposing_the_salt() {
        let config = OAuthClientConfig::new("test-id", "test-secret", "test-hash-salt").unwrap();
        let signature = config.client_request_signature().unwrap();

        assert!(signature.client_time().ends_with("+00:00"));
        assert_eq!(signature.client_hash().len(), 32);
        assert!(!signature.client_hash().contains("test-hash-salt"));
    }

    #[test]
    fn wrapped_token_response_keeps_secrets_out_of_profile() {
        let json = r#"{
          "response": {
            "access_token": "access-secret",
            "refresh_token": "refresh-secret",
            "expires_in": 3600,
            "user": {
              "id": "42",
              "name": "Alice",
              "account": "alice",
              "is_premium": true,
              "profile_image_urls": { "px_170x170": "https://example.invalid/avatar.jpg" }
            }
          }
        }"#;
        let envelope: TokenEnvelope = serde_json::from_str(json).unwrap();
        let tokens = TokenSet::try_from(envelope.into_payload()).unwrap();
        let (_, _, expires_in, user) = tokens.into_parts();

        assert_eq!(expires_in, 3600);
        assert_eq!(
            user,
            AuthenticatedUser {
                id: "42".into(),
                name: "Alice".into(),
                account: "alice".into(),
                avatar_url: Some("https://example.invalid/avatar.jpg".into()),
                is_premium: true,
            }
        );
    }

    #[test]
    fn malformed_token_response_is_rejected() {
        let json = r#"{
          "access_token": "",
          "refresh_token": "refresh-secret",
          "expires_in": 3600,
          "user": { "id": "42" }
        }"#;
        let envelope: TokenEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(
            TokenSet::try_from(envelope.into_payload()).err(),
            Some(OAuthError::InvalidResponse)
        );
    }
}
