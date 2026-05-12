//! Placeholder Peer wrapper (Phase 5 will flesh out methods).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::http::client::HttpClient;
use crate::types::peer::Peer as PeerResponse;

#[allow(dead_code)]
pub(crate) struct PeerInner {
    http: HttpClient,
    workspace_id: String,
    id: String,
    metadata: RwLock<Option<HashMap<String, serde_json::Value>>>,
    configuration: RwLock<Option<HashMap<String, serde_json::Value>>>,
}

/// A peer in a Honcho workspace.
///
/// Wraps the API response and provides lazy-access to peer metadata.
/// Methods for interacting with the peer will be added in Phase 5.
#[derive(Clone)]
pub struct Peer {
    inner: Arc<PeerInner>,
}

impl Peer {
    #[allow(dead_code)]
    pub(crate) fn from_response(honcho: &crate::Honcho, resp: PeerResponse) -> Self {
        Self {
            inner: Arc::new(PeerInner {
                http: honcho.http().clone(),
                workspace_id: honcho.workspace_id().to_owned(),
                id: resp.id,
                metadata: RwLock::new(Some(resp.metadata)),
                configuration: RwLock::new(Some(resp.configuration)),
            }),
        }
    }

    /// The peer's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.inner.id
    }

    /// Cached metadata from the last API response.
    #[must_use]
    #[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    pub fn metadata(&self) -> Option<HashMap<String, serde_json::Value>> {
        self.inner.metadata.read().unwrap().clone()
    }
}
