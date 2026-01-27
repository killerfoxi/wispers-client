//! QUIC transport layer for stream-based P2P connections.
//!
//! This module provides QUIC connections on top of ICE-established UDP paths,
//! using quiche (Cloudflare's QUIC implementation). Authentication uses TLS 1.3
//! with a Pre-Shared Key (PSK) derived from the X25519 Diffie-Hellman exchange.

use boring::ssl::{SslContextBuilder, SslMethod};
use hkdf::Hkdf;
use sha2::Sha256;
use std::sync::Arc;

/// PSK identity used in TLS 1.3 handshake.
/// Both peers must use the same identity string.
pub const PSK_IDENTITY: &[u8] = b"wispers-connect-v1";

/// ALPN protocol identifier for QUIC connections.
pub const ALPN: &[u8] = b"wispers-connect";

/// QUIC version to use (v1 per RFC 9000).
const QUIC_VERSION: u32 = quiche::PROTOCOL_VERSION;

/// Maximum idle timeout in milliseconds.
const MAX_IDLE_TIMEOUT_MS: u64 = 30_000;

/// Initial max data (connection-level flow control).
const INITIAL_MAX_DATA: u64 = 10_000_000; // 10 MB

/// Initial max stream data (per-stream flow control).
const INITIAL_MAX_STREAM_DATA: u64 = 1_000_000; // 1 MB

/// Maximum concurrent bidirectional streams.
const INITIAL_MAX_STREAMS_BIDI: u64 = 100;

/// Length of the derived PSK in bytes.
const PSK_LEN: usize = 32;

/// QUIC configuration error.
#[derive(Debug, thiserror::Error)]
pub enum QuicConfigError {
    #[error("TLS configuration failed: {0}")]
    Tls(String),
    #[error("QUIC configuration failed: {0}")]
    Quic(#[from] quiche::Error),
}

/// Role in the QUIC handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuicRole {
    /// Client initiates the connection (caller).
    Client,
    /// Server accepts the connection (answerer).
    Server,
}

/// Derive a TLS 1.3 Pre-Shared Key from an X25519 shared secret.
///
/// Uses HKDF-SHA256 with a domain-specific salt and info string to derive
/// a 32-byte PSK suitable for TLS 1.3 authentication.
///
/// Both peers perform the same X25519 DH exchange, so they arrive at the
/// same shared secret and thus the same PSK.
pub fn derive_psk(shared_secret: &[u8; 32]) -> [u8; PSK_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(b"wispers-connect-quic-v1"), shared_secret);
    let mut psk = [0u8; PSK_LEN];
    hk.expand(b"tls13-psk", &mut psk)
        .expect("32 bytes is valid for HKDF-SHA256");
    psk
}

/// Create a QUIC configuration with PSK authentication.
///
/// # Arguments
/// * `psk` - The pre-shared key derived from X25519 DH exchange
/// * `role` - Whether this is a client (caller) or server (answerer)
pub fn create_config(psk: [u8; PSK_LEN], role: QuicRole) -> Result<quiche::Config, QuicConfigError> {
    // Create BoringSSL context with PSK callbacks
    let mut ssl_ctx = SslContextBuilder::new(SslMethod::tls())
        .map_err(|e| QuicConfigError::Tls(e.to_string()))?;

    // Wrap PSK in Arc for sharing between callbacks
    let psk = Arc::new(psk);

    match role {
        QuicRole::Client => {
            let psk_clone = Arc::clone(&psk);
            ssl_ctx.set_psk_client_callback(move |_ssl, _hint, identity, psk_out| {
                // Write identity (null-terminated)
                if identity.len() < PSK_IDENTITY.len() + 1 {
                    return Err(boring::error::ErrorStack::get());
                }
                identity[..PSK_IDENTITY.len()].copy_from_slice(PSK_IDENTITY);
                identity[PSK_IDENTITY.len()] = 0; // null terminator

                // Write PSK
                if psk_out.len() < PSK_LEN {
                    return Err(boring::error::ErrorStack::get());
                }
                psk_out[..PSK_LEN].copy_from_slice(psk_clone.as_ref());

                Ok(PSK_LEN)
            });
        }
        QuicRole::Server => {
            let psk_clone = Arc::clone(&psk);
            ssl_ctx.set_psk_server_callback(move |_ssl, identity, psk_out| {
                // Verify identity matches expected
                if identity != Some(PSK_IDENTITY) {
                    return Err(boring::error::ErrorStack::get());
                }

                // Write PSK
                if psk_out.len() < PSK_LEN {
                    return Err(boring::error::ErrorStack::get());
                }
                psk_out[..PSK_LEN].copy_from_slice(psk_clone.as_ref());

                Ok(PSK_LEN)
            });
        }
    }

    // Create quiche config from the SSL context
    let mut config = quiche::Config::with_boring_ssl_ctx_builder(QUIC_VERSION, ssl_ctx)?;

    // Set ALPN protocol
    config.set_application_protos(&[ALPN])?;

    // Disable certificate verification (we're using PSK)
    config.verify_peer(false);

    // Configure timeouts and flow control
    config.set_max_idle_timeout(MAX_IDLE_TIMEOUT_MS);
    config.set_initial_max_data(INITIAL_MAX_DATA);
    config.set_initial_max_stream_data_bidi_local(INITIAL_MAX_STREAM_DATA);
    config.set_initial_max_stream_data_bidi_remote(INITIAL_MAX_STREAM_DATA);
    config.set_initial_max_streams_bidi(INITIAL_MAX_STREAMS_BIDI);

    // Disable 0-RTT for security simplicity
    // (0-RTT data can be replayed)

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psk_derivation_deterministic() {
        let shared_secret = [42u8; 32];
        let psk1 = derive_psk(&shared_secret);
        let psk2 = derive_psk(&shared_secret);
        assert_eq!(psk1, psk2);
    }

    #[test]
    fn test_psk_derivation_different_secrets() {
        let psk1 = derive_psk(&[1u8; 32]);
        let psk2 = derive_psk(&[2u8; 32]);
        assert_ne!(psk1, psk2);
    }

    #[test]
    fn test_psk_length() {
        let psk = derive_psk(&[0u8; 32]);
        assert_eq!(psk.len(), 32);
    }

    #[test]
    fn test_psk_not_all_zeros() {
        let psk = derive_psk(&[0u8; 32]);
        assert!(psk.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_create_config_client() {
        let psk = derive_psk(&[42u8; 32]);
        let config = create_config(psk, QuicRole::Client);
        assert!(config.is_ok());
    }

    #[test]
    fn test_create_config_server() {
        let psk = derive_psk(&[42u8; 32]);
        let config = create_config(psk, QuicRole::Server);
        assert!(config.is_ok());
    }
}
