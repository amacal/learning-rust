use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::databricks;
use crate::environment;
use crate::jupyter;
use crate::kernel::errors::*;
use crate::kernel::senders::*;

use log::{debug, info, warn};

pub struct KernelClient {
    counter: Arc<Mutex<u32>>,
    sessions: Arc<Mutex<HashMap<String, String>>>,

    jupyter: jupyter::JupyterClient,
    databricks: databricks::DatabricksClient,

    sender: mpsc::Sender<KernelRequest>,
    receiver: mpsc::Receiver<KernelRequest>,
}

#[derive(Debug)]
pub struct KernelRequest {
    channel: jupyter::JupyterChannel,
    sender: bytes::Bytes,
    header: jupyter::JupyterHeader,
    execution_count: u32,
    code: String,
}

impl KernelClient {
    pub async fn start(path: &str) -> Result<Self, KernelError> {
        let connection = match jupyter::read_connection(path).await {
            Ok(connection) => connection,
            Err(error) => return raise_connecting_failed(error),
        };

        let jupyter = match jupyter::JupyterClient::connect(connection).await {
            Ok(client) => client,
            Err(error) => return raise_connecting_failed(error),
        };

        let databricks = match environment::find_databricks_credentials() {
            Ok(credentials) => databricks::DatabricksClient::new(credentials.host, credentials.token),
            Err(error) => return raise_databricks_credentials_failed(error),
        };

        let (sender, mut receiver) = mpsc::channel(100);

        let instance = Self {
            counter: Arc::new(Mutex::new(0u32)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            sender: sender,
            receiver: receiver,
            jupyter: jupyter,
            databricks: databricks,
        };

        Ok(instance)
    }

    /*
    fn spawn_databricks_executor(&self, receiver: &mut tokio::sync::mpsc::Receiver<KernelRequest>) {
            loop {
                let request = match receiver.recv().await {
                    None => break,
                    Some(request) => request,
                };

                info!("Received {:?}", request);

                };
            }

            Ok::<(), KernelError>(())
        }
    }*/

    async fn handle_heartbeat(&self, _: bytes::Bytes) -> Result<(), KernelError> {
        Ok(())
    }

    async fn handle_kernel_info_request(
        &mut self,
        channel: jupyter::JupyterChannel,
        sender: bytes::Bytes,
        header: jupyter::JupyterHeader,
        _: jupyter::JupyterKernelInfoRequest,
    ) -> Result<(), KernelError> {
        self.send_kernel_info_reply(channel, sender.clone(), header.clone())
            .with_status("ok")
            .with_banner(self.jupyter.get_kernel_name())
            .with_protocol("5.3")
            .with_implementation("Databricks", "12.2")
            .with_language("Python", "3.9.5", "text/x-python", ".py")
            .execute(&mut self.jupyter)
            .await?;

        self.send_status(header.clone())
            .with_status("starting")
            .execute(&mut self.jupyter)
            .await?;

        self.send_status(header.clone())
            .with_status("idle")
            .execute(&mut self.jupyter)
            .await?;

        Ok(())
    }

    async fn handle_execute_request(
        &mut self,
        channel: jupyter::JupyterChannel,
        sender: bytes::Bytes,
        header: jupyter::JupyterHeader,
        request: jupyter::JupyterExecuteRequest,
    ) -> Result<(), KernelError> {
        let execution_count = {
            let mut counter = self.counter.lock().await;
            *counter = *counter + 1;
            *counter
        };

        self.send_status(header.clone())
            .with_status("busy")
            .execute(&mut self.jupyter)
            .await?;

        self.send_execute_input(header.clone())
            .with_code(&request.code)
            .with_execution_count(execution_count)
            .execute(&mut self.jupyter)
            .await?;

        self.sender
            .send(KernelRequest {
                channel: channel,
                sender: sender.clone(),
                header: header.clone(),
                code: request.code.clone(),
                execution_count: execution_count,
            })
            .await;

        Ok(())
    }

    async fn handle_content(
        &mut self,
        channel: jupyter::JupyterChannel,
        sender: bytes::Bytes,
        content: jupyter::JupyterContent,
    ) -> Result<(), KernelError> {
        match content {
            jupyter::JupyterContent::KernelInfoRequest { header, request } => {
                debug!("Handling {:?} -> {:?} ...", header, request);
                self.handle_kernel_info_request(channel, sender, header, request)
                    .await?
            }
            jupyter::JupyterContent::ExecuteRequest { header, request } => {
                debug!("Handling {:?} -> {:?} ...", header, request);
                self.handle_execute_request(channel, sender, header, request).await?
            }
            jupyter::JupyterContent::InputReply { parent, header, reply } => {
                warn!("Received input reply: {:?} {:?} {:?}", header, parent, reply)
            }
            jupyter::JupyterContent::Unknown { header } => warn!("Received unknown content: {:?}", header),
        }

        Ok(())
    }

    async fn handle_outgoing_request(&mut self, request: KernelRequest) -> Result<(), KernelError> {
        let context = {
            let mut sessions = self.sessions.lock().await;
            match (*sessions).get_key_value(request.header.get_session()) {
                Some((_, value)) => Ok(value.clone()),
                None => {
                    let context = self
                        .send_context_create()
                        .with_cluster_id("1024-181223-ye0dza2w")
                        .with_language("python")
                        .execute(&self.databricks)
                        .await;

                    match context {
                        Ok(context) => {
                            (*sessions).insert(String::from(request.header.get_session()), context.id.clone());
                            Ok(context.id)
                        }
                        Err(error) => {
                            warn!("Cannot create remote context: {:?}", error);
                            Err(error)
                        }
                    }
                }
            }
        };

        let (status, mimetype, response) = match context {
            Ok(context_id) => {
                let command = self
                    .send_command_execute()
                    .with_cluster_id("1024-181223-ye0dza2w")
                    .with_context_id(&context_id)
                    .with_language("python")
                    .with_command(&request.code)
                    .execute(&self.databricks)
                    .await;

                let (status, command_id, result) = match command {
                    Ok(response) => ("ok", Some(response.id), request.code),
                    Err(error) => {
                        warn!("Cannot execute remote command: {:?}", error);
                        ("error", None, error.to_string())
                    }
                };

                match (status, command_id) {
                    ("ok", Some(command_id)) => {
                        let status = self
                            .send_command_status()
                            .with_cluster_id("1024-181223-ye0dza2w")
                            .with_context_id(&context_id)
                            .with_command_id(&command_id)
                            .execute(&self.databricks)
                            .await;

                        match status {
                            Ok(response) => match response.results {
                                None => ("ok", "text/plain", format!("{:?}", response)),
                                Some(databricks::DatabricksCommandStatusResponseResult {
                                    summary: Some(summary),
                                    result_type: _,
                                    data: _,
                                    cause: _,
                                }) => ("error", "text/html", summary),
                                Some(databricks::DatabricksCommandStatusResponseResult {
                                    summary: _,
                                    result_type: _,
                                    data: Some(data),
                                    cause: _,
                                }) => ("ok", "text/plain", data),
                                Some(result) => ("ok", "text/plain", format!("{:?}", result)),
                            },
                            Err(error) => ("erorr", "text/plain", error.to_string()),
                        }
                    }
                    (status, _) => (status, "text/plain", result),
                }
            }
            Err(error) => ("error", "text/plain", error.to_string()),
        };

        self.send_execute_result(request.header.clone())
            .with_execution_count(request.execution_count)
            .with_data_string(mimetype, &response)
            .execute(&mut self.jupyter)
            .await?;

        self.send_execute_reply(request.channel, request.sender.clone(), request.header.clone())
            .with_status(status)
            .with_execution_count(request.execution_count)
            .execute(&mut self.jupyter)
            .await?;

        self.send_status(request.header.clone())
            .with_status("idle")
            .execute(&mut self.jupyter)
            .await?;

        Ok(())
    }

    pub async fn recv(&mut self) -> Result<(), KernelError> {
        loop {
            tokio::select! {
                message = self.jupyter.recv() => {
                    let message = match message {
                        Ok(message) => message,
                        Err(error) => return raise_receiving_failed(error),
                    };

                    let result = match message {
                        jupyter::JupyterMessage::Heartbeat { data } => {
                            debug!("Heartbeat {:?} ...", data);
                            self.handle_heartbeat(data).await?
                        }
                        jupyter::JupyterMessage::Content {
                            channel,
                            sender,
                            content,
                        } => self.handle_content(channel, sender, content).await?,
                    };

                    return Ok(result);
                },
                request = self.receiver.recv() => {
                    if let Some(request) = request {
                        debug!("Received {:?}", request);
                        self.handle_outgoing_request(request).await?;
                    }
                }
            }
        }
    }

    fn send_kernel_info_reply(
        &self,
        channel: jupyter::JupyterChannel,
        identity: bytes::Bytes,
        parent: jupyter::JupyterHeader,
    ) -> JupyterKernelInfoReplySender {
        JupyterKernelInfoReplySender::new(channel, identity, parent)
    }

    fn send_execute_reply(
        &self,
        channel: jupyter::JupyterChannel,
        identity: bytes::Bytes,
        parent: jupyter::JupyterHeader,
    ) -> JupyterExecuteReplySender {
        JupyterExecuteReplySender::new(channel, identity, parent)
    }

    fn send_execute_input(&self, parent: jupyter::JupyterHeader) -> JupyterExecuteInputSender {
        JupyterExecuteInputSender::new(parent)
    }

    fn send_execute_result(&self, parent: jupyter::JupyterHeader) -> JupyterExecuteResultSender {
        JupyterExecuteResultSender::new(parent)
    }

    fn send_input_request(&self, parent: jupyter::JupyterHeader) -> JupyterInputRequestSender {
        JupyterInputRequestSender::new(parent)
    }

    fn send_status(&self, parent: jupyter::JupyterHeader) -> JupyterKernelStatusSender {
        JupyterKernelStatusSender::new(parent)
    }

    fn send_context_create(&self) -> DatabricksContextCreateSender {
        DatabricksContextCreateSender::new()
    }

    fn send_command_execute(&self) -> DatabricksCommandExecuteSender {
        DatabricksCommandExecuteSender::new()
    }

    fn send_command_status(&self) -> DatabricksCommandStatusSender {
        DatabricksCommandStatusSender::new()
    }
}
