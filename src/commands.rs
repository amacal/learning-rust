use structopt::StructOpt;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::bitstream::BitStream;
use crate::inflate::{self, InflateReader, InflateWriter, InflateSymbol};

#[derive(StructOpt, Debug)]
pub struct DecompressCommand {
    #[structopt(help = "The absolute or the relative path of the compressed zlib file")]
    pub source: String,
    #[structopt(help = "The absolute or the relative path of the decompressed zlib file")]
    pub destination: String,
}

#[derive(StructOpt, Debug)]
pub enum Cli {
    #[structopt(name = "decompress", help = "Decompresses zlib file")]
    Decompress(DecompressCommand),
}

pub type CliResult<T> = Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("IO Error on file '{0}': {1}")]
    IO(String, std::io::Error),

    #[error("Inflate Error: {0}")]
    Inflate(inflate::InflateError),
}

fn raise_io_error<T>(file: &str, error: std::io::Error) -> CliResult<T> {
    Err(CliError::IO(file.to_string(), error))
}

fn raise_inflate_error<T>(error: inflate::InflateError) -> CliResult<T> {
    Err(CliError::Inflate(error))
}

impl DecompressCommand {
    pub async fn handle(&self) -> CliResult<()> {
        let mut buffer = Box::new([0; 65_536]);

        let mut source = match File::open(&self.source).await {
            Ok(file) => file,
            Err(error) => return raise_io_error(&self.source, error),
        };

        let mut destination = match File::create(&self.destination).await {
            Ok(file) => file,
            Err(error) => return raise_io_error(&self.destination, error),
        };

        let count = match source.read(&mut buffer[..]).await {
            Ok(count) => count,
            Err(error) => return raise_io_error(&self.source, error),
        };

        let mut bitstream = BitStream::try_from(&buffer[0..count]).unwrap();
        let mut reader = InflateReader::zlib(&mut bitstream).unwrap();
        let mut writer = InflateWriter::new();

        loop {
            loop {
                let symbol = match reader.next(&mut bitstream) {
                    Ok(InflateSymbol::EndBlock) => break,
                    Ok(value) => value,
                    Err(error) => return raise_inflate_error(error),
                };

                if let Some(available) = writer.handle(symbol) {
                    let available = std::cmp::min(available, buffer.len());
                    let target = &mut buffer[0..available];

                    let count = writer.take(target);
                    let target = &target[..count];

                    if let Err(error) = destination.write(target).await {
                        return raise_io_error(&self.destination, error);
                    }
                }

                if let Some(available) = bitstream.hungry() {
                    let available = std::cmp::min(available, buffer.len());
                    let destination = &mut buffer[0..available];

                    let count = match source.read(destination).await {
                        Ok(count) => count,
                        Err(error) => return raise_io_error(&self.source, error),
                    };

                    bitstream.feed(&buffer[0..count]);
                }
            }

            if reader.is_completed() {
                let count = writer.take(&mut buffer[..]);
                let target = &mut buffer[..count];

                if let Err(error) = destination.write(target).await {
                    return raise_io_error(&self.destination, error);
                }

                if let Err(error) = destination.flush().await {
                    return raise_io_error(&self.destination, error);
                }

                break;
            }

            if reader.is_broken() {
                println!("broken");
                break;
            }
        }

        Ok(())
    } 
}