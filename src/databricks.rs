use serde::{Deserialize, Serialize};
use surf::{http::url::ParseError, Error as SurfError, RequestBuilder, StatusCode, Url};
use thiserror::Error;

#[derive(Deserialize, Debug)]
pub struct DbfsFileInfo {
    pub path: String,
    pub is_dir: bool,
    pub file_size: usize,
    pub modification_time: i64,
}

#[derive(Serialize, Debug)]
pub struct DbfsListRequest {
    pub path: String,
}

#[derive(Deserialize, Debug)]
pub struct DbfsListResponse {
    pub files: Option<Vec<DbfsFileInfo>>,
}

#[derive(Serialize, Debug)]
pub struct DbfsStatusRequest<'a> {
    pub path: &'a str,
}

pub type DbfsStatusResponse = DbfsFileInfo;

#[derive(Serialize, Debug)]
pub struct DbfsReadRequest<'a> {
    pub path: &'a str,
    pub offset: usize,
    pub length: usize,
}

#[derive(Deserialize, Debug)]
pub struct DbfsReadResponse {
    pub bytes_read: usize,
    pub data: String,
}

#[derive(Error, Debug)]
pub enum DatabricksApiError {
    #[error("Outgoing message serialization failed")]
    SerializationFailed,
    #[error("Incoming message deserialization failed: {0}")]
    DeserializationFailed(SurfError),
    #[error("Provided endpoint is invalid: '{0}' {1}")]
    InvalidEndpoint(String, ParseError),
    #[error("Server returned unexpected status code: {0}")]
    InvalidStatus(StatusCode),
    #[error("Request failed: {0}")]
    RequestFailed(SurfError),
}

#[derive(Clone)]
pub struct DatabricksApiClient {
    host: String,
    token: String,
}

impl DatabricksApiClient {
    pub fn new(host: String, token: String) -> Self {
        return Self {
            host: host,
            token: token,
        };
    }

    fn prepare_request(
        &self,
        method: surf::http::Method,
        endpoint: &str,
    ) -> Result<RequestBuilder, DatabricksApiError> {
        let authorization = format!("Bearer {}", self.token);
        let uri = format!("https://{}/{}", self.host, endpoint);

        let url = Url::parse(&uri).map_err(|error| DatabricksApiError::InvalidEndpoint(uri, error))?;
        let builder = RequestBuilder::new(method, url).header("Authorization", authorization);

        Ok(builder)
    }

    async fn execute<TRequest: Serialize, TResponse: for<'a> Deserialize<'a>>(
        &self,
        method: surf::http::Method,
        endpoint: &str,
        data: &TRequest,
    ) -> Result<TResponse, DatabricksApiError> {
        let request = self
            .prepare_request(method, endpoint)?
            .body_json(data)
            .map_err(|_| DatabricksApiError::SerializationFailed)?;

        let mut response = match request.await {
            Ok(response) if response.status() == 200 => response,
            Ok(response) => return Err(DatabricksApiError::InvalidStatus(response.status())),
            Err(error) => return Err(DatabricksApiError::RequestFailed(error)),
        };

        return Ok(response
            .body_json()
            .await
            .map_err(DatabricksApiError::DeserializationFailed)?);
    }
}

impl DatabricksApiClient {
    pub async fn dbfs_list(&self, path: String) -> Result<DbfsListResponse, DatabricksApiError> {
        let method = surf::http::Method::Get;
        let request = DbfsListRequest { path: path };

        return self.execute(method, "api/2.0/dbfs/list", &request).await;
    }

    pub async fn dbfs_status(&self, path: &str) -> Result<DbfsStatusResponse, DatabricksApiError> {
        let method = surf::http::Method::Get;
        let request = DbfsStatusRequest { path: path };

        return self.execute(method, "api/2.0/dbfs/get-status", &request).await;
    }

    pub async fn dbfs_read(
        &self,
        path: &str,
        offset: usize,
        length: usize,
    ) -> Result<DbfsReadResponse, DatabricksApiError> {
        let method = surf::http::Method::Get;
        let request = DbfsReadRequest {
            path: path,
            offset: offset,
            length: length,
        };

        return self.execute(method, "api/2.0/dbfs/read", &request).await;
    }
}
