mod bitstream;
mod huffman;
mod inflate;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::bitstream::BitStream;
use crate::inflate::{InflateReader, InflateWriter, InflateSymbol};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let mut buffer = Box::new([0; 65_536]);

    runtime.block_on(async {
        let mut source = File::open(std::env::args().nth(1).unwrap()).await.unwrap();
        let mut destination = File::create(std::env::args().nth(2).unwrap()).await.unwrap();

        let count = match source.read(&mut buffer[..]).await {
            Ok(count) => count,
            Err(error) => return Err(Box::new(error) as Box<dyn std::error::Error>),
        };

        let mut bitstream = BitStream::try_from(&buffer[0..count]).unwrap();
        let mut reader = InflateReader::zlib(&mut bitstream).unwrap();
        let mut writer = InflateWriter::new();

        loop {
            loop {
                let symbol = match reader.next(&mut bitstream) {
                    Ok(InflateSymbol::EndBlock) => break,
                    Ok(value) => value,
                    Err(error) => return Err(Box::new(error)),
                };

                if let Some(available) = writer.handle(symbol) {
                    let available = std::cmp::min(available, buffer.len());
                    let target = &mut buffer[0..available];

                    let count = writer.take(target);
                    destination.write(&target[..count]).await?;
                }

                if let Some(available) = bitstream.hungry() {
                    let available = std::cmp::min(available, buffer.len());
                    let destination = &mut buffer[0..available];

                    let count = match source.read(destination).await {
                        Ok(count) => count,
                        Err(error) => return Err(Box::new(error)),
                    };

                    bitstream.feed(&buffer[0..count]);
                }
            }

            if reader.is_completed() {
                let count = writer.take(&mut buffer[..]);
                destination.write(&mut buffer[..count]).await?;
                destination.flush().await?;
                break;
            }

            if reader.is_broken() {
                println!("broken");
                break;
            }
        }

        Ok(())
    })
}
