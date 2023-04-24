use log::debug;

use crate::kernel::errors::*;
use crate::{databricks, jupyter};

pub struct JupyterKernelInfoReplySender {
    channel: jupyter::JupyterChannel,
    identity: bytes::Bytes,
    header: jupyter::JupyterHeader,
    parent: jupyter::JupyterHeader,
    status: String,
    banner: Option<String>,
    protocol: Option<String>,
    implementation: Option<(String, String)>,
    language: Option<(String, String, String, String)>,
}

pub struct JupyterExecuteReplySender {
    channel: jupyter::JupyterChannel,
    identity: bytes::Bytes,
    header: jupyter::JupyterHeader,
    parent: jupyter::JupyterHeader,
    status: String,
    execution_count: Option<u32>,
}

pub struct JupyterExecuteInputSender {
    header: jupyter::JupyterHeader,
    parent: jupyter::JupyterHeader,
    code: Option<String>,
    execution_count: Option<u32>,
}

pub struct JupyterExecuteResultSender {
    header: jupyter::JupyterHeader,
    parent: jupyter::JupyterHeader,
    execution_count: Option<u32>,
    data: std::collections::HashMap<String, serde_json::Value>,
    metadata: std::collections::HashMap<String, serde_json::Value>,
}

pub struct JupyterKernelStatusSender {
    header: jupyter::JupyterHeader,
    parent: jupyter::JupyterHeader,
    execution_state: Option<String>,
}

pub struct JupyterInputRequestSender {
    header: jupyter::JupyterHeader,
    prompt: Option<String>,
    password: bool,
}

pub struct DatabricksContextCreateSender {
    cluster_id: Option<String>,
    language: Option<String>,
}

pub struct DatabricksCommandExecuteSender {
    cluster_id: Option<String>,
    context_id: Option<String>,
    language: Option<String>,
    command: Option<String>,
}

pub struct DatabricksCommandStatusSender {
    cluster_id: Option<String>,
    context_id: Option<String>,
    command_id: Option<String>,
}

impl JupyterKernelInfoReplySender {
    pub fn new(channel: jupyter::JupyterChannel, identity: bytes::Bytes, parent: jupyter::JupyterHeader) -> Self {
        Self {
            channel: channel,
            identity: identity,
            header: parent.reply("kernel_info_reply"),
            parent: parent,
            status: String::from("ok"),
            banner: None,
            protocol: None,
            implementation: None,
            language: None,
        }
    }

    pub fn with_status(self, status: &str) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: String::from(status),
            banner: self.banner,
            protocol: self.protocol,
            implementation: self.implementation,
            language: self.language,
        }
    }

    pub fn with_banner(self, banner: &str) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: self.status,
            banner: Some(String::from(banner)),
            protocol: self.protocol,
            implementation: self.implementation,
            language: self.language,
        }
    }

    pub fn with_protocol(self, protocol_version: &str) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: self.status,
            protocol: Some(String::from(protocol_version)),
            implementation: self.implementation,
            language: self.language,
            banner: self.banner,
        }
    }

    pub fn with_implementation(self, name: &str, version: &str) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: self.status,
            protocol: self.protocol,
            implementation: Some((String::from(name), String::from(version))),
            language: self.language,
            banner: self.banner,
        }
    }

    pub fn with_language(self, name: &str, version: &str, mimetype: &str, extension: &str) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: self.status,
            protocol: self.protocol,
            implementation: self.implementation,
            language: Some((
                String::from(name),
                String::from(version),
                String::from(mimetype),
                String::from(extension),
            )),
            banner: self.banner,
        }
    }

    pub async fn execute(self, client: &mut jupyter::JupyterClient) -> Result<(), KernelError> {
        match self {
            JupyterKernelInfoReplySender {
                channel,
                identity,
                header,
                parent,
                status,
                banner: Some(banner),
                protocol: Some(protocol_version),
                implementation: Some((implementation_name, implementation_version)),
                language: Some((language_name, language_version, laguage_mimetype, language_extension)),
            } => {
                let content = &jupyter::JupyterKernelInfoReply {
                    status: status,
                    protocol_version: protocol_version,
                    implementation: implementation_name,
                    implementation_version: implementation_version,
                    banner: banner,
                    language_info: jupyter::JupyterKernelLanguageInfo {
                        name: language_name,
                        version: language_version,
                        mimetype: laguage_mimetype,
                        file_extension: language_extension,
                        pygments_lexer: None,
                        codemirror_mode: None,
                        nbconvert_exporter: None,
                    },
                    debugger: false,
                    help_links: vec![],
                };

                debug!("Sending {:?} {:?} ...", &header, &content);
                match client.send(channel, &identity, &header, Some(&parent), &content).await {
                    Ok(()) => Ok(()),
                    Err(error) => raise_sending_failed(error),
                }
            }
            _ => raise_payload_incomplete("JupyterKernelInfoReply"),
        }
    }
}

impl JupyterExecuteReplySender {
    pub fn new(channel: jupyter::JupyterChannel, identity: bytes::Bytes, parent: jupyter::JupyterHeader) -> Self {
        Self {
            channel: channel,
            identity: identity,
            header: parent.reply("execute_reply"),
            parent: parent,
            status: String::from("ok"),
            execution_count: None,
        }
    }

    pub fn with_status(self, status: &str) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: String::from(status),
            execution_count: self.execution_count,
        }
    }

    pub fn with_execution_count(self, execution_count: u32) -> Self {
        Self {
            channel: self.channel,
            identity: self.identity,
            header: self.header,
            parent: self.parent,
            status: self.status,
            execution_count: Some(execution_count),
        }
    }

    pub async fn execute(self, client: &mut jupyter::JupyterClient) -> Result<(), KernelError> {
        match self {
            JupyterExecuteReplySender {
                channel,
                identity,
                header,
                parent,
                status,
                execution_count: Some(execution_count),
            } => {
                let content = &jupyter::JupyterExecuteReply {
                    status: status,
                    execution_count: execution_count,
                };

                debug!("Sending {:?} {:?} ...", &header, &content);
                match client.send(channel, &identity, &header, Some(&parent), &content).await {
                    Ok(()) => Ok(()),
                    Err(error) => raise_sending_failed(error),
                }
            }
            _ => raise_payload_incomplete("JupyterExecuteReply"),
        }
    }
}

impl JupyterExecuteInputSender {
    pub fn new(parent: jupyter::JupyterHeader) -> Self {
        Self {
            header: parent.reply("execute_input"),
            parent: parent,
            code: None,
            execution_count: None,
        }
    }

    pub fn with_code(self, code: &str) -> Self {
        Self {
            header: self.header,
            parent: self.parent,
            code: Some(String::from(code)),
            execution_count: self.execution_count,
        }
    }

    pub fn with_execution_count(self, execution_count: u32) -> Self {
        Self {
            header: self.header,
            parent: self.parent,
            code: self.code,
            execution_count: Some(execution_count),
        }
    }

    pub async fn execute(self, client: &mut jupyter::JupyterClient) -> Result<(), KernelError> {
        match self {
            JupyterExecuteInputSender {
                header,
                parent,
                code: Some(code),
                execution_count: Some(execution_count),
            } => {
                let content = &jupyter::JupyterExecuteInput {
                    code: code,
                    execution_count: execution_count,
                };

                let identity = bytes::Bytes::new();
                let channel = jupyter::JupyterChannel::IOPub;

                debug!("Sending {:?} {:?} ...", &header, &content);
                match client.send(channel, &identity, &header, Some(&parent), &content).await {
                    Ok(()) => Ok(()),
                    Err(error) => raise_sending_failed(error),
                }
            }
            _ => raise_payload_incomplete("JupyterExecuteInput"),
        }
    }
}

impl JupyterExecuteResultSender {
    pub fn new(parent: jupyter::JupyterHeader) -> Self {
        Self {
            header: parent.reply("execute_result"),
            parent: parent,
            execution_count: None,
            data: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_execution_count(self, execution_count: u32) -> Self {
        Self {
            header: self.header,
            parent: self.parent,
            execution_count: Some(execution_count),
            data: self.data,
            metadata: self.metadata,
        }
    }

    pub fn with_data_string(mut self, mimetype: &str, data: &str) -> Self {
        self.data
            .insert(String::from(mimetype), serde_json::Value::String(String::from(data)));

        Self {
            header: self.header,
            parent: self.parent,
            execution_count: self.execution_count,
            data: self.data,
            metadata: self.metadata,
        }
    }

    pub async fn execute(self, client: &mut jupyter::JupyterClient) -> Result<(), KernelError> {
        match self {
            JupyterExecuteResultSender {
                header,
                parent,
                execution_count: Some(execution_count),
                data,
                metadata,
            } => {
                let content = &jupyter::JupyterExecuteResult {
                    execution_count: execution_count,
                    data: data,
                    metadata: metadata,
                };

                let identity = bytes::Bytes::new();
                let channel = jupyter::JupyterChannel::IOPub;

                debug!("Sending {:?} {:?} ...", &header, &content);
                match client.send(channel, &identity, &header, Some(&parent), &content).await {
                    Ok(()) => Ok(()),
                    Err(error) => raise_sending_failed(error),
                }
            }
            _ => raise_payload_incomplete("JupyterExecuteResult"),
        }
    }
}

impl JupyterKernelStatusSender {
    pub fn new(parent: jupyter::JupyterHeader) -> Self {
        Self {
            header: parent.reply("status"),
            parent: parent,
            execution_state: None,
        }
    }

    pub fn with_status(self, status: &str) -> Self {
        Self {
            header: self.header,
            parent: self.parent,
            execution_state: Some(String::from(status)),
        }
    }

    pub async fn execute(self, client: &mut jupyter::JupyterClient) -> Result<(), KernelError> {
        match self {
            JupyterKernelStatusSender {
                header,
                parent,
                execution_state: Some(execution_state),
            } => {
                let content = &jupyter::JupyterKernelStatus {
                    execution_state: execution_state,
                };

                let identity = bytes::Bytes::new();
                let channel = jupyter::JupyterChannel::IOPub;

                debug!("Sending {:?} {:?} ...", &header, &content);
                match client.send(channel, &identity, &header, Some(&parent), &content).await {
                    Ok(()) => Ok(()),
                    Err(error) => raise_sending_failed(error),
                }
            }
            _ => raise_payload_incomplete("JupyterKernelStatusSender"),
        }
    }
}

impl JupyterInputRequestSender {
    pub fn new(parent: jupyter::JupyterHeader) -> Self {
        Self {
            header: parent.reply("input_request"),
            prompt: None,
            password: false,
        }
    }

    pub fn with_prompt(self, prompt: &str) -> Self {
        Self {
            header: self.header,
            prompt: Some(String::from(prompt)),
            password: self.password,
        }
    }

    pub async fn execute(self, client: &mut jupyter::JupyterClient) -> Result<(), KernelError> {
        match self {
            JupyterInputRequestSender {
                header,
                prompt: Some(prompt),
                password,
            } => {
                let content = &jupyter::JupyterInputRequest {
                    prompt: prompt,
                    password: password,
                };

                let identity = bytes::Bytes::new();
                let channel = jupyter::JupyterChannel::StdIn;

                debug!("Sending {:?} {:?} ...", &header, &content);
                match client.send(channel, &identity, &header, None, &content).await {
                    Ok(()) => Ok(()),
                    Err(error) => raise_sending_failed(error),
                }
            }
            _ => raise_payload_incomplete("JupyterInputRequestSender"),
        }
    }
}

impl DatabricksContextCreateSender {
    pub fn new() -> Self {
        Self {
            cluster_id: None,
            language: None,
        }
    }

    pub fn with_cluster_id(self, cluster_id: &str) -> Self {
        Self {
            cluster_id: Some(String::from(cluster_id)),
            language: self.language,
        }
    }

    pub fn with_language(self, language: &str) -> Self {
        Self {
            cluster_id: self.cluster_id,
            language: Some(String::from(language)),
        }
    }

    pub async fn execute(
        self,
        client: &databricks::DatabricksClient,
    ) -> Result<databricks::DatabricksContextCreateResponse, KernelError> {
        match self {
            DatabricksContextCreateSender {
                cluster_id: Some(cluster_id),
                language: Some(language),
            } => {
                let request = databricks::DatabricksContextCreateRequest {
                    cluster_id: cluster_id,
                    language: language,
                };

                let response = match client.create_context(request).await {
                    Ok(response) => response,
                    Err(error) => return raise_databricks_api_failed(error),
                };

                debug!("Execution Context ID: {}", response.id);
                Ok(response)
            }
            _ => raise_payload_incomplete("DatabricksContextCreateSender"),
        }
    }
}

impl DatabricksCommandExecuteSender {
    pub fn new() -> Self {
        Self {
            cluster_id: None,
            context_id: None,
            language: None,
            command: None,
        }
    }

    pub fn with_cluster_id(self, cluster_id: &str) -> Self {
        Self {
            cluster_id: Some(String::from(cluster_id)),
            context_id: self.context_id,
            language: self.language,
            command: self.command,
        }
    }

    pub fn with_context_id(self, context_id: &str) -> Self {
        Self {
            cluster_id: self.cluster_id,
            context_id: Some(String::from(context_id)),
            language: self.language,
            command: self.command,
        }
    }

    pub fn with_language(self, language: &str) -> Self {
        Self {
            cluster_id: self.cluster_id,
            context_id: self.context_id,
            language: Some(String::from(language)),
            command: self.command,
        }
    }

    pub fn with_command(self, command: &str) -> Self {
        Self {
            cluster_id: self.cluster_id,
            context_id: self.context_id,
            language: self.language,
            command: Some(String::from(command)),
        }
    }

    pub async fn execute(
        self,
        client: &databricks::DatabricksClient,
    ) -> Result<databricks::DatabricksCommandExecuteResponse, KernelError> {
        match self {
            DatabricksCommandExecuteSender {
                cluster_id: Some(cluster_id),
                context_id: Some(context_id),
                language: Some(language),
                command: Some(command),
            } => {
                let request = databricks::DatabricksCommandExecuteRequest {
                    cluster_id: cluster_id,
                    context_id: context_id,
                    language: language,
                    command: command,
                };

                let response = match client.execute_command(request).await {
                    Ok(response) => response,
                    Err(error) => return raise_databricks_api_failed(error),
                };

                debug!("Executed Command ID: {}", response.id);
                Ok(response)
            }
            _ => raise_payload_incomplete("DatabricksCommandExecuteSender"),
        }
    }
}

impl DatabricksCommandStatusSender {
    pub fn new() -> Self {
        Self {
            cluster_id: None,
            context_id: None,
            command_id: None,
        }
    }

    pub fn with_cluster_id(self, cluster_id: &str) -> Self {
        Self {
            cluster_id: Some(String::from(cluster_id)),
            context_id: self.context_id,
            command_id: self.command_id,
        }
    }

    pub fn with_context_id(self, context_id: &str) -> Self {
        Self {
            cluster_id: self.cluster_id,
            context_id: Some(String::from(context_id)),
            command_id: self.command_id,
        }
    }

    pub fn with_command_id(self, command_id: &str) -> Self {
        Self {
            cluster_id: self.cluster_id,
            context_id: self.context_id,
            command_id: Some(String::from(command_id)),
        }
    }

    pub async fn execute(
        self,
        client: &databricks::DatabricksClient,
    ) -> Result<databricks::DatabricksCommandStatusResponse, KernelError> {
        match self {
            DatabricksCommandStatusSender {
                cluster_id: Some(cluster_id),
                context_id: Some(context_id),
                command_id: Some(command_id),
            } => {
                let request = databricks::DatabricksCommandStatusRequest {
                    cluster_id: cluster_id,
                    context_id: context_id,
                    command_id: command_id,
                };

                let response = match client.status_command(request).await {
                    Ok(response) => response,
                    Err(error) => return raise_databricks_api_failed(error),
                };

                debug!("Executed Command ID: {} / {}", response.id, response.status);
                Ok(response)
            }
            _ => raise_payload_incomplete("DatabricksCommandStatusSender"),
        }
    }
}
