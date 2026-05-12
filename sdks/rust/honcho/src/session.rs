//! Placeholder Session wrapper (Phase 6 will flesh out methods).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::http::client::HttpClient;
use crate::types::session::Session as SessionResponse;

#[allow(dead_code)]
pub(crate) struct SessionInner {
    http: HttpClient,
    workspace_id: String,
    id: String,
    is_active: bool,
    metadata: RwLock<Option<HashMap<String, serde_json::Value>>>,
    configuration: RwLock<Option<HashMap<String, serde_json::Value>>>,
}

/// A session in a Honcho workspace.
///
/// Wraps the API response and provides lazy-access to session metadata.
/// Methods for interacting with the session will be added in Phase 6.
#[derive(Clone)]
pub struct Session {
    inner: Arc<SessionInner>,
}

impl Session {
    #[allow(dead_code)]
    pub(crate) fn from_response(honcho: &crate::Honcho, resp: SessionResponse) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                http: honcho.http().clone(),
                workspace_id: honcho.workspace_id().to_owned(),
                id: resp.id,
                is_active: resp.is_active,
                metadata: RwLock::new(Some(resp.metadata)),
                configuration: RwLock::new(Some(resp.configuration)),
            }),
        }
    }

    /// The session's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.inner.id
    }

    /// Whether the session is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.inner.is_active
    }

    /// Cached metadata from the last API response.
    #[must_use]
    #[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    pub fn metadata(&self) -> Option<HashMap<String, serde_json::Value>> {
        self.inner.metadata.read().unwrap().clone()
    }
}
