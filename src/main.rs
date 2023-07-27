use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Seek};

use hex;
use ring::digest;

#[derive(Debug, thiserror::Error)]
enum AgentError {
    #[error("Parsing failed: {0}")]
    ParsingFailed(Box<dyn std::error::Error>),

    #[error("Communication failed: {0}")]
    CommunicationFailed(Box<dyn std::error::Error>),

    #[error("Protocol failed")]
    ProtocolFailed(),

    #[error("Unrecognized sequence: {0}")]
    Unrecognized(u8),
}

struct AgentSignRequest<'a> {
    key: &'a [u8],
    content: &'a [u8],
}

impl<'a> AgentSignRequest<'a> {
    fn new(key: &'a [u8], content: &'a [u8]) -> Self {
        Self {
            key: key,
            content: content,
        }
    }

    fn get_payload(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let length = 1 + self.key.len() + 4 + self.content.len() + 4;

        WriteBytesExt::write_u32::<BigEndian>(&mut result, length as u32).unwrap();
        result.push(13);

        result.extend_from_slice(self.key);

        WriteBytesExt::write_u32::<BigEndian>(&mut result, self.content.len() as u32).unwrap();
        result.extend_from_slice(self.content);

        WriteBytesExt::write_u32::<BigEndian>(&mut result, 2).unwrap();
        result
    }
}

enum AgentRequest<'a> {
    RequestIdentities,
    SignRequest(AgentSignRequest<'a>),
}

impl<'a> AgentRequest<'a> {
    async fn write(&self, stream: &mut UnixStream) -> Result<(), AgentError> {
        let message: Vec<u8> = match self {
            AgentRequest::RequestIdentities => vec![0, 0, 0, 1, 11],
            AgentRequest::SignRequest(data) => data.get_payload(),
        };

        match stream.write_all(&message).await {
            Err(error) => Err(AgentError::CommunicationFailed(Box::new(error))),
            Ok(_) => Ok(()),
        }
    }
}

struct AgentResponse<'a> {
    data: &'a [u8],
}

impl<'a> AgentResponse<'a> {
    fn from(buffer: &'a Vec<u8>, count: usize) -> Self {
        Self {
            data: buffer[..count].as_ref(),
        }
    }

    fn parse(&self) -> Result<AgentResponseVariant<'a>, AgentError> {
        let mut cursor = Cursor::new(self.data);
        let count = ReadBytesExt::read_u32::<BigEndian>(&mut cursor);

        let (slice, index) = match count {
            Ok(length) if length > self.data.len() as u32 => return Err(AgentError::ProtocolFailed()),
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
            Ok(length) => (&self.data[5..4 + length as usize], 4),
        };

        match self.data.get(index) {
            Some(discriminator) if *discriminator == 12 => match AgentIdentities::parse(slice) {
                Err(error) => Err(error),
                Ok(identities) => Ok(AgentResponseVariant::Identities(identities)),
            },
            Some(discriminator) if *discriminator == 14 => match AgentSignature::parse(slice) {
                Err(error) => Err(error),
                Ok(signature) => Ok(AgentResponseVariant::Signature(signature)),
            },
            Some(discriminator) => Err(AgentError::Unrecognized(*discriminator)),
            None => Err(AgentError::ProtocolFailed()),
        }
    }
}

#[derive(Debug)]
enum AgentResponseVariant<'a> {
    Identities(AgentIdentities<'a>),
    Signature(AgentSignature<'a>),
}

#[derive(Debug)]
struct AgentRsaIdentity<'a> {
    raw: &'a [u8],
    key_type: String,
    exponent: &'a [u8],
    modulus: &'a [u8],
    comment: String,
}

impl<'a> AgentRsaIdentity<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, AgentError> {
        let mut cursor = Cursor::new(data);

        let blob_length = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => value as usize,
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let key_type = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => {
                let start = cursor.position() as usize;
                let end = start + value as usize;

                match cursor.seek(std::io::SeekFrom::Current(value as i64)) {
                    Ok(_) => String::from_utf8_lossy(&data[start..end]).into_owned(),
                    Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
                }
            }
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let exponent = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => {
                let start = cursor.position() as usize;
                let end = start + value as usize;

                match cursor.seek(std::io::SeekFrom::Current(value as i64)) {
                    Ok(_) => &data[start..end],
                    Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
                }
            }
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let modulus = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => {
                let start = cursor.position() as usize;
                let end = start + value as usize;

                match cursor.seek(std::io::SeekFrom::Current(value as i64)) {
                    Ok(_) => &data[start..end],
                    Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
                }
            }
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let comment = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => {
                let start = cursor.position() as usize;
                let end = start + value as usize;

                match cursor.seek(std::io::SeekFrom::Current(value as i64)) {
                    Ok(_) => String::from_utf8_lossy(&data[start..end]).into_owned(),
                    Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
                }
            }
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        Ok(Self {
            raw: &data[..4 + blob_length],
            key_type: key_type,
            exponent: exponent,
            modulus: modulus,
            comment: comment,
        })
    }
}

#[derive(Debug)]
struct AgentIdentities<'a> {
    items: Vec<AgentRsaIdentity<'a>>,
}

impl<'a> AgentIdentities<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, AgentError> {
        let mut cursor = Cursor::new(data);
        let count = ReadBytesExt::read_u32::<BigEndian>(&mut cursor);

        let count = match count {
            Ok(count) => count,
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let mut slice: &[u8] = &data[4..];
        let mut items = Vec::new();

        for _ in 0..count {
            match AgentRsaIdentity::parse(slice) {
                Err(error) => return Err(error),
                Ok(identity) => {
                    slice = &slice[identity.raw.len()..];
                    items.push(identity);
                }
            }
        }

        Ok(Self { items: items })
    }
}

#[derive(Debug)]
struct AgentSignature<'a> {
    _raw: &'a [u8],
    _key_type: String,
    signature_blob: &'a [u8],
}

impl<'a> AgentSignature<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, AgentError> {
        let mut cursor = Cursor::new(data);

        let _ = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => value as usize,
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let key_type = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => {
                let start = cursor.position() as usize;
                let end = start + value as usize;

                match cursor.seek(std::io::SeekFrom::Current(value as i64)) {
                    Ok(_) => String::from_utf8_lossy(&data[start..end]).into_owned(),
                    Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
                }
            }
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        let signature_blob = match ReadBytesExt::read_u32::<BigEndian>(&mut cursor) {
            Ok(value) => {
                let start = cursor.position() as usize;
                let end = start + value as usize;

                match cursor.seek(std::io::SeekFrom::Current(value as i64)) {
                    Ok(_) => &data[start..end],
                    Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
                }
            }
            Err(error) => return Err(AgentError::ParsingFailed(Box::new(error))),
        };

        Ok(Self {
            _raw: data,
            _key_type: key_type,
            signature_blob: signature_blob,
        })
    }
}

#[tokio::main]
async fn main() {
    let agent_sock_path = std::env::var("SSH_AUTH_SOCK").unwrap();
    let mut stream = UnixStream::connect(agent_sock_path).await.unwrap();

    let request = AgentRequest::RequestIdentities;
    request.write(&mut stream).await.unwrap();

    let mut buffer = vec![0; 4096];
    let count = stream.read(&mut buffer).await.unwrap();

    let response = AgentResponse::from(&buffer, count);
    let variant = response.parse();

    if let Ok(AgentResponseVariant::Identities(identities)) = variant {
        for identity in identities.items {
            println!("Key Type / Comment: {} / {}", identity.key_type, identity.comment);
            println!("exponent + modulus: {} bytes + {} bytes", identity.exponent.len(), identity.modulus.len());

            let content = std::fs::read("LICENSE").unwrap();
            let mut context = digest::Context::new(&digest::SHA256);

            context.update(&content);

            let digest = context.finish();
            let request = AgentSignRequest::new(identity.raw, &content);

            let request = AgentRequest::SignRequest(request);
            request.write(&mut stream).await.unwrap();

            let mut buffer = vec![0; 4096];
            let count = stream.read(&mut buffer).await.unwrap();

            let response = AgentResponse::from(&buffer, count);
            let variant = response.parse();

            if let Ok(AgentResponseVariant::Signature(signature)) = variant {
                std::fs::write("LICENSE.sig", signature.signature_blob).unwrap();
                println!("\nDigest SHA-256: {}", hex::encode(digest.as_ref()));
                println!("Signature written to LICENSE.sig");
            }
        }
    }
}
