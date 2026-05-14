use std::collections::HashMap;
use std::io::Read;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::FileSource;
use crate::error::Result;
use crate::session::PeerSpec;
use crate::types::message::MessageSearchOptions;
use crate::types::session::SessionPeerConfig;

use super::runtime::block_on;

/// Synchronous wrapper around [`crate::Session`].
#[derive(Clone)]
pub struct Session {
    inner: crate::Session,
}

impl Session {
    pub(crate) fn new(inner: crate::Session) -> Self {
        Self { inner }
    }

    /// Session's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        self.inner.id()
    }

    /// Whether the session is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    /// Cached metadata.
    #[must_use]
    pub fn metadata(&self) -> Option<HashMap<String, Value>> {
        self.inner.metadata()
    }

    /// Cached configuration.
    #[must_use]
    pub fn configuration(&self) -> Option<HashMap<String, Value>> {
        self.inner.configuration()
    }

    /// Refresh cached state from the server.
    pub fn refresh(&self) -> Result<()> {
        block_on(self.inner.refresh())
    }

    /// Fetch and return metadata.
    pub fn get_metadata(&self) -> Result<HashMap<String, Value>> {
        block_on(self.inner.get_metadata())
    }

    /// Set metadata on the server.
    pub fn set_metadata(&self, metadata: HashMap<String, Value>) -> Result<()> {
        block_on(self.inner.set_metadata(metadata))
    }

    /// Fetch and return configuration.
    pub fn get_configuration(&self) -> Result<HashMap<String, Value>> {
        block_on(self.inner.get_configuration())
    }

    /// Set configuration on the server.
    pub fn set_configuration(&self, configuration: HashMap<String, Value>) -> Result<()> {
        block_on(self.inner.set_configuration(configuration))
    }

    /// Add a single peer to this session.
    pub fn add_peer(&self, id: impl Into<String>) -> Result<()> {
        block_on(self.inner.add_peer(id))
    }

    /// Add multiple peers.
    pub fn add_peers(&self, specs: impl IntoIterator<Item = impl Into<PeerSpec>>) -> Result<()> {
        block_on(self.inner.add_peers(specs))
    }

    /// Set the complete peer list (replaces existing).
    pub fn set_peers(&self, specs: impl IntoIterator<Item = impl Into<PeerSpec>>) -> Result<()> {
        block_on(self.inner.set_peers(specs))
    }

    /// Remove peers from this session.
    pub fn remove_peers(&self, ids: impl IntoIterator<Item = impl Into<String>>) -> Result<()> {
        block_on(self.inner.remove_peers(ids))
    }

    /// List peers in this session.
    pub fn peers(&self) -> Result<Vec<super::Peer>> {
        block_on(self.inner.peers()).map(|peers| peers.into_iter().map(super::Peer::new).collect())
    }

    /// Get per-peer configuration.
    pub fn get_peer_configuration(&self, peer_id: &str) -> Result<SessionPeerConfig> {
        block_on(self.inner.get_peer_configuration(peer_id))
    }

    /// Set per-peer configuration.
    pub fn set_peer_configuration(&self, peer_id: &str, config: &SessionPeerConfig) -> Result<()> {
        block_on(self.inner.set_peer_configuration(peer_id, config))
    }

    /// Add messages to this session.
    pub fn add_messages(
        &self,
        messages: Vec<crate::types::message::MessageCreate>,
    ) -> Result<Vec<crate::Message>> {
        block_on(self.inner.add_messages(messages))
    }

    /// List messages, collecting across pages.
    pub fn messages(&self) -> Result<Vec<crate::Message>> {
        block_on(async {
            let page = self.inner.messages().await?;
            super::iter::collect_all_pages(page).await
        })
    }

    /// Delete this session.
    pub fn delete(&self) -> Result<()> {
        block_on(self.inner.delete())
    }

    /// Clone this session.
    pub fn clone_session(&self) -> Result<Session> {
        block_on(self.inner.clone_session()).map(Session::new)
    }

    /// Clone this session up to a message.
    pub fn clone_session_with_message(&self, message_id: &str) -> Result<Session> {
        block_on(self.inner.clone_session_with_message(message_id)).map(Session::new)
    }

    /// Get a single message by ID.
    pub fn get_message(&self, id: &str) -> Result<crate::Message> {
        block_on(self.inner.get_message(id))
    }

    /// Update a message's metadata.
    pub fn update_message(
        &self,
        id: &str,
        metadata: HashMap<String, Value>,
    ) -> Result<crate::Message> {
        block_on(self.inner.update_message(id, metadata))
    }

    /// Get session context.
    pub fn context(&self) -> Result<crate::types::session::SessionContext> {
        block_on(self.inner.context())
    }

    /// Get session context with custom parameters.
    pub fn context_with_options(
        &self,
        options: &crate::types::session::SessionContextOptions,
    ) -> Result<crate::types::session::SessionContext> {
        block_on(self.inner.context_with_options(options))
    }

    /// Get available summaries.
    pub fn summaries(&self) -> Result<crate::types::session::SessionSummaries> {
        block_on(self.inner.summaries())
    }

    /// Search messages within this session.
    pub fn search(&self, query: &str) -> Result<Vec<crate::Message>> {
        block_on(self.inner.search(query))
    }

    /// Search messages within this session with custom options.
    pub fn search_with_options(
        &self,
        options: &MessageSearchOptions,
    ) -> Result<Vec<crate::Message>> {
        block_on(self.inner.search_with_options(options))
    }

    /// Get a peer's representation scoped to this session.
    pub fn representation(&self, peer_id: &str) -> Result<String> {
        block_on(self.inner.representation(peer_id))
    }

    /// Get processing queue status for this session.
    pub fn queue_status(
        &self,
        observer_id: Option<&str>,
        sender_id: Option<&str>,
    ) -> Result<crate::types::dream::QueueStatus> {
        block_on(self.inner.queue_status(observer_id, sender_id))
    }

    /// Begin a file upload to this session.
    ///
    /// Returns a [`BlockingUploadFileBuilder`]. You **must** call `.peer(id)`
    /// and then `.send()` to complete the upload.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn example(session: &honcho_ai::blocking::Session) -> honcho_ai::error::Result<()> {
    /// let msgs = session
    ///     .upload_file(honcho_ai::FileSource::bytes("doc.pdf", b"data", "application/pdf"))
    ///     .peer("alice")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn upload_file(&self, source: impl Into<FileSource>) -> BlockingUploadFileBuilder<'_> {
        BlockingUploadFileBuilder {
            inner: self.inner.upload_file(source),
        }
    }

    /// Begin a file upload from a synchronous reader.
    ///
    /// The reader is fully consumed into memory before the builder is returned,
    /// so this is not truly streaming — but it provides the same builder API as
    /// the async counterpart for convenience.
    ///
    /// # Errors
    ///
    /// Returns [`HonchoError::Io`](crate::error::HonchoError::Io) if reading
    /// from `reader` fails.
    pub fn upload_file_streamed(
        &self,
        filename: impl Into<String>,
        mut reader: impl Read + Send + 'static,
        content_type: impl Into<String>,
    ) -> Result<BlockingUploadFileBuilder<'_>> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(BlockingUploadFileBuilder {
            inner: self
                .inner
                .upload_file(FileSource::bytes(filename, bytes, content_type)),
        })
    }
}

/// Blocking wrapper around [`crate::UploadFileBuilder`].
pub struct BlockingUploadFileBuilder<'a> {
    inner: crate::UploadFileBuilder<'a>,
}

impl BlockingUploadFileBuilder<'_> {
    /// Set the peer that owns the uploaded file (required).
    #[must_use]
    pub fn peer(self, id: impl Into<String>) -> Self {
        Self {
            inner: self.inner.peer(id),
        }
    }

    /// Attach arbitrary JSON metadata to the created message(s).
    #[must_use]
    pub fn metadata(self, value: Value) -> Self {
        Self {
            inner: self.inner.metadata(value),
        }
    }

    /// Attach configuration to the created message(s).
    #[must_use]
    pub fn configuration(self, value: Value) -> Self {
        Self {
            inner: self.inner.configuration(value),
        }
    }

    /// Override the creation timestamp (ISO 3339).
    #[must_use]
    pub fn created_at(self, dt: DateTime<Utc>) -> Self {
        Self {
            inner: self.inner.created_at(dt),
        }
    }

    /// Send the upload request and return the created messages.
    ///
    /// # Errors
    ///
    /// Returns [`HonchoError::Validation`](crate::error::HonchoError::Validation)
    /// if no peer was set via `.peer()`.
    pub fn send(self) -> Result<Vec<crate::Message>> {
        block_on(self.inner.send())
    }
}
