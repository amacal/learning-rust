mod client;
mod errors;
mod messages;

use std::fmt::Debug;

use log::info;
use serde::{Deserialize, Serialize};
use surf::{RequestBuilder, Url};

use crate::databricks::errors::*;

pub use crate::databricks::messages::*;
pub use crate::databricks::errors::DatabricksApiError;

#[derive(Clone)]
pub struct DatabricksClient {
    host: String,
    token: String,
}

impl DatabricksClient {
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

        let url = match Url::parse(&uri) {
            Ok(url) => url,
            Err(error) => return raise_invalid_endpoint(&uri, error),
        };

        Ok(RequestBuilder::new(method, url).header("Authorization", authorization))
    }

    async fn execute<TRequest: Serialize + Debug, TResponse: for<'a> Deserialize<'a>>(
        &self,
        method: surf::http::Method,
        endpoint: &str,
        data: &TRequest,
    ) -> Result<TResponse, DatabricksApiError> {
        info!("Sending {} to {} with {:?}", method, endpoint, data);
        let builder = self.prepare_request(method, endpoint)?;

        let request = match method {
            http_types::Method::Get => builder.query(&data),
            _ => builder.body_json(data)
        };

        let request = match request {
            Ok(request) => request,
            Err(error) => return raise_serialization_failed(error),
        };

        let mut response = match request.await {
            Ok(response) if response.status() == 200 => response,
            Ok(response) => return raise_invalid_status(response.status()),
            Err(error) => return raise_request_failed(error),
        };

        let result = match response.body_json().await {
            Ok(result) => result,
            Err(error) => return raise_deserialization_failed(error),
        };

        return Ok(result);
    }
}

impl DatabricksClient {
    pub async fn create_context(
        &self,
        request: DatabricksContextCreateRequest,
    ) -> Result<DatabricksContextCreateResponse, DatabricksApiError> {
        return self
            .execute(surf::http::Method::Post, "api/1.2/contexts/create", &request)
            .await;
    }

    pub async fn execute_command(
        &self,
        request: DatabricksCommandExecuteRequest,
    ) -> Result<DatabricksCommandExecuteResponse, DatabricksApiError> {
        return self
            .execute(surf::http::Method::Post, "api/1.2/commands/execute", &request)
            .await;
    }

    pub async fn status_command(
        &self,
        request: DatabricksCommandStatusRequest,
    ) -> Result<DatabricksCommandStatusResponse, DatabricksApiError> {
        return self
            .execute(surf::http::Method::Get, "api/1.2/commands/status", &request)
            .await;
    }
}
