//! Conclusion wrapper and scoped access.

use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::http::client::HttpClient;
use crate::types::conclusion::Conclusion as ConclusionData;

pub(crate) struct ConclusionInner {
    #[allow(dead_code)]
    http: HttpClient,
    workspace_id: String,
    id: String,
    content: String,
    observer_id: String,
    observed_id: String,
    session_id: Option<String>,
    created_at: DateTime<Utc>,
}

/// A conclusion about a peer, produced by observation.
///
/// Wraps the API response and provides field accessors.
#[derive(Clone)]
pub struct Conclusion {
    inner: Arc<ConclusionInner>,
}

impl Conclusion {
    #[allow(dead_code)]
    pub(crate) fn from_parts(http: HttpClient, workspace_id: String, resp: ConclusionData) -> Self {
        Self {
            inner: Arc::new(ConclusionInner {
                http,
                workspace_id,
                id: resp.id,
                content: resp.content,
                observer_id: resp.observer_id,
                observed_id: resp.observed_id,
                session_id: resp.session_id,
                created_at: resp.created_at,
            }),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_response(honcho: &crate::Honcho, resp: ConclusionData) -> Self {
        Self::from_parts(
            honcho.http().clone(),
            honcho.workspace_id().to_owned(),
            resp,
        )
    }

    /// The conclusion's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.inner.id
    }

    /// The conclusion content text.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.inner.content
    }

    /// ID of the peer that made this observation.
    #[must_use]
    pub fn observer_id(&self) -> &str {
        &self.inner.observer_id
    }

    /// ID of the peer being observed.
    #[must_use]
    pub fn observed_id(&self) -> &str {
        &self.inner.observed_id
    }

    /// Optional session this conclusion is scoped to.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.inner.session_id.as_deref()
    }

    /// When this conclusion was created.
    #[must_use]
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.inner.created_at
    }

    /// The workspace this conclusion belongs to.
    #[must_use]
    pub fn workspace_id(&self) -> &str {
        &self.inner.workspace_id
    }
}

impl fmt::Debug for Conclusion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let truncated = if self.inner.content.len() > 50 {
            &self.inner.content[..50]
        } else {
            &self.inner.content
        };
        f.debug_struct("Conclusion")
            .field("id", &self.inner.id)
            .field("content", &truncated)
            .finish()
    }
}

impl fmt::Display for Conclusion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner.content)
    }
}

/// Parameters for creating a single conclusion.
///
/// Use [`ConclusionCreateParams::new()`] for the common case, or the
/// [`bon::Builder`]–generated builder for optional fields.
#[derive(Debug, Clone, Serialize, bon::Builder)]
pub struct ConclusionCreateParams {
    /// The conclusion content text.
    pub(crate) content: String,
    /// Optional session ID to associate the conclusion with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_id: Option<String>,
}

impl ConclusionCreateParams {
    /// Shortcut: create params with content only (no session).
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            session_id: None,
        }
    }
}

pub(crate) struct ConclusionScopeInner {
    #[allow(dead_code)]
    http: HttpClient,
    #[allow(dead_code)]
    workspace_id: String,
    #[allow(clippy::similar_names)]
    observer: String,
    #[allow(clippy::similar_names)]
    observed: String,
}

/// Scoped access to conclusions for a specific observer/observed relationship.
///
/// Typically obtained via `peer.conclusions()` (self-scoped) or
/// `peer.conclusions_of(target)` (cross-peer). Clone is cheap (Arc-backed).
#[derive(Clone)]
pub struct ConclusionScope {
    inner: Arc<ConclusionScopeInner>,
}

impl ConclusionScope {
    #[allow(dead_code, clippy::similar_names)]
    pub(crate) fn new(
        http: HttpClient,
        workspace_id: String,
        observer_id: String,
        observed_id: String,
    ) -> Self {
        Self {
            inner: Arc::new(ConclusionScopeInner {
                http,
                workspace_id,
                observer: observer_id,
                observed: observed_id,
            }),
        }
    }

    /// The observer peer ID for this scope.
    #[must_use]
    pub fn observer_id(&self) -> &str {
        &self.inner.observer
    }

    /// The observed peer ID for this scope.
    #[must_use]
    pub fn observed_id(&self) -> &str {
        &self.inner.observed
    }
}

// TODO (F9.8): Peer::conclusions()       → ConclusionScope (observer=observed=self.id)
// TODO (F9.8): Peer::conclusions_of(t)   → ConclusionScope (observed=target)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_params_minimal_serializes_content_only() {
        let params = ConclusionCreateParams::new("hello");
        let json = serde_json::to_value(params).unwrap();
        assert_eq!(json["content"], "hello");
        assert!(json.get("session_id").is_none());
    }

    #[test]
    fn create_params_with_session_id_serializes_both() {
        let params = ConclusionCreateParams::builder()
            .content("world".to_owned())
            .session_id("s1".to_owned())
            .build();
        let json = serde_json::to_value(params).unwrap();
        assert_eq!(json["content"], "world");
        assert_eq!(json["session_id"], "s1");
    }

    #[test]
    fn debug_truncates_long_content() {
        let data = make_conclusion_data("a".repeat(80), None);
        let conc = Conclusion::from_parts(
            HttpClient::from_params(
                HttpClient::builder()
                    .base_url("http://localhost".to_owned())
                    .build(),
            )
            .unwrap(),
            "ws".to_owned(),
            data,
        );
        let dbg = format!("{conc:?}");
        assert!(dbg.contains("Conclusion { id: \"c1\", content: \""));
        assert!(!dbg.contains(&"a".repeat(80)));
    }

    #[test]
    fn display_returns_full_content() {
        let long = "x".repeat(200);
        let data = make_conclusion_data(long.clone(), None);
        let conc = Conclusion::from_parts(
            HttpClient::from_params(
                HttpClient::builder()
                    .base_url("http://localhost".to_owned())
                    .build(),
            )
            .unwrap(),
            "ws".to_owned(),
            data,
        );
        assert_eq!(format!("{conc}"), long);
    }

    #[test]
    fn getters_return_correct_values() {
        let data = make_conclusion_data("content here".to_owned(), Some("sess-1".to_owned()));
        let conc = Conclusion::from_parts(
            HttpClient::from_params(
                HttpClient::builder()
                    .base_url("http://localhost".to_owned())
                    .build(),
            )
            .unwrap(),
            "ws-1".to_owned(),
            data,
        );
        assert_eq!(conc.id(), "c1");
        assert_eq!(conc.content(), "content here");
        assert_eq!(conc.observer_id(), "obs");
        assert_eq!(conc.observed_id(), "obd");
        assert_eq!(conc.session_id(), Some("sess-1"));
        assert_eq!(conc.workspace_id(), "ws-1");
    }

    fn make_conclusion_data(content: String, session_id: Option<String>) -> ConclusionData {
        ConclusionData {
            id: "c1".to_owned(),
            content,
            observer_id: "obs".to_owned(),
            observed_id: "obd".to_owned(),
            session_id,
            created_at: chrono::Utc::now(),
        }
    }

    fn test_http() -> HttpClient {
        HttpClient::from_params(
            HttpClient::builder()
                .base_url("http://localhost".to_owned())
                .build(),
        )
        .unwrap()
    }

    #[test]
    fn conclusion_scope_new_self_scoped() {
        let scope = ConclusionScope::new(
            test_http(),
            "ws".to_owned(),
            "p1".to_owned(),
            "p1".to_owned(),
        );
        assert_eq!(scope.observer_id(), "p1");
        assert_eq!(scope.observed_id(), "p1");
    }

    #[test]
    fn conclusion_scope_with_different_target() {
        let scope = ConclusionScope::new(
            test_http(),
            "ws".to_owned(),
            "alice".to_owned(),
            "bob".to_owned(),
        );
        assert_eq!(scope.observer_id(), "alice");
        assert_eq!(scope.observed_id(), "bob");
    }

    #[test]
    fn conclusion_scope_clone_is_cheap() {
        let scope =
            ConclusionScope::new(test_http(), "ws".to_owned(), "a".to_owned(), "b".to_owned());
        let clone = scope.clone();
        assert_eq!(Arc::strong_count(&scope.inner), 2);
        assert_eq!(clone.observer_id(), "a");
        assert_eq!(clone.observed_id(), "b");
        drop(clone);
        assert_eq!(Arc::strong_count(&scope.inner), 1);
    }

    #[test]
    fn conclusion_scope_construction_basic() {
        let scope = ConclusionScope::new(
            test_http(),
            "ws-99".to_owned(),
            "observer".to_owned(),
            "observed".to_owned(),
        );
        assert_eq!(scope.observer_id(), "observer");
        assert_eq!(scope.observed_id(), "observed");
    }
}
