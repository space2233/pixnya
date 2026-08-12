use pixiv_client_auth::AuthenticatedUser;
use pixiv_client_domain::ConnectionMode;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

#[derive(Default)]
pub(crate) struct SessionState {
    current: Mutex<Option<ActiveSession>>,
    operation_gate: tokio::sync::Mutex<()>,
    next_generation: AtomicU64,
}

struct ActiveSession {
    access_token: Zeroizing<String>,
    user: AuthenticatedUser,
    expires_at_unix_seconds: u64,
    connection_mode: ConnectionMode,
    generation: u64,
}

pub(crate) struct AuthenticatedContext {
    access_token: Zeroizing<String>,
    user_id: String,
    connection_mode: ConnectionMode,
    generation: u64,
}

pub(crate) struct SessionTransaction<'state> {
    state: &'state SessionState,
    _operation: tokio::sync::MutexGuard<'state, ()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionSnapshot {
    pub logged_in: bool,
    pub user: Option<AuthenticatedUser>,
    pub expires_at_unix_seconds: Option<u64>,
    pub connection_mode: Option<ConnectionMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionStateError {
    Unavailable,
}

impl SessionState {
    pub(crate) async fn operation_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.operation_gate.lock().await
    }

    pub(crate) async fn transaction(&self) -> SessionTransaction<'_> {
        SessionTransaction {
            state: self,
            _operation: self.operation_gate.lock().await,
        }
    }

    pub(crate) fn install(
        &self,
        access_token: Zeroizing<String>,
        user: AuthenticatedUser,
        expires_in_seconds: u64,
        connection_mode: ConnectionMode,
    ) -> Result<SessionSnapshot, SessionStateError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionStateError::Unavailable)?;
        let expires_at = now
            .checked_add(Duration::from_secs(expires_in_seconds))
            .ok_or(SessionStateError::Unavailable)?
            .as_secs();
        let mut current = self
            .current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)?;
        let generation = self
            .next_generation
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        current.replace(ActiveSession {
            access_token,
            user,
            expires_at_unix_seconds: expires_at,
            connection_mode,
            generation,
        });
        snapshot_from(current.as_ref())
    }

    pub(crate) fn snapshot(&self) -> Result<SessionSnapshot, SessionStateError> {
        let current = self
            .current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)?;
        snapshot_from(current.as_ref())
    }

    pub(crate) fn clear(&self) -> Result<SessionSnapshot, SessionStateError> {
        self.current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)?
            .take();
        Ok(SessionSnapshot::logged_out())
    }

    pub(crate) fn set_connection_mode(
        &self,
        connection_mode: ConnectionMode,
    ) -> Result<Option<SessionSnapshot>, SessionStateError> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)?;
        let Some(session) = current.as_mut() else {
            return Ok(None);
        };
        session.connection_mode = connection_mode;
        session.generation = self
            .next_generation
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        snapshot_from(current.as_ref()).map(Some)
    }

    pub(crate) fn authenticated_context(
        &self,
        minimum_remaining_seconds: u64,
    ) -> Result<Option<AuthenticatedContext>, SessionStateError> {
        let now = unix_time_seconds()?;
        let freshness_boundary = now
            .checked_add(minimum_remaining_seconds)
            .ok_or(SessionStateError::Unavailable)?;
        let current = self
            .current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)?;

        Ok(current.as_ref().and_then(|session| {
            (session.expires_at_unix_seconds > freshness_boundary).then(|| AuthenticatedContext {
                access_token: session.access_token.clone(),
                user_id: session.user.id.clone(),
                connection_mode: session.connection_mode,
                generation: session.generation,
            })
        }))
    }

    pub(crate) fn connection_context(
        &self,
    ) -> Result<Option<(ConnectionMode, u64)>, SessionStateError> {
        self.current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)
            .map(|current| {
                current
                    .as_ref()
                    .map(|session| (session.connection_mode, session.generation))
            })
    }

    pub(crate) fn generation(&self) -> Result<Option<u64>, SessionStateError> {
        self.current
            .lock()
            .map_err(|_| SessionStateError::Unavailable)
            .map(|current| current.as_ref().map(|session| session.generation))
    }
}

impl SessionTransaction<'_> {
    pub(crate) fn snapshot(&self) -> Result<SessionSnapshot, SessionStateError> {
        self.state.snapshot()
    }

    pub(crate) fn set_connection_mode(
        &mut self,
        connection_mode: ConnectionMode,
    ) -> Result<Option<SessionSnapshot>, SessionStateError> {
        self.state.set_connection_mode(connection_mode)
    }

    pub(crate) fn clear(&mut self) -> Result<SessionSnapshot, SessionStateError> {
        self.state.clear()
    }
}

impl AuthenticatedContext {
    pub(crate) fn access_token(&self) -> &str {
        self.access_token.as_str()
    }

    pub(crate) fn connection_mode(&self) -> ConnectionMode {
        self.connection_mode
    }

    pub(crate) fn user_id(&self) -> &str {
        &self.user_id
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}

impl SessionSnapshot {
    pub(crate) fn logged_out() -> Self {
        Self {
            logged_in: false,
            user: None,
            expires_at_unix_seconds: None,
            connection_mode: None,
        }
    }
}

fn snapshot_from(session: Option<&ActiveSession>) -> Result<SessionSnapshot, SessionStateError> {
    Ok(match session {
        Some(session) => SessionSnapshot {
            logged_in: true,
            user: Some(session.user.clone()),
            expires_at_unix_seconds: Some(session.expires_at_unix_seconds),
            connection_mode: Some(session.connection_mode),
        },
        None => SessionSnapshot::logged_out(),
    })
}

fn unix_time_seconds() -> Result<u64, SessionStateError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| SessionStateError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::SessionState;
    use pixiv_client_auth::AuthenticatedUser;
    use zeroize::Zeroizing;

    #[test]
    fn snapshots_never_contain_access_tokens() {
        let state = SessionState::default();
        let snapshot = state
            .install(
                Zeroizing::new("access-secret".to_owned()),
                AuthenticatedUser {
                    id: "42".into(),
                    name: "Alice".into(),
                    account: "alice".into(),
                    avatar_url: None,
                    is_premium: false,
                },
                3600,
                pixiv_client_domain::ConnectionMode::Standard,
            )
            .unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();

        assert!(snapshot.logged_in);
        assert!(!serialized.contains("access-secret"));
        assert!(!serialized.contains("token"));
    }

    #[test]
    fn authenticated_context_keeps_token_inside_rust_and_honours_expiry_margin() {
        let state = SessionState::default();
        state
            .install(
                Zeroizing::new("access-secret".to_owned()),
                AuthenticatedUser {
                    id: "42".into(),
                    name: "Alice".into(),
                    account: "alice".into(),
                    avatar_url: None,
                    is_premium: false,
                },
                120,
                pixiv_client_domain::ConnectionMode::Ech,
            )
            .unwrap();

        let context = state.authenticated_context(60).unwrap().unwrap();
        assert_eq!(context.access_token(), "access-secret");
        assert_eq!(context.user_id(), "42");
        assert_eq!(
            context.connection_mode(),
            pixiv_client_domain::ConnectionMode::Ech
        );
        assert_eq!(context.generation(), 1);
        assert_eq!(state.generation().unwrap(), Some(1));
        assert!(state.authenticated_context(120).unwrap().is_none());
    }

    #[test]
    fn changing_connection_mode_preserves_the_session_and_advances_its_generation() {
        let state = SessionState::default();
        state
            .install(
                Zeroizing::new("access-secret".to_owned()),
                AuthenticatedUser {
                    id: "42".into(),
                    name: "Alice".into(),
                    account: "alice".into(),
                    avatar_url: None,
                    is_premium: false,
                },
                3600,
                pixiv_client_domain::ConnectionMode::Standard,
            )
            .unwrap();

        let snapshot = state
            .set_connection_mode(pixiv_client_domain::ConnectionMode::Compatible)
            .unwrap()
            .unwrap();
        let context = state.authenticated_context(0).unwrap().unwrap();

        assert!(snapshot.logged_in);
        assert_eq!(snapshot.user.unwrap().id, "42");
        assert_eq!(
            snapshot.connection_mode,
            Some(pixiv_client_domain::ConnectionMode::Compatible)
        );
        assert_eq!(context.access_token(), "access-secret");
        assert_eq!(context.generation(), 2);
    }
}
