use structopt::StructOpt;

use std::fs::File;
use std::io::{Read, Write};

use tokio::fs::File as TokioFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::inflate::{InflateError, InflateEvent, InflateSymbol, InflateWriter};
use crate::zlib::{ZlibError, ZlibEvent, ZlibReader};

#[derive(StructOpt, Debug)]
pub struct DecompressAsyncCommand {
    #[structopt(help = "The absolute or the relative path of the compressed zlib file")]
    pub source: String,
    #[structopt(help = "The absolute or the relative path of the decompressed zlib file")]
    pub destination: String,
}

#[derive(StructOpt, Debug)]
pub struct DecompressSyncCommand {
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
    #[structopt(name = "decompress-async", help = "Decompresses zlib file asynchronously")]
    DecompressAsync(DecompressAsyncCommand),

    #[structopt(name = "decompress-sync", help = "Decompresses zlib file synchronously")]
    DecompressSync(DecompressSyncCommand),

    #[structopt(name = "block", help = "Analyzes deflate blocks")]
    Block(BlockCommand),
}

pub type CliResult<T> = Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("I/O error on file '{0}': {1}")]
    IO(String, std::io::Error),

    #[error("Inflate Error: {0}")]
    Inflate(InflateError),

    #[error("Zlib failed: {0}")]
    Zlib(ZlibError),

    #[error("Checksum error: {0}")]
    Checksum(String),
}

impl CliError {
    fn raise_io_error<T>(file: &str, error: std::io::Error) -> CliResult<T> {
        Err(CliError::IO(file.to_string(), error))
    }

    fn raise_inflate_error<T>(error: InflateError) -> CliResult<T> {
        Err(CliError::Inflate(error))
    }

    fn raise_zlib_error<T>(error: ZlibError) -> CliResult<T> {
        Err(CliError::Zlib(error))
    }

    fn raise_checksum_error<T>(description: &str) -> CliResult<T> {
        Err(CliError::Checksum(description.to_string()))
    }
}

impl DecompressAsyncCommand {
    pub async fn handle(&self) -> CliResult<()> {
        let mut buffer = Box::new([0; 65_536]);

        let mut source = match TokioFile::open(&self.source).await {
            Ok(file) => file,
            Err(error) => return CliError::raise_io_error(&self.source, error),
        };

        let mut destination = match TokioFile::create(&self.destination).await {
            Ok(file) => file,
            Err(error) => return CliError::raise_io_error(&self.destination, error),
        };

        let count = match source.read(&mut buffer[..]).await {
            Ok(count) => count,
            Err(error) => return CliError::raise_io_error(&self.source, error),
        };

        let mut writer: InflateWriter<131_072> = InflateWriter::new();
        let mut reader: ZlibReader<131_072> = match ZlibReader::open(&buffer[..count]) {
            Ok(reader) => reader,
            Err(error) => return CliError::raise_zlib_error(error),
        };

        loop {
            loop {
                let event = match reader.next() {
                    Ok(event) => event,
                    Err(error) => return CliError::raise_zlib_error(error),
                };

                let symbol = match event {
                    ZlibEvent::Checksum(_) => continue,
                    ZlibEvent::Inflate(InflateEvent::BlockStarted(_)) => continue,
                    ZlibEvent::Inflate(InflateEvent::BlockEnded(_)) => continue,
                    ZlibEvent::Inflate(InflateEvent::SymbolDecoded(symbol)) => match symbol {
                        InflateSymbol::EndBlock => break,
                        value => value,
                    },
                };

                if let Some(available) = writer.handle(symbol) {
                    let available = std::cmp::min(available, buffer.len());
                    let target = &mut buffer[0..available];

                    let count = writer.collect(target);
                    let target = &target[..count];

                    if let Err(error) = destination.write(target).await {
                        return CliError::raise_io_error(&self.destination, error);
                    }
                }

                if let Some(available) = reader.appendable() {
                    let available = std::cmp::min(available, buffer.len());
                    let destination = &mut buffer[0..available];

                    let count = match source.read(destination).await {
                        Ok(count) => count,
                        Err(error) => return CliError::raise_io_error(&self.source, error),
                    };

                    if let Err(error) = reader.append(&buffer[0..count]) {
                        return CliError::raise_zlib_error(error);
                    }
                }
            }

            if reader.is_completed() {
                let count = writer.collect(&mut buffer[..]);
                let target = &mut buffer[..count];

                if let Err(error) = destination.write(target).await {
                    return CliError::raise_io_error(&self.destination, error);
                }

                if let Err(error) = destination.flush().await {
                    return CliError::raise_io_error(&self.destination, error);
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

impl DecompressSyncCommand {
    pub fn handle(&self) -> CliResult<()> {
        let mut buffer = Box::new([0; 65_536]);

        let mut source = match File::open(&self.source) {
            Ok(file) => file,
            Err(error) => return CliError::raise_io_error(&self.source, error),
        };

        let mut destination = match File::create(&self.destination) {
            Ok(file) => file,
            Err(error) => return CliError::raise_io_error(&self.destination, error),
        };

        let count = match source.read(&mut buffer[..]) {
            Ok(count) => count,
            Err(error) => return CliError::raise_io_error(&self.source, error),
        };

        let mut checksum = None;
        let mut writer: InflateWriter<131_072> = InflateWriter::new();

        let mut reader: ZlibReader<131_072> = match ZlibReader::open(&buffer[0..count]) {
            Ok(reader) => reader,
            Err(error) => return CliError::raise_zlib_error(error),
        };

        loop {
            loop {
                let event = match reader.next() {
                    Ok(event) => event,
                    Err(error) => return CliError::raise_zlib_error(error),
                };

                let symbol = match event {
                    ZlibEvent::Checksum(value) => break checksum = Some(value),
                    ZlibEvent::Inflate(InflateEvent::BlockStarted(_)) => continue,
                    ZlibEvent::Inflate(InflateEvent::BlockEnded(_)) => continue,
                    ZlibEvent::Inflate(InflateEvent::SymbolDecoded(symbol)) => match symbol {
                        InflateSymbol::EndBlock => break,
                        value => value,
                    },
                };

                if let Some(available) = writer.handle(symbol) {
                    let available = std::cmp::min(available, buffer.len());
                    let target = &mut buffer[0..available];

                    let count = writer.collect(target);
                    let target = &target[..count];

                    if let Err(error) = destination.write(target) {
                        return CliError::raise_io_error(&self.destination, error);
                    }
                }

                if let Some(available) = reader.appendable() {
                    let available = std::cmp::min(available, buffer.len());
                    let destination = &mut buffer[0..available];

                    let count = match source.read(destination) {
                        Ok(count) => count,
                        Err(error) => return CliError::raise_io_error(&self.source, error),
                    };

                    if let Err(error) = reader.append(&buffer[0..count]) {
                        return CliError::raise_zlib_error(error);
                    }
                }
            }

            if reader.is_completed() {
                let count = writer.collect(&mut buffer[..]);
                let target = &mut buffer[..count];

                if let Err(error) = destination.write(target) {
                    return CliError::raise_io_error(&self.destination, error);
                }

                if let Err(error) = destination.flush() {
                    return CliError::raise_io_error(&self.destination, error);
                }

                match checksum {
                    None => return CliError::raise_checksum_error("missing checksum"),
                    Some(value) if value != writer.checksum() => return CliError::raise_checksum_error("wrong checksum"),
                    Some(_) => (),
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

        let mut source = match TokioFile::open(&self.source).await {
            Ok(file) => file,
            Err(error) => return CliError::raise_io_error(&self.source, error),
        };

        let count = match source.read(&mut buffer[..]).await {
            Ok(count) => count,
            Err(error) => return CliError::raise_io_error(&self.source, error),
        };

        let mut reader: ZlibReader<131_072> = match ZlibReader::open(&buffer[0..count]) {
            Ok(reader) => reader,
            Err(error) => return CliError::raise_zlib_error(error),
        };

        loop {
            loop {
                let event = match reader.next() {
                    Ok(ZlibEvent::Inflate(event)) => event,
                    Ok(ZlibEvent::Checksum(_)) => break,
                    Err(error) => return CliError::raise_zlib_error(error),
                };

                if let Some(available) = reader.appendable() {
                    let available = std::cmp::min(available, buffer.len());
                    let destination = &mut buffer[0..available];

                    let count = match source.read(destination).await {
                        Ok(count) => count,
                        Err(error) => return CliError::raise_io_error(&self.source, error),
                    };

                    if let Err(error) = reader.append(&buffer[0..count]) {
                        return CliError::raise_zlib_error(error);
                    }
                }

                match event {
                    InflateEvent::BlockStarted(index) => {
                        println!("Block {} started", index);

                        let info = match reader.block() {
                            Ok(info) => info,
                            Err(error) => return CliError::raise_inflate_error(error),
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
                            println!("  Length:   {}", data.len());
                        }
                        _ => {}
                    },
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
