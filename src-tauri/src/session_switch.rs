use crate::session::{SessionSnapshot, SessionState, SessionStateError, SessionTransaction};
use pixiv_client_domain::ConnectionMode;
use std::sync::Arc;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionSwitchError {
    SecureStorageUnavailable,
    SessionUnavailable,
}

pub(crate) struct SessionSwitchCredential {
    refresh_token: Zeroizing<String>,
    connection_mode: ConnectionMode,
}

impl SessionSwitchCredential {
    pub(crate) fn new(refresh_token: Zeroizing<String>, connection_mode: ConnectionMode) -> Self {
        Self {
            refresh_token,
            connection_mode,
        }
    }

    pub(crate) fn token(&self) -> &str {
        self.refresh_token.as_str()
    }

    pub(crate) fn connection_mode(&self) -> ConnectionMode {
        self.connection_mode
    }
}

#[allow(async_fn_in_trait)]
pub(crate) trait SessionCredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<SessionSwitchCredential>, SessionSwitchError>;

    async fn save(
        &self,
        refresh_token: &str,
        connection_mode: ConnectionMode,
    ) -> Result<(), SessionSwitchError>;

    async fn delete(&self) -> Result<(), SessionSwitchError>;
}

pub(crate) trait SessionTransportCache: Send + Sync {
    fn clear(&self) -> Result<(), SessionSwitchError>;
}

pub(crate) trait SessionModeTransaction {
    fn snapshot(&self) -> Result<SessionSnapshot, SessionSwitchError>;

    fn set_connection_mode(
        &mut self,
        connection_mode: ConnectionMode,
    ) -> Result<Option<SessionSnapshot>, SessionSwitchError>;

    fn clear(&mut self) -> Result<SessionSnapshot, SessionSwitchError>;
}

#[allow(async_fn_in_trait)]
pub(crate) trait SessionModeStore: Send + Sync {
    type Transaction<'a>: SessionModeTransaction
    where
        Self: 'a;

    async fn begin(&self) -> Result<Self::Transaction<'_>, SessionSwitchError>;
}

#[derive(Clone)]
pub(crate) struct SessionSwitchCoordinator {
    request_gate: Arc<tokio::sync::RwLock<()>>,
    mutation_gate: Arc<tokio::sync::Semaphore>,
}

pub(crate) struct MediaRouteLease {
    connection_mode: ConnectionMode,
    _request: tokio::sync::OwnedRwLockReadGuard<()>,
}

impl Default for SessionSwitchCoordinator {
    fn default() -> Self {
        Self {
            request_gate: Arc::new(tokio::sync::RwLock::new(())),
            mutation_gate: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}

impl SessionSwitchCoordinator {
    pub(crate) async fn request_guard(&self) -> tokio::sync::OwnedRwLockReadGuard<()> {
        self.request_gate.clone().read_owned().await
    }

    pub(crate) async fn mutation_guard(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, SessionSwitchError> {
        self.mutation_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| SessionSwitchError::SessionUnavailable)
    }

    pub(crate) async fn media_route(
        &self,
        session: &SessionState,
    ) -> Result<Option<MediaRouteLease>, SessionSwitchError> {
        let request = self.request_guard().await;
        let Some((connection_mode, _)) = session
            .connection_context()
            .map_err(SessionSwitchError::from)?
        else {
            return Ok(None);
        };
        Ok(Some(MediaRouteLease {
            connection_mode,
            _request: request,
        }))
    }

    #[cfg(test)]
    fn closed_for_test() -> Self {
        let mutation_gate = Arc::new(tokio::sync::Semaphore::new(1));
        mutation_gate.close();
        Self {
            request_gate: Arc::new(tokio::sync::RwLock::new(())),
            mutation_gate,
        }
    }

    pub(crate) async fn switch<S, C, T>(
        &self,
        target_mode: ConnectionMode,
        session: &S,
        credentials: &C,
        transports: &T,
    ) -> Result<SessionSnapshot, SessionSwitchError>
    where
        S: SessionModeStore,
        C: SessionCredentialStore,
        T: SessionTransportCache,
    {
        let _mutation = self.mutation_guard().await?;
        let _requests = self.request_gate.clone().write_owned().await;
        let mut transaction = session.begin().await?;
        let before = transaction.snapshot()?;
        let Some(old_mode) = before.connection_mode else {
            return Ok(before);
        };
        if old_mode == target_mode {
            return Ok(before);
        }

        let credential = credentials
            .load()
            .await?
            .ok_or(SessionSwitchError::SecureStorageUnavailable)?;
        if credential.connection_mode() != old_mode {
            return Err(SessionSwitchError::SessionUnavailable);
        }
        transports.clear()?;
        if let Err(error) = credentials.save(credential.token(), target_mode).await {
            if credentials
                .save(credential.token(), old_mode)
                .await
                .is_err()
            {
                let _ = credentials.delete().await;
                let _ = transaction.clear();
            }
            return Err(error);
        }
        match transaction.set_connection_mode(target_mode) {
            Ok(Some(updated)) => Ok(updated),
            Ok(None) | Err(_) => {
                if credentials
                    .save(credential.token(), old_mode)
                    .await
                    .is_err()
                {
                    let _ = credentials.delete().await;
                    let _ = transaction.clear();
                    return Err(SessionSwitchError::SecureStorageUnavailable);
                }
                Err(SessionSwitchError::SessionUnavailable)
            }
        }
    }
}

impl MediaRouteLease {
    pub(crate) fn connection_mode(&self) -> ConnectionMode {
        self.connection_mode
    }
}

impl SessionModeTransaction for SessionTransaction<'_> {
    fn snapshot(&self) -> Result<SessionSnapshot, SessionSwitchError> {
        self.snapshot().map_err(SessionSwitchError::from)
    }

    fn set_connection_mode(
        &mut self,
        connection_mode: ConnectionMode,
    ) -> Result<Option<SessionSnapshot>, SessionSwitchError> {
        self.set_connection_mode(connection_mode)
            .map_err(SessionSwitchError::from)
    }

    fn clear(&mut self) -> Result<SessionSnapshot, SessionSwitchError> {
        self.clear().map_err(SessionSwitchError::from)
    }
}

impl SessionModeStore for SessionState {
    type Transaction<'a> = SessionTransaction<'a>;

    async fn begin(&self) -> Result<Self::Transaction<'_>, SessionSwitchError> {
        Ok(self.transaction().await)
    }
}

impl From<SessionStateError> for SessionSwitchError {
    fn from(_: SessionStateError) -> Self {
        Self::SessionUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SessionCredentialStore, SessionModeStore, SessionModeTransaction, SessionSwitchCoordinator,
        SessionSwitchCredential, SessionSwitchError, SessionTransportCache,
    };
    use crate::session::{SessionSnapshot, SessionState};
    use pixiv_client_auth::AuthenticatedUser;
    use pixiv_client_domain::ConnectionMode;
    use std::collections::VecDeque;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;
    use tokio::sync::{Mutex, MutexGuard};
    use zeroize::Zeroizing;

    #[derive(Clone, Copy)]
    enum SaveAction {
        Succeed,
        FailAfterWrite,
        FailWithoutWrite,
        FailBeforeInvalidation,
    }

    struct StoredValue {
        refresh_token: String,
        connection_mode: ConnectionMode,
    }

    #[derive(Clone)]
    struct FakeCredentialStore {
        stored: Arc<Mutex<Option<StoredValue>>>,
        save_actions: Arc<Mutex<VecDeque<SaveAction>>>,
        delete_fails: Arc<AtomicBool>,
        invalidated: Arc<AtomicBool>,
        load_started: Arc<tokio::sync::Notify>,
    }

    impl FakeCredentialStore {
        fn with_token(refresh_token: &str, connection_mode: ConnectionMode) -> Self {
            Self {
                stored: Arc::new(Mutex::new(Some(StoredValue {
                    refresh_token: refresh_token.to_owned(),
                    connection_mode,
                }))),
                save_actions: Arc::new(Mutex::new(VecDeque::new())),
                delete_fails: Arc::new(AtomicBool::new(false)),
                invalidated: Arc::new(AtomicBool::new(false)),
                load_started: Arc::new(tokio::sync::Notify::new()),
            }
        }

        async fn queue_save(&self, action: SaveAction) {
            self.save_actions.lock().await.push_back(action);
        }

        async fn value(&self) -> Option<(String, ConnectionMode)> {
            self.stored
                .lock()
                .await
                .as_ref()
                .map(|stored| (stored.refresh_token.clone(), stored.connection_mode))
        }

        fn fail_delete(&self) {
            self.delete_fails.store(true, Ordering::SeqCst);
        }
    }

    impl SessionCredentialStore for FakeCredentialStore {
        async fn load(&self) -> Result<Option<SessionSwitchCredential>, SessionSwitchError> {
            self.load_started.notify_waiters();
            if self.invalidated.load(Ordering::SeqCst) {
                return Ok(None);
            }
            Ok(self.stored.lock().await.as_ref().map(|stored| {
                SessionSwitchCredential::new(
                    Zeroizing::new(stored.refresh_token.clone()),
                    stored.connection_mode,
                )
            }))
        }

        async fn save(
            &self,
            refresh_token: &str,
            connection_mode: ConnectionMode,
        ) -> Result<(), SessionSwitchError> {
            let action = self
                .save_actions
                .lock()
                .await
                .pop_front()
                .unwrap_or(SaveAction::Succeed);
            if matches!(action, SaveAction::FailBeforeInvalidation) {
                return Err(SessionSwitchError::SecureStorageUnavailable);
            }
            self.invalidated.store(true, Ordering::SeqCst);
            if matches!(action, SaveAction::Succeed | SaveAction::FailAfterWrite) {
                self.stored.lock().await.replace(StoredValue {
                    refresh_token: refresh_token.to_owned(),
                    connection_mode,
                });
            }
            match action {
                SaveAction::Succeed => {
                    self.invalidated.store(false, Ordering::SeqCst);
                    Ok(())
                }
                SaveAction::FailAfterWrite | SaveAction::FailWithoutWrite => {
                    Err(SessionSwitchError::SecureStorageUnavailable)
                }
                SaveAction::FailBeforeInvalidation => unreachable!(),
            }
        }

        async fn delete(&self) -> Result<(), SessionSwitchError> {
            self.invalidated.store(true, Ordering::SeqCst);
            if self.delete_fails.load(Ordering::SeqCst) {
                return Err(SessionSwitchError::SecureStorageUnavailable);
            }
            self.stored.lock().await.take();
            self.invalidated.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Clone, Copy)]
    enum SetAction {
        Succeed,
        FailWithoutChange,
    }

    struct FakeSessionData {
        snapshot: SessionSnapshot,
        set_actions: VecDeque<SetAction>,
    }

    struct FakeSessionStore {
        data: Mutex<FakeSessionData>,
    }

    struct FakeSessionTransaction<'a> {
        data: MutexGuard<'a, FakeSessionData>,
    }

    impl FakeSessionStore {
        fn logged_in(connection_mode: ConnectionMode) -> Self {
            Self {
                data: Mutex::new(FakeSessionData {
                    snapshot: SessionSnapshot {
                        logged_in: true,
                        user: Some(AuthenticatedUser {
                            id: "42".into(),
                            name: "Alice".into(),
                            account: "alice".into(),
                            avatar_url: None,
                            is_premium: false,
                        }),
                        expires_at_unix_seconds: Some(4_000_000_000),
                        connection_mode: Some(connection_mode),
                    },
                    set_actions: VecDeque::new(),
                }),
            }
        }

        async fn queue_set(&self, action: SetAction) {
            self.data.lock().await.set_actions.push_back(action);
        }

        async fn snapshot(&self) -> SessionSnapshot {
            self.data.lock().await.snapshot.clone()
        }
    }

    impl SessionModeTransaction for FakeSessionTransaction<'_> {
        fn snapshot(&self) -> Result<SessionSnapshot, SessionSwitchError> {
            Ok(self.data.snapshot.clone())
        }

        fn set_connection_mode(
            &mut self,
            connection_mode: ConnectionMode,
        ) -> Result<Option<SessionSnapshot>, SessionSwitchError> {
            match self
                .data
                .set_actions
                .pop_front()
                .unwrap_or(SetAction::Succeed)
            {
                SetAction::Succeed => {
                    self.data.snapshot.connection_mode = Some(connection_mode);
                    Ok(Some(self.data.snapshot.clone()))
                }
                SetAction::FailWithoutChange => Err(SessionSwitchError::SessionUnavailable),
            }
        }

        fn clear(&mut self) -> Result<SessionSnapshot, SessionSwitchError> {
            self.data.snapshot = SessionSnapshot::logged_out();
            Ok(self.data.snapshot.clone())
        }
    }

    impl SessionModeStore for FakeSessionStore {
        type Transaction<'a> = FakeSessionTransaction<'a>;

        async fn begin(&self) -> Result<Self::Transaction<'_>, SessionSwitchError> {
            Ok(FakeSessionTransaction {
                data: self.data.lock().await,
            })
        }
    }

    #[derive(Default)]
    struct FakeTransportCache {
        clears: AtomicUsize,
    }

    impl SessionTransportCache for FakeTransportCache {
        fn clear(&self) -> Result<(), SessionSwitchError> {
            self.clears.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn successful_switch_persists_new_mode_without_rotating_refresh_token() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = FakeSessionStore::logged_in(ConnectionMode::Standard);
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            let transports = FakeTransportCache::default();

            let updated = coordinator
                .switch(ConnectionMode::Ech, &session, &credentials, &transports)
                .await
                .unwrap();

            assert_eq!(updated.connection_mode, Some(ConnectionMode::Ech));
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Ech))
            );
            assert_eq!(transports.clears.load(Ordering::SeqCst), 1);

            let restarted_adapter = credentials.clone();
            let restored = restarted_adapter.load().await.unwrap().unwrap();
            assert_eq!(restored.connection_mode(), ConnectionMode::Ech);
            assert_eq!(restored.token(), "refresh-secret");
        });
    }

    #[test]
    fn secure_storage_failure_rolls_back_an_uncertain_write() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = FakeSessionStore::logged_in(ConnectionMode::Standard);
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            credentials.queue_save(SaveAction::FailAfterWrite).await;
            credentials.queue_save(SaveAction::Succeed).await;
            let transports = FakeTransportCache::default();

            let result = coordinator
                .switch(ConnectionMode::Ech, &session, &credentials, &transports)
                .await;

            assert_eq!(result, Err(SessionSwitchError::SecureStorageUnavailable));
            assert_eq!(
                session.snapshot().await.connection_mode,
                Some(ConnectionMode::Standard)
            );
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Standard))
            );
        });
    }

    #[test]
    fn interrupted_credential_overwrite_remains_blocked_after_restart() {
        tauri::async_runtime::block_on(async {
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            credentials.queue_save(SaveAction::FailAfterWrite).await;

            let result = credentials
                .save("refresh-secret", ConnectionMode::Compatible)
                .await;

            assert_eq!(result, Err(SessionSwitchError::SecureStorageUnavailable));
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Compatible))
            );
            let restarted_adapter = credentials.clone();
            assert!(restarted_adapter.load().await.unwrap().is_none());
        });
    }

    #[test]
    fn invalidation_failure_does_not_overwrite_the_existing_credential() {
        tauri::async_runtime::block_on(async {
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            credentials
                .queue_save(SaveAction::FailBeforeInvalidation)
                .await;

            let result = credentials
                .save("refresh-secret", ConnectionMode::Compatible)
                .await;

            assert_eq!(result, Err(SessionSwitchError::SecureStorageUnavailable));
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Standard))
            );
        });
    }

    #[test]
    fn rollback_failure_invalidates_both_session_and_stored_credential() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = FakeSessionStore::logged_in(ConnectionMode::Standard);
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            credentials.queue_save(SaveAction::FailAfterWrite).await;
            credentials.queue_save(SaveAction::FailWithoutWrite).await;
            let transports = FakeTransportCache::default();

            let result = coordinator
                .switch(ConnectionMode::Ech, &session, &credentials, &transports)
                .await;

            assert_eq!(result, Err(SessionSwitchError::SecureStorageUnavailable));
            assert_eq!(session.snapshot().await, SessionSnapshot::logged_out());
            assert_eq!(credentials.value().await, None);
        });
    }

    #[test]
    fn rollback_and_delete_failure_cannot_restore_uncertain_compatible_credential_after_restart() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = FakeSessionStore::logged_in(ConnectionMode::Standard);
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            credentials.queue_save(SaveAction::FailAfterWrite).await;
            credentials.queue_save(SaveAction::FailWithoutWrite).await;
            credentials.fail_delete();
            let transports = FakeTransportCache::default();

            let result = coordinator
                .switch(
                    ConnectionMode::Compatible,
                    &session,
                    &credentials,
                    &transports,
                )
                .await;

            assert_eq!(result, Err(SessionSwitchError::SecureStorageUnavailable));
            assert_eq!(session.snapshot().await, SessionSnapshot::logged_out());
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Compatible))
            );

            let restarted_adapter = credentials.clone();
            assert!(restarted_adapter.load().await.unwrap().is_none());
        });
    }

    #[test]
    fn in_memory_failure_rolls_back_persisted_mode() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = FakeSessionStore::logged_in(ConnectionMode::Standard);
            session.queue_set(SetAction::FailWithoutChange).await;
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            let transports = FakeTransportCache::default();

            let result = coordinator
                .switch(ConnectionMode::Ech, &session, &credentials, &transports)
                .await;

            assert_eq!(result, Err(SessionSwitchError::SessionUnavailable));
            assert_eq!(
                session.snapshot().await.connection_mode,
                Some(ConnectionMode::Standard)
            );
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Standard))
            );
        });
    }

    #[test]
    fn in_memory_failure_and_rollback_failure_enter_a_consistent_logged_out_state() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = FakeSessionStore::logged_in(ConnectionMode::Standard);
            session.queue_set(SetAction::FailWithoutChange).await;
            let credentials =
                FakeCredentialStore::with_token("refresh-secret", ConnectionMode::Standard);
            credentials.queue_save(SaveAction::Succeed).await;
            credentials.queue_save(SaveAction::FailWithoutWrite).await;
            let transports = FakeTransportCache::default();

            let result = coordinator
                .switch(ConnectionMode::Ech, &session, &credentials, &transports)
                .await;

            assert_eq!(result, Err(SessionSwitchError::SecureStorageUnavailable));
            assert_eq!(session.snapshot().await, SessionSnapshot::logged_out());
            assert_eq!(credentials.value().await, None);
        });
    }

    #[test]
    fn switch_waits_for_background_requests_before_touching_credentials() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let request = coordinator.request_guard().await;
            let session = Arc::new(FakeSessionStore::logged_in(ConnectionMode::Standard));
            let credentials = Arc::new(FakeCredentialStore::with_token(
                "refresh-secret",
                ConnectionMode::Standard,
            ));
            let transports = Arc::new(FakeTransportCache::default());
            let task = {
                let coordinator = coordinator.clone();
                let session = session.clone();
                let credentials = credentials.clone();
                let transports = transports.clone();
                tauri::async_runtime::spawn(async move {
                    coordinator
                        .switch(
                            ConnectionMode::Ech,
                            session.as_ref(),
                            credentials.as_ref(),
                            transports.as_ref(),
                        )
                        .await
                })
            };

            tokio::time::sleep(Duration::from_millis(25)).await;
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Standard))
            );
            drop(request);
            let updated = task.await.unwrap().unwrap();
            assert_eq!(updated.connection_mode, Some(ConnectionMode::Ech));
        });
    }

    #[test]
    fn media_route_lease_keeps_the_selected_mode_stable_until_the_request_finishes() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let session = Arc::new(SessionState::default());
            session
                .install(
                    Zeroizing::new("access-secret".to_owned()),
                    AuthenticatedUser {
                        id: "42".into(),
                        name: "Alice".into(),
                        account: "alice".into(),
                        avatar_url: None,
                        is_premium: false,
                    },
                    3_600,
                    ConnectionMode::Standard,
                )
                .unwrap();
            let credentials = Arc::new(FakeCredentialStore::with_token(
                "refresh-secret",
                ConnectionMode::Standard,
            ));
            let transports = Arc::new(FakeTransportCache::default());

            let route = coordinator.media_route(&session).await.unwrap().unwrap();
            assert_eq!(route.connection_mode(), ConnectionMode::Standard);

            let task = {
                let coordinator = coordinator.clone();
                let session = session.clone();
                let credentials = credentials.clone();
                let transports = transports.clone();
                tauri::async_runtime::spawn(async move {
                    coordinator
                        .switch(
                            ConnectionMode::Ech,
                            session.as_ref(),
                            credentials.as_ref(),
                            transports.as_ref(),
                        )
                        .await
                })
            };

            tokio::time::sleep(Duration::from_millis(25)).await;
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Standard))
            );
            drop(route);

            let updated = task.await.unwrap().unwrap();
            assert_eq!(updated.connection_mode, Some(ConnectionMode::Ech));
        });
    }

    #[test]
    fn switch_waits_for_account_mutations_before_touching_credentials() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::default();
            let mutation = coordinator.mutation_guard().await.unwrap();
            let session = Arc::new(FakeSessionStore::logged_in(ConnectionMode::Standard));
            let credentials = Arc::new(FakeCredentialStore::with_token(
                "refresh-secret",
                ConnectionMode::Standard,
            ));
            let transports = Arc::new(FakeTransportCache::default());
            let task = {
                let coordinator = coordinator.clone();
                let session = session.clone();
                let credentials = credentials.clone();
                let transports = transports.clone();
                tauri::async_runtime::spawn(async move {
                    coordinator
                        .switch(
                            ConnectionMode::Ech,
                            session.as_ref(),
                            credentials.as_ref(),
                            transports.as_ref(),
                        )
                        .await
                })
            };

            tokio::time::sleep(Duration::from_millis(25)).await;
            assert_eq!(
                credentials.value().await,
                Some(("refresh-secret".to_owned(), ConnectionMode::Standard))
            );
            drop(mutation);
            let updated = task.await.unwrap().unwrap();
            assert_eq!(updated.connection_mode, Some(ConnectionMode::Ech));
        });
    }

    #[test]
    fn unavailable_mutation_gate_reports_session_unavailable() {
        tauri::async_runtime::block_on(async {
            let coordinator = SessionSwitchCoordinator::closed_for_test();
            assert!(matches!(
                coordinator.mutation_guard().await,
                Err(SessionSwitchError::SessionUnavailable)
            ));
        });
    }
}
