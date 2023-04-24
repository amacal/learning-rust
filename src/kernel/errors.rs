use crate::databricks;
use crate::environment;
use crate::jupyter;

#[derive(thiserror::Error, Debug)]
pub enum KernelError {
    #[error("Connecting failed: {error:?}")]
    ConnectingFailed { error: jupyter::JupyterError },
    #[error("Databricks failed: {error:?}")]
    DatabricksCredentialsFailed {
        error: environment::DatabricksCredentialsError,
    },
    #[error("Databricks failed: {error:?}")]
    DatabricksApiFailed {
        error: databricks::DatabricksApiError,
    },
    #[error("Receiving failed: {error:?}")]
    ReceivingFailed { error: jupyter::JupyterError },
    #[error("Sending failed: {error:?}")]
    SendingFailed { error: jupyter::JupyterError },
    #[error("Incomplete payload: {content}")]
    PayloadIncomplete { content: String },
}

pub fn raise_connecting_failed<T>(error: jupyter::JupyterError) -> Result<T, KernelError> {
    return Err(KernelError::ConnectingFailed { error: error });
}

pub fn raise_databricks_credentials_failed<T>(error: environment::DatabricksCredentialsError) -> Result<T, KernelError> {
    return Err(KernelError::DatabricksCredentialsFailed { error: error });
}

pub fn raise_databricks_api_failed<T>(error: databricks::DatabricksApiError) -> Result<T, KernelError> {
    return Err(KernelError::DatabricksApiFailed { error: error });
}

pub fn raise_receiving_failed<T>(error: jupyter::JupyterError) -> Result<T, KernelError> {
    return Err(KernelError::ReceivingFailed { error: error });
}

pub fn raise_sending_failed<T>(error: jupyter::JupyterError) -> Result<T, KernelError> {
    return Err(KernelError::SendingFailed { error: error });
}

pub fn raise_payload_incomplete<T>(content: &str) -> Result<T, KernelError> {
    return Err(KernelError::PayloadIncomplete {
        content: String::from(content),
    });
}
