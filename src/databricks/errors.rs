#[derive(thiserror::Error, Debug)]
pub enum DatabricksApiError {
    #[error("Outgoing message serialization failed: {error}")]
    SerializationFailed { error: http_types::Error },
    #[error("Incoming message deserialization failed: {error}")]
    DeserializationFailed { error: surf::Error },
    #[error("Provided endpoint is invalid: '{uri}' {error}")]
    InvalidEndpoint {
        uri: String,
        error: surf::http::url::ParseError,
    },
    #[error("Server returned unexpected status code: {status_code}")]
    InvalidStatus { status_code: surf::StatusCode },
    #[error("Request failed: {error}")]
    RequestFailed { error: surf::Error },
}

pub fn raise_serialization_failed<T>(error: http_types::Error) -> Result<T, DatabricksApiError> {
    Err(DatabricksApiError::SerializationFailed { error: error })
}

pub fn raise_deserialization_failed<T>(error: surf::Error) -> Result<T, DatabricksApiError> {
    Err(DatabricksApiError::DeserializationFailed { error: error })
}

pub fn raise_invalid_endpoint<T>(uri: &str, error: surf::http::url::ParseError) -> Result<T, DatabricksApiError> {
    Err(DatabricksApiError::InvalidEndpoint {
        uri: String::from(uri),
        error: error,
    })
}

pub fn raise_invalid_status<T>(status_code: surf::StatusCode) -> Result<T, DatabricksApiError> {
    Err(DatabricksApiError::InvalidStatus {
        status_code: status_code,
    })
}

pub fn raise_request_failed<T>(error: surf::Error) -> Result<T, DatabricksApiError> {
    Err(DatabricksApiError::RequestFailed { error: error })
}
