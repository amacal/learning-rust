mod bitstream;

use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll};

use bitstream_io::{BitRead, BitReader};
use byteorder::{BigEndian, ByteOrder};
use crc32fast;

use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};

#[derive(thiserror::Error, Debug)]
pub enum PngError {
    #[error("IO Failed: {0}")]
    IO(std::io::Error),

    #[error("Invalid Signature")]
    InvalidSignature,

    #[error("Invalid Header")]
    InvalidHeader,

    #[error("Invalid Checksum")]
    InvalidChecksum,

    #[error("Invalid Chunks")]
    InvalidChunks,
}

impl From<PngError> for std::io::Error {
    fn from(err: PngError) -> std::io::Error {
        match err {
            PngError::IO(e) => e,
            _ => std::io::Error::new(std::io::ErrorKind::Other, err),
        }
    }
}

#[derive(Debug)]
pub struct PngFile<TSource: AsyncRead + Unpin> {
    source: TSource,
    header: PngHeader,
    buffer: Vec<u8>,
    buffer_start: usize,
    buffer_end: usize,
    buffer_feeding: bool,
    chunk_hash: crc32fast::Hasher,
    chunk_position: usize,
    chunk_length: usize,
    chunk_verified: bool,
    chunk_completed: bool,
}

impl<TSource: AsyncRead + Unpin> PngFile<TSource> {
    async fn new(mut source: TSource) -> Result<Self, PngError> {
        let mut buffer = vec![0; 65536];
        let signature = [137, 80, 78, 71, 13, 10, 26, 10];

        if let Err(error) = source.read_exact(&mut buffer[0..8]).await {
            return Err(PngError::IO(error));
        }

        if buffer[0..8] != signature {
            return Err(PngError::InvalidSignature);
        }

        let header = Self::read_header(&mut source, &mut buffer).await?;
        let (length, hash) = Self::skip_chunks(&mut source, &mut buffer).await?;

        Ok(Self {
            source: source,
            header: header,
            buffer: buffer,
            buffer_start: 0,
            buffer_end: 0,
            buffer_feeding: false,
            chunk_length: length,
            chunk_position: 0,
            chunk_verified: false,
            chunk_completed: false,
            chunk_hash: hash,
        })
    }
}

impl<TSource: AsyncRead + Unpin> PngFile<TSource> {
    async fn read_header(source: &mut TSource, buffer: &mut [u8]) -> Result<PngHeader, PngError> {
        let signature = [0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52];
        let mut hasher = crc32fast::Hasher::new();

        if let Err(error) = source.read_exact(&mut buffer[0..25]).await {
            return Err(PngError::IO(error));
        }

        if &buffer[0..8] != &signature {
            return Err(PngError::InvalidHeader);
        }

        let crc_computed = {
            hasher.update(&buffer[4..21]);
            hasher.finalize()
        };

        if crc_computed != BigEndian::read_u32(&buffer[21..25]) {
            return Err(PngError::InvalidChecksum);
        }

        Ok(PngHeader::parse(&buffer[8..21]))
    }

    async fn skip_chunks(source: &mut TSource, buffer: &mut [u8]) -> Result<(usize, crc32fast::Hasher), PngError> {
        loop {
            if let Err(error) = source.read_exact(&mut buffer[0..8]).await {
                return Err(PngError::IO(error));
            }

            let mut length = BigEndian::read_u32(&buffer[0..4]) as usize;
            let header = String::from_utf8_lossy(&buffer[4..8]).into_owned();

            if header == "IEND" {
                return Err(PngError::InvalidChunks);
            }

            let mut hasher = crc32fast::Hasher::new();
            hasher.update(&buffer[4..8]);

            if header == "IDAT" {
                return Ok((length, hasher));
            }

            while length > 0 {
                let available = if length < buffer.len() { length } else { buffer.len() };
                let received = source.read(&mut buffer[0..available]).await;

                let received = match received {
                    Ok(received) => received,
                    Err(error) => return Err(PngError::IO(error)),
                };

                length -= received;
                hasher.update(&buffer[0..received]);
            }

            if let Err(error) = source.read_exact(&mut buffer[0..4]).await {
                return Err(PngError::IO(error));
            }

            let crc = BigEndian::read_u32(&buffer[0..4]);
            let crc_computed = hasher.finalize();

            if crc != crc_computed {
                return Err(PngError::InvalidChecksum);
            }
        }
    }

    fn align_buffer(&mut self) {
        if self.buffer_start == self.buffer_end {
            self.buffer_start = 0;
            self.buffer_end = 0;
        }

        if self.buffer_start > self.buffer.len() / 2 {
            self.buffer.copy_within(self.buffer_start..self.buffer_end, 0);
            self.buffer_end -= self.buffer_start;
            self.buffer_start = 0;
        }
    }

    fn verify_chunk(&mut self) -> Result<(), PngError> {
        let mut hasher = crc32fast::Hasher::new();
        std::mem::swap(&mut self.chunk_hash, &mut hasher);

        let buffer = &self.buffer[self.buffer_start..self.buffer_end];
        let crc = BigEndian::read_u32(&buffer[0..4]);
        let crc_computed = hasher.finalize();

        if crc != crc_computed {
            return Err(PngError::InvalidChecksum);
        }

        self.buffer_start += 4;
        self.chunk_verified = true;

        Ok(())
    }

    fn open_chunk(&mut self) -> Result<(), PngError> {
        let buffer = &self.buffer[self.buffer_start..self.buffer_end];
        let length = BigEndian::read_u32(&buffer[0..4]) as usize;
        let header = String::from_utf8_lossy(&buffer[4..8]).into_owned();

        match &header[..] {
            "IDAT" => (),
            "IEND" => {
                self.chunk_length = 0;
                self.chunk_position = 0;
                self.chunk_completed = true;
            }
            _ => return Err(PngError::InvalidChunks),
        }

        self.chunk_verified = false;
        self.chunk_position = 0;
        self.chunk_length = length;
        self.chunk_hash.update(&buffer[4..8]);
        self.buffer_start += 8;

        Ok(())
    }

    fn copy_data<'a>(&mut self, destination: &mut ReadBuf<'a>) {
        let available = std::cmp::min(
            std::cmp::min(
                self.buffer_end - self.buffer_start,
                self.chunk_length - self.chunk_position,
            ),
            destination.remaining(),
        );

        let buffer_end = self.buffer_start + available;
        let source = &self.buffer[self.buffer_start..buffer_end];

        self.chunk_hash.update(source);

        destination.initialized_mut()[0..available].clone_from_slice(source);
        destination.advance(available);

        self.buffer_start += available;
        self.chunk_position += available;
    }
}

impl<TSource: AsyncRead + Unpin> AsyncRead for PngFile<TSource> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        if this.buffer_feeding == false {
            this.align_buffer();
            this.buffer_feeding = true;
        }

        let mut target = ReadBuf::new(&mut this.buffer[this.buffer_end..]);
        let source = Pin::new(&mut this.source);

        let count = match source.poll_read(cx, &mut target) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(result) => match result {
                Err(error) => return Poll::Ready(Err(error)),
                Ok(_) => target.filled().len(),
            },
        };

        this.buffer_feeding = false;
        this.buffer_end += count;

        if this.chunk_position == this.chunk_length && this.chunk_verified == false && this.chunk_completed == false {
            this.verify_chunk()?;
        }

        if this.chunk_position == this.chunk_length && this.chunk_completed == false {
            this.open_chunk()?;
        }

        this.copy_data(buf);
        Poll::Ready(std::io::Result::Ok(()))
    }
}

impl PngFile<File> {
    pub async fn open(path: &str) -> Result<Self, PngError> {
        let file = match File::open(path).await {
            Err(error) => return Err(PngError::IO(error)),
            Ok(file) => file,
        };

        Ok(PngFile::new(file).await?)
    }
}

#[derive(Debug)]
pub struct PngHeader {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    compression_method: u8,
    filter_method: u8,
    interlace_method: u8,
}

impl PngHeader {
    pub fn parse(data: &[u8]) -> Self {
        Self {
            width: BigEndian::read_u32(&data[0..4]),
            height: BigEndian::read_u32(&data[4..8]),
            bit_depth: data[8],
            color_type: data[9],
            compression_method: data[10],
            filter_method: data[11],
            interlace_method: data[12],
        }
    }
}

#[derive(Debug)]
pub struct Deflate<TSource: AsyncRead + Unpin> {
    source: TSource,
    buffer: Vec<u8>,
    buffer_start: usize,
    buffer_end: usize,
    compression_method: u8,
    compression_info: u8,
    check_bits: u8,
    preset_dictionary: u8,
    compression_level: u8,
}

impl<TSource: AsyncRead + Unpin> Deflate<TSource> {
    pub async fn new(mut source: TSource) -> Result<Self, PngError> {
        let mut buffer = vec![0; 1024];

        if let Err(error) = source.read_exact(&mut buffer[0..2]).await {
            return Err(PngError::IO(error));
        }

        let mut reader: BitReader<&[u8], bitstream_io::LittleEndian> = BitReader::new(&buffer);

        let compression_method: u8 = reader.read(4).unwrap();
        let compression_info: u8 = reader.read(4).unwrap();
        let check_bits: u8 = reader.read(5).unwrap();
        let preset_dictionary: u8 = reader.read(1).unwrap();
        let compression_level: u8 = reader.read(2).unwrap();

        Ok(Self {
            source: source,
            buffer: buffer,
            buffer_start: 0,
            buffer_end: 0,
            compression_method: compression_method,
            compression_info: compression_info,
            check_bits: check_bits,
            preset_dictionary: preset_dictionary,
            compression_level: compression_level,
        })
    }

    pub async fn next_data(&mut self) -> Result<(), PngError> {
        let count = match self.source.read(&mut self.buffer).await {
            Ok(count) => count,
            Err(error) => return Err(PngError::IO(error)),
        };

        self.buffer_end += count;
        println!("{}", count);
        println!("{:?}", &self.buffer[..self.buffer_end]);

        let cursor = Cursor::new(&self.buffer[..]);
        let mut reader = bitstream_io::BitReader::endian(cursor, bitstream_io::LittleEndian);
        
        println!("last {}", reader.read::<u8>(1).unwrap());
        println!("mode {}", reader.read::<u8>(2).unwrap());
        
        println!("hlit {}", reader.read::<u8>(5).unwrap());
        println!("hdist {}", reader.read::<u8>(5).unwrap());
        println!("hclen {}", reader.read::<u8>(4).unwrap());

        for i in 0..18 {
            println!("hclen {} {}", i, reader.read::<u8>(3).unwrap());
        }

        println!("{}", reader.position_in_bits().unwrap());

        Ok(())
    }
}

fn rol_u8(value: u8, shift: u8) -> u8 {
    (value << shift) | (value >> (8 - shift))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //let file = PngFile::open("test.png").await?;
    //let mut deflate = Deflate::new(file).await?;

    //deflate.next_data().await?;

    println!("{}", rol_u8(1, 41));
    println!("{}", rol_u8(1, 105));

    Ok(())
}
