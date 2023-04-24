#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksContextCreateRequest {
    #[serde(rename = "clusterId")]
    pub cluster_id: String,
    pub language: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksContextCreateResponse {
    pub id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksCommandExecuteRequest {
    #[serde(rename = "clusterId")]
    pub cluster_id: String,
    #[serde(rename = "contextId")]
    pub context_id: String,
    pub language: String,
    pub command: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksCommandExecuteResponse {
    pub id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksCommandStatusRequest {
    #[serde(rename = "clusterId")]
    pub cluster_id: String,
    #[serde(rename = "contextId")]
    pub context_id: String,
    #[serde(rename = "commandId")]
    pub command_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksCommandStatusResponse {
    pub id: String,
    pub status: String,
    pub results: Option<DatabricksCommandStatusResponseResult>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct DatabricksCommandStatusResponseResult {
    #[serde(rename = "resultType")]
    pub result_type: String,
    pub data: Option<String>,
    pub summary: Option<String>,
    pub cause: Option<String>,
}
