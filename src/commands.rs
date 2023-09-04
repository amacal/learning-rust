use structopt::StructOpt;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::bitstream::{BitStream, self};
use crate::inflate::{self, InflateEvent, InflateReader, InflateSymbol, InflateWriter};

#[derive(StructOpt, Debug)]
pub struct DecompressCommand {
    #[structopt(help = "The absolute or the relative path of the compressed zlib file")]
    pub source: String,
    #[structopt(help = "The absolute or the relative path of the decompressed zlib file")]
    pub destination: String,
}

#[derive(StructOpt, Debug)]
pub struct BlockCommand {
    #[structopt(help = "The absolute or the relative path of the compressed zlib file")]
    pub source: String,
}

#[derive(StructOpt, Debug)]
pub enum Cli {
    #[structopt(name = "decompress", help = "Decompresses zlib file")]
    Decompress(DecompressCommand),

    #[structopt(name = "block", help = "Analyzes deflate blocks")]
    Block(BlockCommand),
}

pub type CliResult<T> = Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("IO Error on file '{0}': {1}")]
    IO(String, std::io::Error),

    #[error("BitStream Error: {0}")]
    BitStream(bitstream::BitStreamError),

    #[error("Inflate Error: {0}")]
    Inflate(inflate::InflateError),
}

fn raise_io_error<T>(file: &str, error: std::io::Error) -> CliResult<T> {
    Err(CliError::IO(file.to_string(), error))
}

fn raise_bitstream_error<T>(error: bitstream::BitStreamError) -> CliResult<T> {
    Err(CliError::BitStream(error))
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

        let mut writer: InflateWriter<131_072> = InflateWriter::new();

        let mut bitstream: BitStream<131_072> = match BitStream::try_from(&buffer[0..count]) {
            Ok(bitstream) => bitstream,
            Err(error) => return raise_bitstream_error(error),
        };

        let mut reader = match InflateReader::zlib(&mut bitstream) {
            Ok(reader) => reader,
            Err(error) => return raise_inflate_error(error),
        };

        loop {
            loop {
                let symbol = match reader.next(&mut bitstream) {
                    Ok(InflateEvent::BlockStarted(_)) => continue,
                    Ok(InflateEvent::BlockEnded(_)) => continue,
                    Ok(InflateEvent::SymbolDecoded(symbol)) => match symbol {
                        InflateSymbol::EndBlock => break,
                        value => value,
                    },
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

impl BlockCommand {
    pub async fn handle(&self) -> CliResult<()> {
        let mut buffer = Box::new([0; 65_536]);

        let mut source = match File::open(&self.source).await {
            Ok(file) => file,
            Err(error) => return raise_io_error(&self.source, error),
        };

        let count = match source.read(&mut buffer[..]).await {
            Ok(count) => count,
            Err(error) => return raise_io_error(&self.source, error),
        };

        let mut bitstream: BitStream<131_072> = match BitStream::try_from(&buffer[0..count]) {
            Ok(bitstream) => bitstream,
            Err(error) => return raise_bitstream_error(error),
        };

        let mut reader = match InflateReader::zlib(&mut bitstream) {
            Ok(reader) => reader,
            Err(error) => return raise_inflate_error(error),
        };

        loop {
            loop {
                let event = match reader.next(&mut bitstream) {
                    Ok(event) => event,
                    Err(error) => return raise_inflate_error(error),
                };

                if let Some(available) = bitstream.hungry() {
                    let available = std::cmp::min(available, buffer.len());
                    let destination = &mut buffer[0..available];

                    let count = match source.read(destination).await {
                        Ok(count) => count,
                        Err(error) => return raise_io_error(&self.source, error),
                    };

                    bitstream.feed(&buffer[0..count]);
                }

                match event {
                    InflateEvent::BlockStarted(index) => {
                        println!("Block {} started", index);

                        let info = match reader.block() {
                            Ok(info) => info,
                            Err(error) => return raise_inflate_error(error),
                        };

                        println!("  Last:     {}", info.last);
                        println!("  Mode:     {}", info.mode);
                        println!("  Decoder:  {}", info.decoder);

                        if let Some(literals) = info.literals {
                            println!("  Literals:");

                            for (symbol, code) in literals.iter().enumerate() {
                                if code.length > 0 {
                                    println!("    {:>3} -> {}", symbol, code);
                                }
                            }
                        }

                        if let Some(distances) = info.distances {
                            println!("  Distances:");

                            for (symbol, code) in distances.iter().enumerate() {
                                if code.length > 0 {
                                    println!("    {:>3} -> {}", symbol, code);
                                }
                            }
                        }
                    }
                    InflateEvent::BlockEnded(index) => {
                        println!("Block {} ended", index);
                        break;
                    }
                    InflateEvent::SymbolDecoded(symbol) => match symbol {
                        InflateSymbol::Uncompressed { data } => {
                            println!("  Length    {}", data.len());
                        }
                        _ => {}
                    }
                }
            }

            if reader.is_completed() {
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
