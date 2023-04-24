use crate::jupyter::client::JupyterChannel;

#[derive(thiserror::Error, Debug)]
pub enum JupyterError {
    #[error("{name} cannot be deserialized: {error}")]
    DeserializationFailed { name: String, error: serde_json::Error },
    #[error("{name} cannot be serialized: {error}")]
    SerializationFailed { name: String, error: serde_json::Error },
    #[error("Message verification failed: {reason}")]
    VerificationFailed { reason: String },
    #[error("Reading connection file '{path}' failed: {reason}")]
    ConnectionFileFailed { path: String, reason: String },
    #[error("Handling connection socket '{channel:?}' failed: {error}")]
    ConnectionSocketFailed {
        channel: JupyterChannel,
        error: zeromq::ZmqError,
    },
    #[error("Queuing outgoing message on '{channel:?}' failed: {error}")]
    QueueingFailed {
        channel: JupyterChannel,
        error: tokio::sync::mpsc::error::SendError<(JupyterChannel, zeromq::ZmqMessage)>,
    },
    #[error("Found {found} number of frames, but {expected} expected")]
    NotEnoughFrames { found: usize, expected: usize },
    #[error("Message is incomplete because '{name}' was not provided")]
    MessageIncomplete { name: String },
    #[error("Invalid signature from received frames: {identifier}")]
    InvalidSignature { identifier: String },
    #[error("Invalid implemented code create some issue: {reason}")]
    InvalidImplementation { reason: String },
}

pub fn raise_deserialization_failed<T>(name: &str, error: serde_json::Error) -> Result<T, JupyterError> {
    return Err(JupyterError::DeserializationFailed {
        name: String::from(name),
        error: error,
    });
}

pub fn raise_serialization_failed<T>(name: &str, error: serde_json::Error) -> Result<T, JupyterError> {
    return Err(JupyterError::SerializationFailed {
        name: String::from(name),
        error: error,
    });
}

pub fn raise_verification_failed<T>(reason: String) -> Result<T, JupyterError> {
    return Err(JupyterError::VerificationFailed { reason: reason });
}

pub fn raise_connection_file_failed<T>(path: &str, reason: String) -> Result<T, JupyterError> {
    return Err(JupyterError::ConnectionFileFailed {
        path: String::from(path),
        reason: reason,
    });
}

pub fn raise_connection_socket_failed<T>(channel: JupyterChannel, error: zeromq::ZmqError) -> Result<T, JupyterError> {
    return Err(JupyterError::ConnectionSocketFailed {
        channel: channel,
        error: error,
    });
}

pub fn raise_queueing_failed<T>(
    channel: JupyterChannel,
    error: tokio::sync::mpsc::error::SendError<(JupyterChannel, zeromq::ZmqMessage)>,
) -> Result<T, JupyterError> {
    return Err(JupyterError::QueueingFailed {
        channel: channel,
        error: error,
    });
}

pub fn raise_not_enough_frames<T>(found: usize, expected: usize) -> Result<T, JupyterError> {
    return Err(JupyterError::NotEnoughFrames {
        found: found,
        expected: expected,
    });
}

pub fn raise_message_incomplete<T>(name: &str) -> Result<T, JupyterError> {
    return Err(JupyterError::MessageIncomplete {
        name: String::from(name),
    });
}

pub fn raise_invalid_signature<T>(identifier: &bytes::Bytes) -> Result<T, JupyterError> {
    return Err(JupyterError::InvalidSignature {
        identifier: hex::encode(identifier),
    });
}

pub fn raise_invalid_implementation<T>(reason: String) -> Result<T, JupyterError> {
    return Err(JupyterError::InvalidImplementation { reason: reason });
}
