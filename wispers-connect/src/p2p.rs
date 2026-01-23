//! Peer-to-peer connection types.
//!
//! This module provides the types for establishing and managing P2P connections
//! between activated nodes.

use thiserror::Error;

use crate::ice::{IceAnswerer, IceCaller, IceError};

/// Error type for P2P connection operations.
#[derive(Debug, Error)]
pub enum P2pError {
    #[error("hub error: {0}")]
    Hub(#[from] crate::hub::HubError),

    #[error("ICE error: {0}")]
    Ice(#[from] IceError),

    #[error("peer rejected connection: {0}")]
    PeerRejected(String),

    #[error("signature verification failed")]
    SignatureVerificationFailed,

    #[error("connection closed")]
    ConnectionClosed,

    #[error("encryption error")]
    Encryption,
}

/// A peer-to-peer connection to another node (caller side).
///
/// This provides encrypted UDP communication with a peer node after
/// successful ICE negotiation.
pub struct P2pConnection {
    /// The peer's node number.
    pub peer_node_number: i32,

    /// Connection ID assigned by the answerer.
    pub connection_id: i64,

    /// The underlying ICE connection.
    ice: IceCaller,

    /// Shared secret derived from X25519 key exchange (for encryption).
    #[allow(dead_code)]
    shared_secret: [u8; 32],
}

impl P2pConnection {
    /// Create a new P2P connection (internal use).
    pub(crate) fn new(
        peer_node_number: i32,
        connection_id: i64,
        ice: IceCaller,
        shared_secret: [u8; 32],
    ) -> Self {
        Self {
            peer_node_number,
            connection_id,
            ice,
            shared_secret,
        }
    }

    /// Send data to the peer.
    ///
    /// The data is encrypted using the shared secret before transmission.
    pub fn send(&self, data: &[u8]) -> Result<(), P2pError> {
        // TODO: Encrypt data with shared_secret using AES-GCM
        self.ice.send(data)?;
        Ok(())
    }

    /// Receive data from the peer.
    ///
    /// Returns decrypted data from the peer.
    pub async fn recv(&self) -> Result<Vec<u8>, P2pError> {
        let data = self.ice.recv().await?;
        // TODO: Decrypt data with shared_secret using AES-GCM
        Ok(data)
    }

    /// Close the connection.
    pub fn close(self) {
        self.ice.close();
    }

    /// Get the current connection state.
    pub fn is_connected(&self) -> bool {
        self.ice.state().is_connected()
    }
}

/// A peer-to-peer connection to another node (answerer side).
pub struct P2pConnectionAnswerer {
    /// The peer's node number (the caller).
    pub peer_node_number: i32,

    /// Connection ID we assigned.
    pub connection_id: i64,

    /// The underlying ICE connection.
    ice: IceAnswerer,

    /// Shared secret derived from X25519 key exchange (for encryption).
    #[allow(dead_code)]
    shared_secret: [u8; 32],
}

impl P2pConnectionAnswerer {
    /// Create a new P2P connection answerer (internal use).
    pub(crate) fn new(
        peer_node_number: i32,
        connection_id: i64,
        ice: IceAnswerer,
        shared_secret: [u8; 32],
    ) -> Self {
        Self {
            peer_node_number,
            connection_id,
            ice,
            shared_secret,
        }
    }

    /// Wait for the ICE connection to complete.
    pub async fn connect(&self) -> Result<(), P2pError> {
        self.ice.connect().await?;
        Ok(())
    }

    /// Send data to the peer.
    pub fn send(&self, data: &[u8]) -> Result<(), P2pError> {
        // TODO: Encrypt data with shared_secret using AES-GCM
        self.ice.send(data)?;
        Ok(())
    }

    /// Receive data from the peer.
    pub async fn recv(&self) -> Result<Vec<u8>, P2pError> {
        let data = self.ice.recv().await?;
        // TODO: Decrypt data with shared_secret using AES-GCM
        Ok(data)
    }

    /// Close the connection.
    pub fn close(self) {
        self.ice.close();
    }

    /// Get the current connection state.
    pub fn is_connected(&self) -> bool {
        self.ice.state().is_connected()
    }
}

/// Re-export StunTurnConfig from proto.
pub use crate::hub::proto::StunTurnConfig;
