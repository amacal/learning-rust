#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterKernelInfoRequest {}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterKernelInfoReply {
    pub status: String,
    pub protocol_version: String,
    pub implementation: String,
    pub implementation_version: String,
    pub language_info: JupyterKernelLanguageInfo,
    pub banner: String,
    pub debugger: bool,
    pub help_links: Vec<JupyterKernelHelpLink>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterKernelLanguageInfo {
    pub name: String,
    pub version: String,
    pub mimetype: String,
    pub file_extension: String,
    pub pygments_lexer: Option<String>,
    pub codemirror_mode: Option<serde_json::Value>,
    pub nbconvert_exporter: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterKernelHelpLink {
    pub text: String,
    pub url: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterKernelStatus {
    pub execution_state: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterExecuteInput {
    pub code: String,
    pub execution_count: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterExecuteRequest {
    pub code: String,
    pub silent: bool,
    pub store_history: bool,
    pub user_expressions: std::collections::HashMap<String, String>,
    pub allow_stdin: bool,
    pub stop_on_error: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterExecuteResult {
    pub execution_count: u32,
    pub data: std::collections::HashMap<String, serde_json::Value>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterDisplayData {
    pub execution_count: u32,
    pub data: std::collections::HashMap<String, serde_json::Value>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub transient: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterExecuteReply {
    pub status: String,
    pub execution_count: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterInputRequest {
    pub prompt: String,
    pub password: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JupyterInputReply {
    pub value: String,
}