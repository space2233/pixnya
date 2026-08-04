use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use std::fmt;
use subtle::ConstantTimeEq;
use url::Url;
use zeroize::{Zeroize, ZeroizeOnDrop};

mod oauth;

pub use oauth::{
    AuthenticatedUser, ClientRequestSignature, OAuthClient, OAuthClientConfig, OAuthError,
    ProfileImageUrls, TokenSet,
};

const ENTROPY_BYTES: usize = 32;
const PKCE_METHOD: &str = "S256";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackTarget {
    scheme: String,
    host: String,
    path: String,
}

impl CallbackTarget {
    pub fn new(
        scheme: impl AsRef<str>,
        host: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<Self, LoginError> {
        let candidate = format!("{}://{}{}", scheme.as_ref(), host.as_ref(), path.as_ref());
        let parsed = Url::parse(&candidate).map_err(|_| LoginError::InvalidCallbackTarget)?;

        if parsed.cannot_be_a_base()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(LoginError::InvalidCallbackTarget);
        }

        let host = parsed
            .host_str()
            .filter(|value| !value.is_empty())
            .ok_or(LoginError::InvalidCallbackTarget)?;

        if parsed.path().is_empty() || parsed.path() == "/" {
            return Err(LoginError::InvalidCallbackTarget);
        }

        Ok(Self {
            scheme: parsed.scheme().to_owned(),
            host: host.to_owned(),
            path: parsed.path().to_owned(),
        })
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn display_value(&self) -> String {
        format!("{}://{}{}", self.scheme, self.host, self.path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    Pending,
    Completed,
    Cancelled,
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct SecretString(String);

pub struct AuthorizationParameters<'attempt> {
    state: &'attempt str,
    code_challenge: &'attempt str,
}

impl AuthorizationParameters<'_> {
    pub fn state(&self) -> &str {
        self.state
    }

    pub fn code_challenge(&self) -> &str {
        self.code_challenge
    }

    pub fn code_challenge_method(&self) -> &'static str {
        PKCE_METHOD
    }
}

pub struct AuthorizationGrant {
    code: SecretString,
    code_verifier: SecretString,
}

impl fmt::Debug for AuthorizationGrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizationGrant")
            .field("code", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .finish()
    }
}

impl AuthorizationGrant {
    pub fn code(&self) -> &str {
        &self.code.0
    }

    pub fn code_verifier(&self) -> &str {
        &self.code_verifier.0
    }
}

pub struct LoginAttempt {
    target: CallbackTarget,
    state: SecretString,
    code_verifier: SecretString,
    code_challenge: String,
    status: LoginStatus,
}

impl LoginAttempt {
    pub fn begin(target: CallbackTarget) -> Result<Self, LoginError> {
        let mut entropy = [0_u8; ENTROPY_BYTES * 2];
        getrandom::fill(&mut entropy).map_err(|_| LoginError::SecureRandomUnavailable)?;
        Ok(Self::begin_from_entropy(target, entropy))
    }

    pub fn target(&self) -> &CallbackTarget {
        &self.target
    }

    pub fn status(&self) -> LoginStatus {
        self.status
    }

    pub fn authorization_parameters(&self) -> Result<AuthorizationParameters<'_>, LoginError> {
        if self.status != LoginStatus::Pending {
            return Err(LoginError::AttemptNotPending);
        }

        Ok(AuthorizationParameters {
            state: &self.state.0,
            code_challenge: &self.code_challenge,
        })
    }

    pub fn accept_callback(
        &mut self,
        callback_url: &str,
    ) -> Result<AuthorizationGrant, LoginError> {
        self.accept_callback_with_state_policy(callback_url, StatePolicy::Required)
    }

    /// Accepts a callback captured by the non-exported login surface that owns this attempt.
    ///
    /// Pixiv's app login endpoint does not echo OAuth `state`. The caller must therefore bind
    /// this method to the exact private WebView/window launch that created the PKCE verifier.
    /// If a callback does contain `state`, it is still validated in constant time.
    pub fn accept_private_surface_callback(
        &mut self,
        callback_url: &str,
    ) -> Result<AuthorizationGrant, LoginError> {
        self.accept_callback_with_state_policy(callback_url, StatePolicy::Optional)
    }

    fn accept_callback_with_state_policy(
        &mut self,
        callback_url: &str,
        state_policy: StatePolicy,
    ) -> Result<AuthorizationGrant, LoginError> {
        if self.status != LoginStatus::Pending {
            return Err(LoginError::AttemptNotPending);
        }

        let callback = Url::parse(callback_url).map_err(|_| LoginError::InvalidCallbackUrl)?;
        if callback.scheme() != self.target.scheme
            || callback.host_str() != Some(self.target.host.as_str())
            || callback.path() != self.target.path
            || !callback.username().is_empty()
            || callback.password().is_some()
            || callback.port().is_some()
            || callback.fragment().is_some()
        {
            return Err(LoginError::UnexpectedCallbackTarget);
        }

        let mut states = Vec::new();
        let mut codes = Vec::new();
        let mut errors = Vec::new();
        for (key, value) in callback.query_pairs() {
            match key.as_ref() {
                "state" => states.push(value.into_owned()),
                "code" => codes.push(value.into_owned()),
                "error" => errors.push(value.into_owned()),
                _ => {}
            }
        }

        if states.len() > 1 {
            return Err(LoginError::DuplicateState);
        }
        match states.pop() {
            Some(received_state) => {
                if self
                    .state
                    .0
                    .as_bytes()
                    .ct_eq(received_state.as_bytes())
                    .unwrap_u8()
                    != 1
                {
                    return Err(LoginError::StateMismatch);
                }
            }
            None if state_policy == StatePolicy::Required => return Err(LoginError::MissingState),
            None => {}
        }

        if codes.len() > 1 || errors.len() > 1 {
            return Err(LoginError::DuplicateAuthorizationResult);
        }
        if !codes.is_empty() && !errors.is_empty() {
            return Err(LoginError::ConflictingAuthorizationResult);
        }
        if !errors.is_empty() {
            self.finish_without_grant(LoginStatus::Completed);
            return Err(LoginError::AuthorizationDenied);
        }

        let code = codes.pop().ok_or(LoginError::MissingAuthorizationResult)?;
        if code.is_empty() {
            return Err(LoginError::EmptyAuthorizationCode);
        }

        let verifier = std::mem::take(&mut self.code_verifier.0);
        self.state.0.zeroize();
        self.code_challenge.zeroize();
        self.status = LoginStatus::Completed;

        Ok(AuthorizationGrant {
            code: SecretString(code),
            code_verifier: SecretString(verifier),
        })
    }

    pub fn cancel(&mut self) -> Result<(), LoginError> {
        if self.status != LoginStatus::Pending {
            return Err(LoginError::AttemptNotPending);
        }

        self.finish_without_grant(LoginStatus::Cancelled);
        Ok(())
    }

    fn begin_from_entropy(target: CallbackTarget, entropy: [u8; ENTROPY_BYTES * 2]) -> Self {
        let state = URL_SAFE_NO_PAD.encode(&entropy[..ENTROPY_BYTES]);
        let code_verifier = URL_SAFE_NO_PAD.encode(&entropy[ENTROPY_BYTES..]);
        let code_challenge = derive_pkce_challenge(&code_verifier);

        Self {
            target,
            state: SecretString(state),
            code_verifier: SecretString(code_verifier),
            code_challenge,
            status: LoginStatus::Pending,
        }
    }

    fn finish_without_grant(&mut self, status: LoginStatus) {
        self.state.0.zeroize();
        self.code_verifier.0.zeroize();
        self.code_challenge.zeroize();
        self.status = status;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatePolicy {
    Required,
    Optional,
}

fn derive_pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginError {
    InvalidCallbackTarget,
    SecureRandomUnavailable,
    AttemptNotPending,
    InvalidCallbackUrl,
    UnexpectedCallbackTarget,
    MissingState,
    DuplicateState,
    StateMismatch,
    MissingAuthorizationResult,
    DuplicateAuthorizationResult,
    ConflictingAuthorizationResult,
    EmptyAuthorizationCode,
    AuthorizationDenied,
}

impl fmt::Display for LoginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidCallbackTarget => "invalid OAuth callback target",
            Self::SecureRandomUnavailable => "secure random source is unavailable",
            Self::AttemptNotPending => "login attempt is not pending",
            Self::InvalidCallbackUrl => "invalid OAuth callback URL",
            Self::UnexpectedCallbackTarget => "unexpected OAuth callback target",
            Self::MissingState => "OAuth callback is missing state",
            Self::DuplicateState => "OAuth callback contains duplicate state",
            Self::StateMismatch => "OAuth callback state does not match",
            Self::MissingAuthorizationResult => "OAuth callback has no authorization result",
            Self::DuplicateAuthorizationResult => {
                "OAuth callback contains duplicate authorization results"
            }
            Self::ConflictingAuthorizationResult => {
                "OAuth callback contains both success and error results"
            }
            Self::EmptyAuthorizationCode => "OAuth callback contains an empty authorization code",
            Self::AuthorizationDenied => "OAuth authorization was denied",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for LoginError {}

#[cfg(test)]
mod tests {
    use super::{
        derive_pkce_challenge, CallbackTarget, LoginAttempt, LoginError, LoginStatus, ENTROPY_BYTES,
    };

    fn target() -> CallbackTarget {
        CallbackTarget::new("pixiv-client", "oauth", "/callback").unwrap()
    }

    fn attempt() -> LoginAttempt {
        let mut entropy = [0_u8; ENTROPY_BYTES * 2];
        for (index, byte) in entropy.iter_mut().enumerate() {
            *byte = index as u8;
        }
        LoginAttempt::begin_from_entropy(target(), entropy)
    }

    #[test]
    fn pkce_challenge_matches_rfc_7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

        assert_eq!(
            derive_pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_parameters_are_url_safe_and_use_s256() {
        let attempt = attempt();
        let parameters = attempt.authorization_parameters().unwrap();

        assert_eq!(parameters.state().len(), 43);
        assert_eq!(parameters.code_challenge().len(), 43);
        assert_eq!(parameters.code_challenge_method(), "S256");
        assert!(!parameters.state().contains('='));
        assert!(!parameters.code_challenge().contains('='));
    }

    #[test]
    fn valid_callback_returns_code_and_original_verifier_once() {
        let mut attempt = attempt();
        let parameters = attempt.authorization_parameters().unwrap();
        let state = parameters.state().to_owned();
        let callback =
            format!("pixiv-client://oauth/callback?code=authorization-code&state={state}");

        let grant = attempt.accept_callback(&callback).unwrap();

        assert_eq!(grant.code(), "authorization-code");
        assert_eq!(grant.code_verifier().len(), 43);
        assert!(!format!("{grant:?}").contains("authorization-code"));
        assert!(!format!("{grant:?}").contains(grant.code_verifier()));
        assert_eq!(attempt.status(), LoginStatus::Completed);
        assert_eq!(
            attempt.accept_callback(&callback).unwrap_err(),
            LoginError::AttemptNotPending
        );
    }

    #[test]
    fn private_login_surface_accepts_pixiv_callback_without_state() {
        let mut attempt = attempt();

        let grant = attempt
            .accept_private_surface_callback(
                "pixiv-client://oauth/callback?code=authorization-code",
            )
            .unwrap();

        assert_eq!(grant.code(), "authorization-code");
        assert_eq!(grant.code_verifier().len(), 43);
        assert_eq!(attempt.status(), LoginStatus::Completed);
    }

    #[test]
    fn private_login_surface_still_rejects_wrong_target_or_wrong_optional_state() {
        let mut wrong_target = attempt();
        assert_eq!(
            wrong_target
                .accept_private_surface_callback(
                    "pixiv-client://attacker/callback?code=authorization-code",
                )
                .unwrap_err(),
            LoginError::UnexpectedCallbackTarget
        );

        let mut wrong_state = attempt();
        assert_eq!(
            wrong_state
                .accept_private_surface_callback(
                    "pixiv-client://oauth/callback?code=authorization-code&state=wrong",
                )
                .unwrap_err(),
            LoginError::StateMismatch
        );
    }

    #[test]
    fn mismatched_state_is_rejected_without_completing_attempt() {
        let mut attempt = attempt();

        assert_eq!(
            attempt
                .accept_callback("pixiv-client://oauth/callback?code=value&state=wrong")
                .unwrap_err(),
            LoginError::StateMismatch
        );
        assert_eq!(attempt.status(), LoginStatus::Pending);
    }

    #[test]
    fn unexpected_callback_target_is_rejected() {
        let mut attempt = attempt();
        let state = attempt
            .authorization_parameters()
            .unwrap()
            .state()
            .to_owned();

        assert_eq!(
            attempt
                .accept_callback(&format!(
                    "pixiv-client://attacker/callback?code=value&state={state}"
                ))
                .unwrap_err(),
            LoginError::UnexpectedCallbackTarget
        );
    }

    #[test]
    fn duplicate_or_conflicting_results_are_rejected() {
        let mut duplicate_state_attempt = attempt();
        let state = duplicate_state_attempt
            .authorization_parameters()
            .unwrap()
            .state()
            .to_owned();
        assert_eq!(
            duplicate_state_attempt
                .accept_callback(&format!(
                    "pixiv-client://oauth/callback?code=value&state={state}&state={state}"
                ))
                .unwrap_err(),
            LoginError::DuplicateState
        );

        let mut conflicting_attempt = attempt();
        let state = conflicting_attempt
            .authorization_parameters()
            .unwrap()
            .state()
            .to_owned();
        assert_eq!(
            conflicting_attempt
                .accept_callback(&format!(
                    "pixiv-client://oauth/callback?code=value&error=denied&state={state}"
                ))
                .unwrap_err(),
            LoginError::ConflictingAuthorizationResult
        );
    }

    #[test]
    fn denial_and_cancellation_close_the_attempt() {
        let mut denied = attempt();
        let state = denied
            .authorization_parameters()
            .unwrap()
            .state()
            .to_owned();
        assert_eq!(
            denied
                .accept_callback(&format!(
                    "pixiv-client://oauth/callback?error=access_denied&state={state}"
                ))
                .unwrap_err(),
            LoginError::AuthorizationDenied
        );
        assert_eq!(denied.status(), LoginStatus::Completed);

        let mut cancelled = attempt();
        cancelled.cancel().unwrap();
        assert_eq!(cancelled.status(), LoginStatus::Cancelled);
        assert_eq!(
            cancelled.cancel().unwrap_err(),
            LoginError::AttemptNotPending
        );
    }
}
