use crate::jupyter::errors::*;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct JupyterHeader {
    msg_id: String,
    msg_type: String,
    session: String,
    username: String,
    date: String,
    version: String,
}

#[derive(Debug)]
pub struct JupyterWireProtocol {
    frames: Vec<bytes::Bytes>,
}

#[derive(Debug)]
pub struct JupyterWireBuilder {
    header: Option<bytes::Bytes>,
    parent: Option<bytes::Bytes>,
    metadata: Option<bytes::Bytes>,
    content: Option<bytes::Bytes>,
}

fn generate_new_uuid() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}

fn format_current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn compute_hmac_sha256(key: &bytes::Bytes, data: &[impl AsRef<[u8]>]) -> bytes::Bytes {
    let mut digest = hmac_sha256::HMAC::new(key);

    for item in data {
        digest.update(item);
    }

    let result = digest.finalize();
    return bytes::Bytes::copy_from_slice(&result);
}

impl JupyterHeader {
    fn new(msg_type: String, session: String, username: String, version: String) -> Self {
        Self {
            msg_id: generate_new_uuid(),
            date: format_current_timestamp(),
            msg_type: msg_type,
            session: session,
            username: username,
            version: version,
        }
    }

    pub fn get_type(&self) -> &str {
        return &self.msg_type
    }

    pub fn get_session(&self) -> &str {
        return &self.session
    }

    pub fn reply(&self, msg_type: &str) -> Self {
        JupyterHeader::new(
            String::from(msg_type),
            self.session.clone(),
            self.username.clone(),
            self.version.clone(),
        )
    }

    fn from_json(data: &bytes::Bytes) -> Result<Self, JupyterError> {
        let message: JupyterHeader = match serde_json::from_slice(data) {
            Ok(message) => message,
            Err(error) => return raise_deserialization_failed("Header", error),
        };

        Ok(message)
    }

    fn to_json(&self) -> Result<bytes::Bytes, JupyterError> {
        let data: bytes::Bytes = match serde_json::to_vec(self) {
            Ok(data) => bytes::Bytes::from(data),
            Err(error) => return raise_serialization_failed("Header", error),
        };

        Ok(data)
    }
}

impl JupyterWireProtocol {
    fn new(frames: Vec<bytes::Bytes>) -> Result<Self, JupyterError> {
        if frames.len() < 7 {
            return raise_not_enough_frames(frames.len(), 7);
        }

        let instance = Self { frames: frames };

        Ok(instance)
    }

    pub fn verify(&self, key: &bytes::Bytes) -> Result<bool, JupyterError> {
        let signature = match self.get_signature() {
            Ok(signature) => signature,
            Err(error) => return Err(error),
        };

        let slice = match self.frames.get(3..7) {
            Some(slice) => slice,
            None => return raise_not_enough_frames(self.frames.len(), 7),
        };

        return Ok(compute_hmac_sha256(&key, slice) == signature);
    }

    pub fn from_zmq(message: zeromq::ZmqMessage) -> Result<Self, JupyterError> {
        if message.len() < 7 {
            return raise_not_enough_frames(message.len(), 7);
        }

        Ok(Self {
            frames: message.into_vec(),
        })
    }

    pub fn to_zmq(self) -> Result<zeromq::ZmqMessage, JupyterError> {
        match zeromq::ZmqMessage::try_from(self.frames) {
            Ok(message) => return Ok(message),
            Err(error) => return raise_invalid_implementation(error.to_string()),
        };
    }

    pub fn get_identifier(&self) -> bytes::Bytes {
        return bytes::Bytes::copy_from_slice(self.frames[0].as_ref());
    }

    fn get_signature(&self) -> Result<bytes::Bytes, JupyterError> {
        let signature = match String::from_utf8(self.frames[2].to_vec()) {
            Ok(text) => match hex::decode(text) {
                Ok(bytes) => bytes::Bytes::from(bytes),
                Err(error) => return raise_verification_failed(error.to_string()),
            },
            Err(error) => return raise_verification_failed(error.to_string()),
        };

        Ok(signature)
    }

    pub fn get_parent(&self) -> Result<Option<JupyterHeader>, JupyterError> {
        if bytes::Bytes::from_static(b"{}") == &self.frames[4] {
            return Ok(None);
        }
        
        match JupyterHeader::from_json(&self.frames[4]) {
            Ok(parent) => Ok(Some(parent)),
            Err(erorr) => return Err(erorr),
        }
    }

    pub fn get_header(&self) -> Result<JupyterHeader, JupyterError> {
        return JupyterHeader::from_json(&self.frames[3]);
    }

    pub fn get_content<'a, T: serde::Deserialize<'a>>(&'a self) -> Result<T, JupyterError> {
        let content: T = match serde_json::from_slice(&self.frames[6]) {
            Ok(content) => content,
            Err(error) => return raise_deserialization_failed("Content", error),
        };

        Ok(content)
    }
}

impl JupyterWireBuilder {
    pub fn new() -> Self {
        Self {
            header: None,
            parent: None,
            metadata: None,
            content: None,
        }
    }

    pub fn with_header(self, message: &JupyterHeader) -> Result<JupyterWireBuilder, JupyterError> {
        let header = match message.to_json() {
            Ok(data) => data,
            Err(error) => return Err(error),
        };

        Ok(Self {
            header: Some(header),
            parent: self.parent,
            metadata: self.metadata,
            content: self.content,
        })
    }

    pub fn with_parent(self, message: &JupyterHeader) -> Result<JupyterWireBuilder, JupyterError> {
        let parent = match message.to_json() {
            Ok(data) => data,
            Err(error) => return Err(error),
        };

        Ok(Self {
            header: self.header,
            parent: Some(parent),
            metadata: self.metadata,
            content: self.content,
        })
    }

    pub fn with_content<T: serde::Serialize>(self, content: &T) -> Result<JupyterWireBuilder, JupyterError> {
        let content = match serde_json::to_vec(content) {
            Ok(data) => bytes::Bytes::from(data),
            Err(error) => return raise_serialization_failed("Content", error),
        };

        Ok(Self {
            header: self.header,
            parent: self.parent,
            metadata: self.metadata,
            content: Some(content),
        })
    }

    pub fn sign(self, identity: &bytes::Bytes, key: &bytes::Bytes) -> Result<JupyterWireProtocol, JupyterError> {
        let header = match self.header {
            Some(header) => header,
            None => return raise_message_incomplete("Header"),
        };

        let parent = match self.parent {
            Some(parent) => parent,
            None => bytes::Bytes::from_static(b"{}"),
        };

        let metadata = match self.metadata {
            Some(metadata) => metadata,
            None => bytes::Bytes::from_static(b"{}"),
        };

        let content = match self.content {
            Some(content) => content,
            None => bytes::Bytes::from_static(b"{}"),
        };

        let delimiter = bytes::Bytes::from_static(b"<IDS|MSG>");
        let signature = compute_hmac_sha256(key, &[&header, &parent, &metadata, &content]);

        let payload = vec![
            identity.clone(),
            delimiter,
            bytes::Bytes::from(hex::encode(signature)),
            header,
            parent,
            metadata,
            content,
        ];

        let protocol = match JupyterWireProtocol::new(payload) {
            Ok(protocol) => protocol,
            Err(error) => return Err(error),
        };

        Ok(protocol)
    }
}
