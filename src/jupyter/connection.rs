use crate::jupyter::client::JupyterChannel;
use crate::jupyter::errors::JupyterError;
use crate::jupyter::errors::raise_connection_file_failed;

#[derive(serde::Deserialize, Debug)]
pub struct JupyterConnectionInfo {
    shell_port: u16,
    iopub_port: u16,
    stdin_port: u16,
    control_port: u16,
    hb_port: u16,
    ip: String,
    key: String,
    transport: String,
    signature_scheme: String,
    kernel_name: String,
}

pub async fn read_connection(path: &str) -> Result<JupyterConnectionInfo, JupyterError> {
    let mut contents = String::new();
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) => return raise_connection_file_failed(path, error.to_string()),
    };

    match tokio::io::AsyncReadExt::read_to_string(&mut file, &mut contents).await {
        Ok(_) => (),
        Err(error) => return raise_connection_file_failed(path, error.to_string()),
    };

    let data: JupyterConnectionInfo = match serde_json::from_str(&contents) {
        Ok(data) => data,
        Err(error) => return raise_connection_file_failed(path, error.to_string()),
    };

    Ok(data)
}

impl JupyterConnectionInfo {
    pub fn get_signing_key(&self) -> bytes::Bytes {
        return bytes::Bytes::copy_from_slice(self.key.as_ref());
    }

    pub fn get_signature_scheme(&self) -> &str {
        return &self.signature_scheme
    }

    pub fn get_kernel_name(&self) -> &str {
        return &self.kernel_name
    }

    pub fn format_endpoint(&self, channel: JupyterChannel) -> String {
        let port = match channel {
            JupyterChannel::Shell => self.shell_port,
            JupyterChannel::IOPub => self.iopub_port,
            JupyterChannel::Control => self.control_port,
            JupyterChannel::StdIn => self.stdin_port,
            JupyterChannel::Heartbeat => self.hb_port,
        };

        return format!("{}://{}:{}", self.transport, self.ip, port);
    }
}
