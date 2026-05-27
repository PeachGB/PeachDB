use thiserror::Error;

#[derive(Error, Debug)]
pub enum PeachDbError {
    // ============================================================================
    // I/O Errors
    // ============================================================================
    /// Wraps standard I/O errors from file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // Codec Errors
    /// Invalid magic bytes detected in file header or Record.
    #[error("invalid magic bytes")]
    InvalidMagicBytes,
    /// Invalid index entry format or content (e.g., malformed offset or length).
    #[error("invalid index entry")]
    InvalidIndexEntry,

    /// Unknown data type byte encountered during deserialization.
    #[error("unknown data type byte: {byte:#x}")]
    UnknownDtype { byte: u8 },

    /// Record detected as corrupted (e.g., CRC32 mismatch or corrupted structure).
    #[error("corrupted record: {reason}")]
    CorruptedRecord { reason: String },

    ///Calling finish() on unfinished object
    #[error("Invalid finish() call, Called on unfinished object")]
    InvalidFinishCall,

    /// Buffer too short to contain expected data.
    #[error("buffer too short")]
    BufferTooShort,
    ///Empty Buffer
    #[error("Empty Buffer")]
    EmptyBuffer,

    // WAL Errors
    /// Incomplete WAL entry (e.g., missing COMMIT marker).
    #[error("incomplete WAL entry: {reason}")]
    IncompleteEntry { reason: String },

    /// WAL replay failed during recovery.
    #[error("WAL replay failed: {reason}")]
    ReplayFailed { reason: String },

    // Protocol Errors
    /// Unknown command byte in client request.
    #[error("unknown command byte: {byte:#x}")]
    UnknownCommand { byte: u8 },

    /// Invalid request format or malformed message.
    #[error("invalid request format: {reason}")]
    InvalidRequestFormat { reason: String },

    /// Client payload exceeds maximum allowed size.
    #[error("payload too large: {size} bytes exceeds maximum {max} bytes")]
    PayloadTooLarge { size: usize, max: usize },

    // DB Errors
    #[error("DataBase Name to long; must be less than 64 Bytes")]
    DBNameToLong,
    /// Key does not exist in the database.
    #[error("key not found")]
    KeyNotFoundt,

    /// Type mismatch when reading a value (e.g., GET on wrong type).
    #[error("type mismatch for key '{key}': expected {expected}, found {found}")]
    TypeMismatch {
        key: String,
        expected: String,
        found: String,
    },
    #[error("invalid WAL entry")]
    InvalidWalEntry,
}

impl PeachDbError {
    /// Returns true if this error is transient and retry might succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PeachDbError::Io(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::Interrupted | std::io::ErrorKind::TimedOut
                )
        )
    }

    /// Returns true if this error indicates a protocol violation (client fault).
    pub fn is_protocol_error(&self) -> bool {
        matches!(
            self,
            PeachDbError::UnknownCommand { .. }
                | PeachDbError::InvalidRequestFormat { .. }
                | PeachDbError::PayloadTooLarge { .. }
                | PeachDbError::UnknownDtype { .. }
        )
    }
}
