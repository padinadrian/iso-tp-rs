//! Defines the error type for this library.

/* ========== Includes ========== */
use thiserror::Error;

/* ========== Enums ========== */

/// Error type for ISO-TP library.
#[derive(Error, Debug)]
pub enum Error {
    #[error("received more data (`{0}`) than expected (`{1}`)")]
    Overflow(u16, u16),
    #[error("missed frame; expected index `{0}`, received index `{1}`")]
    MissedFrame(u8, u8),
    #[error("internal buffer (`{0}`) is smaller than expected transfer length (`{1}`)")]
    BufferTooSmall(u16, u16),
}

/// Result type for ISO-TP library.
pub type Result<T> = std::result::Result<T, Error>;
