use log::info;
use zeromq::prelude::*;

use crate::jupyter::connection::*;
use crate::jupyter::errors::*;
use crate::jupyter::messages::*;
use crate::jupyter::protocol::*;

#[derive(Debug, Copy, Clone)]
pub enum JupyterChannel {
    Shell,
    IOPub,
    Control,
    StdIn,
    Heartbeat,
}

pub enum JupyterContent {
    KernelInfoRequest {
        header: JupyterHeader,
        request: JupyterKernelInfoRequest,
    },
    ExecuteRequest {
        header: JupyterHeader,
        request: JupyterExecuteRequest,
    },
    InputReply {
        parent: Option<JupyterHeader>,
        header: JupyterHeader,
        reply: JupyterInputReply,
    },
    Unknown {
        header: JupyterHeader,
    },
}

pub enum JupyterMessage {
    Heartbeat {
        data: bytes::Bytes,
    },
    Content {
        channel: JupyterChannel,
        sender: bytes::Bytes,
        content: JupyterContent,
    },
}

pub struct JupyterClient {
    configuration: JupyterConnectionInfo,
    shell: zeromq::RouterSocket,
    iopub: zeromq::PubSocket,
    control: zeromq::RouterSocket,
    stdin: zeromq::RouterSocket,
    heartbeat: zeromq::RepSocket,
}

impl JupyterClient {
    async fn bind(
        configuration: &JupyterConnectionInfo,
        channel: JupyterChannel,
        socket: &mut impl zeromq::Socket,
    ) -> Result<(), JupyterError> {
        let endpoint = configuration.format_endpoint(channel);

        match socket.bind(&endpoint).await {
            Err(error) => return raise_connection_socket_failed(channel, error),
            Ok(endpoint) => info!("Channel '{:?}' bound to endpoint {}", channel, endpoint),
        };

        Ok(())
    }

    pub fn get_kernel_name(&self) -> &str {
        self.configuration.get_kernel_name()
    }

    pub async fn connect(configuration: JupyterConnectionInfo) -> Result<Self, JupyterError> {
        let mut shell = zeromq::RouterSocket::new();
        let mut iopub = zeromq::PubSocket::new();
        let mut control = zeromq::RouterSocket::new();
        let mut stdin = zeromq::RouterSocket::new();
        let mut heartbeat = zeromq::RepSocket::new();

        JupyterClient::bind(&configuration, JupyterChannel::Shell, &mut shell).await?;
        JupyterClient::bind(&configuration, JupyterChannel::IOPub, &mut iopub).await?;
        JupyterClient::bind(&configuration, JupyterChannel::Control, &mut control).await?;
        JupyterClient::bind(&configuration, JupyterChannel::StdIn, &mut stdin).await?;
        JupyterClient::bind(&configuration, JupyterChannel::Heartbeat, &mut heartbeat).await?;

        let instance = Self {
            configuration: configuration,
            shell: shell,
            iopub: iopub,
            control: control,
            stdin: stdin,
            heartbeat: heartbeat,
        };

        Ok(instance)
    }

    fn parse_heartbeat_message(&self, message: zeromq::ZmqMessage) -> Result<JupyterMessage, JupyterError> {
        info!("Length: {:?}", message.len());
        info!("Data: {:?}", message.into_vec());

        Ok(JupyterMessage::Heartbeat {
            data: bytes::Bytes::new(),
        })
    }

    fn parse_protocol_message(
        &self,
        channel: JupyterChannel,
        message: zeromq::ZmqMessage,
    ) -> Result<JupyterMessage, JupyterError> {
        info!("Found {} frames in message from '{:?}'", message.len(), channel);

        let protocol = match JupyterWireProtocol::from_zmq(message) {
            Ok(protocol) => protocol,
            Err(error) => return Err(error),
        };

        match protocol.verify(&self.configuration.get_signing_key()) {
            Ok(true) => info!("Verified message from '{:?}'", channel),
            Ok(false) => return raise_invalid_signature(&protocol.get_identifier()),
            Err(error) => return Err(error),
        }

        let parent = match protocol.get_parent() {
            Ok(parent) => parent,
            Err(error) => return Err(error),
        };

        let header = match protocol.get_header() {
            Ok(header) => header,
            Err(error) => return Err(error),
        };

        let content = match header.get_type() {
            "kernel_info_request" => JupyterContent::KernelInfoRequest {
                header: header,
                request: protocol.get_content()?,
            },
            "execute_request" => JupyterContent::ExecuteRequest {
                header: header,
                request: protocol.get_content()?,
            },
            "input_reply" => JupyterContent::InputReply {
                parent: parent,
                header: header,
                reply: protocol.get_content()?,
            },
            _ => JupyterContent::Unknown { header: header },
        };

        Ok(JupyterMessage::Content {
            channel: channel,
            sender: protocol.get_identifier(),
            content: content,
        })
    }

    fn recv_complete(
        &self,
        channel: JupyterChannel,
        result: Result<zeromq::ZmqMessage, zeromq::ZmqError>,
    ) -> Result<JupyterMessage, JupyterError> {
        let message = match result {
            Ok(message) => message,
            Err(error) => return raise_connection_socket_failed(channel, error),
        };

        match channel {
            JupyterChannel::Heartbeat => self.parse_heartbeat_message(message),
            _ => self.parse_protocol_message(channel, message),
        }
    }

    pub async fn recv(&mut self) -> Result<JupyterMessage, JupyterError> {
        loop {
            tokio::select! {
                shell = self.shell.recv() => {
                    info!("Channel '{:?}' completed", JupyterChannel::Shell);
                    return self.recv_complete(JupyterChannel::Shell, shell)
                },
                control = self.control.recv() => {
                    info!("Channel '{:?}' completed", JupyterChannel::Control);
                    return self.recv_complete(JupyterChannel::Control, control)
                },
                stdin = self.stdin.recv() => {
                    info!("Channel '{:?}' completed", JupyterChannel::StdIn);
                    return self.recv_complete(JupyterChannel::StdIn, stdin)
                },
                heartbeat = self.heartbeat.recv() => {
                    info!("Channel '{:?}' completed", JupyterChannel::Heartbeat);
                    return self.recv_complete(JupyterChannel::Heartbeat, heartbeat)
                }
            }
        }
    }

    pub async fn send<T: serde::Serialize>(
        &mut self,
        channel: JupyterChannel,
        identity: &bytes::Bytes,
        header: &JupyterHeader,
        parent: Option<&JupyterHeader>,
        content: &T,
    ) -> Result<(), JupyterError> {
        info!("Sending '{}' to '{:?}' ...", header.get_type(), channel);
        let builder = JupyterWireBuilder::new();

        let builder = match builder.with_header(header) {
            Ok(builder) => builder,
            Err(error) => return Err(error),
        };

        let builder = match parent {
            None => builder,
            Some(parent) => match builder.with_parent(parent) {
                Ok(builder) => builder,
                Err(error) => return Err(error),
            },
        };

        let builder = match builder.with_content(content) {
            Ok(builder) => builder,
            Err(error) => return Err(error),
        };

        let protocol = match builder.sign(identity, &self.configuration.get_signing_key()) {
            Ok(protocol) => protocol,
            Err(error) => return Err(error),
        };

        match protocol.verify(&self.configuration.get_signing_key()) {
            Ok(true) => (),
            Ok(false) => return raise_invalid_implementation(String::from("signature verification failed")),
            Err(error) => return Err(error),
        };

        let message = match protocol.to_zmq() {
            Ok(message) => message,
            Err(error) => return Err(error),
        };

        let sending = match channel {
            JupyterChannel::Shell => self.shell.send(message).await,
            JupyterChannel::IOPub => self.iopub.send(message).await,
            JupyterChannel::Control => self.control.send(message).await,
            JupyterChannel::StdIn => self.stdin.send(message).await,
            JupyterChannel::Heartbeat => self.heartbeat.send(message).await,
        };

        match sending {
            Ok(_) => Ok(info!("Sending to '{:?}' succeeded", channel)),
            Err(error) => return raise_connection_socket_failed(channel, error),
        }
    }
}
