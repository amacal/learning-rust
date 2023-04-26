use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::databricks;
use crate::environment;
use crate::jupyter;
use crate::kernel::errors::*;
use crate::kernel::senders::*;

use log::{debug, warn};

pub struct KernelClient {
    cluster_id: String,
    counter: Arc<Mutex<u32>>,
    sessions: Arc<Mutex<HashMap<String, String>>>,

    jupyter: jupyter::JupyterClient,
    databricks: databricks::DatabricksClient,

    sender: mpsc::Sender<KernelRequest>,
    receiver: mpsc::Receiver<KernelRequest>,
}

#[derive(Debug, Clone)]
pub struct KernelRequest {
    channel: jupyter::JupyterChannel,
    sender: bytes::Bytes,
    header: jupyter::JupyterHeader,
    execution_count: u32,
    code: String,
}

impl KernelClient {
    pub async fn start(path: &str, cluster_id: &str) -> Result<Self, KernelError> {
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

        let (sender, receiver) = mpsc::channel(100);

        let instance = Self {
            counter: Arc::new(Mutex::new(0u32)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            cluster_id: String::from(cluster_id),
            sender: sender,
            receiver: receiver,
            jupyter: jupyter,
            databricks: databricks,
        };

        Ok(instance)
    }

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

    async fn find_context_by_session(&mut self, session: &str) -> Result<String, KernelError> {
        let mut sessions = self.sessions.lock().await;

        if let Some((_, value)) = (*sessions).get_key_value(session) {
            return Ok(value.clone());
        }

        let context = self
            .send_context_create()
            .with_cluster_id(&self.cluster_id)
            .with_language("python")
            .execute(&self.databricks)
            .await;

        match context {
            Ok(context) => {
                (*sessions).insert(String::from(session), context.id.clone());
                Ok(context.id)
            }
            Err(error) => {
                warn!("Cannot create remote context: {:?}", error);
                Err(error)
            }
        }
    }

    async fn propagate_error(&mut self, request: &KernelRequest, error: KernelError) -> Result<(), KernelError> {
        self.send_update_display_data(request.header.clone())
            .with_execution_count(request.execution_count)
            .with_data_string("text/plain", &error.to_string())
            .with_transient("display_id", "result")
            .execute(&mut self.jupyter)
            .await
    }

    async fn handle_outgoing_request(&mut self, request: KernelRequest) -> Result<(), KernelError> {
        let context = match self.find_context_by_session(request.header.get_session()).await {
            Ok(context) => context,
            Err(error) => return self.propagate_error(&request, error).await,
        };

        self.send_status(request.header.clone())
            .with_status("busy")
            .execute(&mut self.jupyter)
            .await?;

        let execution = self
            .send_command_execute()
            .with_cluster_id(&self.cluster_id)
            .with_context_id(&context)
            .with_language("python")
            .with_command(&request.code)
            .execute(&self.databricks)
            .await;

        let execution = match execution {
            Ok(response) => response.id,
            Err(error) => return self.propagate_error(&request, error).await,
        };

        self.send_display_data(request.header.clone())
            .with_execution_count(request.execution_count)
            .with_data_string("text/plain", "Running")
            .with_transient("display_id", "result")
            .execute(&mut self.jupyter)
            .await?;

        loop {
            let status = self
                .send_command_status()
                .with_cluster_id(&self.cluster_id)
                .with_context_id(&context)
                .with_command_id(&execution)
                .execute(&self.databricks)
                .await;

            let response = match status {
                Ok(response) => response,
                Err(error) => return self.propagate_error(&request, error).await,
            };

            let (mimetype, status, data) = match (&response.status, &response.results) {
                (status, Some(result)) if status == "Finished" => match (&result.data, &result.summary) {
                    (Some(data), _) => ("text/plain", String::from(status), String::from(data)),
                    (_, Some(summary)) => ("text/html", String::from(status), String::from(summary)),
                    (None, None) => ("text/plain", String::from(status), format!("{:?}", response)),
                },
                (status, _) => ("text/plain", String::from(status), String::from(status)),
            };

            self.send_update_display_data(request.header.clone())
                .with_execution_count(request.execution_count)
                .with_data_string(mimetype, &data)
                .with_transient("display_id", "result")
                .execute(&mut self.jupyter)
                .await?;

            if status != "Running" && status != "Queued" {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        self.send_execute_reply(request.channel, request.sender.clone(), request.header.clone())
            .with_status("ok")
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
                },
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

    fn send_display_data(&self, parent: jupyter::JupyterHeader) -> JupyterDisplayDataSender {
        JupyterDisplayDataSender::new(parent)
    }

    fn send_update_display_data(&self, parent: jupyter::JupyterHeader) -> JupyterDisplayDataSender {
        JupyterDisplayDataSender::update(parent)
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
