use std::env;
use thiserror::Error;

#[derive(Debug)]
pub struct DatabricksCredentials {
    pub host: String,
    pub token: String,
}

#[derive(Error, Debug)]
pub enum DatabricksCredentialsError {
    #[error("Missing or invalid DATABRICKS_HOST variable")]
    MissingHost,
    #[error("Missing or invalid DATABRICKS_TOKEN variable")]
    MissingToken,
}

pub fn find_databricks_credentials() -> Result<DatabricksCredentials, DatabricksCredentialsError> {
    let host = match env::var("DATABRICKS_HOST") {
        Ok(value) if value.len() > 0 => value,
        _ => return Err(DatabricksCredentialsError::MissingHost),
    };

    let token = match env::var("DATABRICKS_TOKEN") {
        Ok(value) if value.len() > 0 => value,
        _ => return Err(DatabricksCredentialsError::MissingToken),
    };

    return Ok(DatabricksCredentials {
        host: host,
        token: token,
    });
}
