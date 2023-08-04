use byteorder::{ByteOrder, LittleEndian};
use futures::stream::{unfold, StreamExt};
use std::{borrow::Cow, io::SeekFrom, pin::Pin};

use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, AsyncSeekExt, ReadBuf, AsyncWriteExt},
};

#[derive(Debug)]
pub struct BitmapFileHeader {
    pub bf_type: u16,
    pub bf_size: u32,
    pub bf_reserved1: u16,
    pub bf_reserved2: u16,
    pub bf_off_bits: u32,
}

impl BitmapFileHeader {
    pub fn parse(data: &[u8]) -> Self {
        Self {
            bf_type: LittleEndian::read_u16(&data[0..2]),
            bf_size: LittleEndian::read_u32(&data[2..6]),
            bf_reserved1: LittleEndian::read_u16(&data[6..8]),
            bf_reserved2: LittleEndian::read_u16(&data[8..10]),
            bf_off_bits: LittleEndian::read_u32(&data[10..14]),
        }
    }
}

#[derive(Debug)]
pub struct BitmapInfoHeader {
    pub bi_size: u32,
    pub bi_width: i32,
    pub bi_height: i32,
    pub bi_planes: u16,
    pub bi_bit_count: u16,
    pub bi_compression: u32,
    pub bi_size_image: u32,
    pub bi_x_pels_per_meter: i32,
    pub bi_y_pels_per_meter: i32,
    pub bi_clr_used: u32,
    pub bi_clr_important: u32,
}

impl BitmapInfoHeader {
    pub fn parse(data: &[u8]) -> Self {
        BitmapInfoHeader {
            bi_size: LittleEndian::read_u32(&data[0..4]),
            bi_width: LittleEndian::read_i32(&data[4..8]),
            bi_height: LittleEndian::read_i32(&data[8..12]),
            bi_planes: LittleEndian::read_u16(&data[12..14]),
            bi_bit_count: LittleEndian::read_u16(&data[14..16]),
            bi_compression: LittleEndian::read_u32(&data[16..20]),
            bi_size_image: LittleEndian::read_u32(&data[20..24]),
            bi_x_pels_per_meter: LittleEndian::read_i32(&data[24..28]),
            bi_y_pels_per_meter: LittleEndian::read_i32(&data[28..32]),
            bi_clr_used: LittleEndian::read_u32(&data[32..36]),
            bi_clr_important: LittleEndian::read_u32(&data[36..40]),
        }
    }
}

pub async fn extract_metadata(file: &mut File) -> Result<(BitmapFileHeader, BitmapInfoHeader), std::io::Error> {
    let mut buffer: [u8; 2048] = [0; 2048];

    file.seek(SeekFrom::Start(0)).await?;
    file.read(&mut buffer).await?;

    let file_header = BitmapFileHeader::parse(&buffer[..]);
    let info_header = BitmapInfoHeader::parse(&buffer[0x0e..]);

    file.seek(SeekFrom::Start(file_header.bf_off_bits as u64)).await?;
    Ok((file_header, info_header))
}

#[derive(Debug)]
pub enum RlePacket<'a> {
    Encoded { count: usize, value: u8 },
    Absolute { data: Cow<'a, [u8]> },
    EndOfLine,
    EndOfBitmap,
}

impl<'a> RlePacket<'a> {
    fn from<'b>(packet: &RlePacket<'a>) -> RlePacket<'b> {
        match packet {
            Self::Encoded { count, value } => RlePacket::Encoded {
                count: *count,
                value: *value,
            },
            Self::Absolute { data } => RlePacket::Absolute {
                data: Cow::Owned(data.to_vec()),
            },
            Self::EndOfLine => RlePacket::EndOfLine,
            Self::EndOfBitmap => RlePacket::EndOfBitmap,
        }
    }

    pub fn decode<'b>(source: &'b [u8]) -> Option<RlePacket<'b>> {
        if source.len() < 2 {
            return None;
        }

        if source[0] > 0 {
            return Some(RlePacket::Encoded {
                count: source[0] as usize,
                value: source[1],
            });
        }

        if source[1] == 0 {
            return Some(RlePacket::EndOfLine);
        }

        if source[1] == 1 {
            return Some(RlePacket::EndOfBitmap);
        }

        if source[1] > 0 && source.len() >= 2 + source[1] as usize {
            return Some(RlePacket::Absolute {
                data: Cow::Borrowed(&source[2..2 + source[1] as usize]),
            });
        }

        None
    }

    pub fn data_size(&self) -> usize {
        match self {
            Self::Encoded { count, value: _value } => *count,
            Self::Absolute { data } => data.len(),
            _ => 0,
        }
    }

    pub fn packet_size(&self) -> usize {
        match self {
            Self::Absolute { data } => 2 + data.len() + (data.len() % 2),
            _ => 2,
        }
    }

    pub fn write(&'a self, buf: &mut ReadBuf<'_>) -> Option<usize> {
        match self {
            Self::Absolute { data } if buf.remaining() >= data.len() => {
                match data {
                    Cow::Borrowed(data) => buf.put_slice(data),
                    Cow::Owned(data) => buf.put_slice(&data),
                }
                Some(data.len())
            }
            Self::Encoded { count, value } if buf.remaining() >= *count => {
                let data = vec![*value; *count];
                buf.put_slice(&data);
                Some(*count)
            }
            Self::EndOfLine | Self::EndOfBitmap => Some(0),
            _ => None,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RleDecoderError {
    #[error("IO Failed: {0}")]
    IO(std::io::Error),
}

#[derive(Debug)]
pub struct RleDecoder<TSource: AsyncRead + Sized + Unpin> {
    source: TSource,
    file_header: BitmapFileHeader,
    info_header: BitmapInfoHeader,
    buffer: Vec<u8>,
    position: usize,
    start: usize,
    end: usize,
}

impl<TSource: AsyncRead + Unpin> RleDecoder<TSource> {
    async fn fetch(&mut self) -> Result<(), RleDecoderError> {
        if self.start > self.buffer.len() / 2 {
            self.buffer.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        }

        self.end += match self.source.read(&mut self.buffer[self.end..]).await {
            Ok(count) => count,
            Err(error) => return Err(RleDecoderError::IO(error)),
        };

        Ok(())
    }

    async fn fill<'a>(&'a mut self, packets: &mut Vec<RlePacket<'a>>) {
        loop {
            let buffer = &self.buffer[self.start..self.end];
            let packet = RlePacket::decode(buffer);

            if let Some(packet) = packet {
                self.start += packet.packet_size();
                self.position += packet.data_size();

                packets.push(packet);
                continue;
            }

            break;
        }
    }

    pub async fn next_packets<'a>(&'a mut self) -> Result<Vec<RlePacket<'a>>, RleDecoderError> {
        let mut packets = Vec::new();

        self.fetch().await?;
        self.fill(&mut packets).await;

        Ok(packets)
    }
}

impl RleDecoder<File> {
    fn from(source: File, file_header: BitmapFileHeader, info_header: BitmapInfoHeader) -> Self {
        Self {
            source: source,
            buffer: vec![0; 65536],
            file_header: file_header,
            info_header: info_header,
            position: 0,
            start: 0,
            end: 0,
        }
    }

    pub async fn file(name: &str) -> Result<Self, std::io::Error> {
        let mut file = File::open(name).await?;
        let (file_header, info_header) = extract_metadata(&mut file).await?;

        Ok(RleDecoder::from(file, file_header, info_header))
    }

    pub async fn seek(&mut self, offset: usize) -> Result<(), std::io::Error> {
        self.source.seek(SeekFrom::Start(offset as u64)).await?;
        self.position = offset - self.file_header.bf_off_bits as usize;

        self.start = 0;
        self.end = 0;

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct BitmapRow {
    pub bitmap_line: usize,
    pub source_offset: usize,
    pub target_offset: usize,
}

pub async fn decode_packets<'a>(
    decoder: &'a mut RleDecoder<File>,
) -> Result<Pin<Box<impl futures::Stream<Item = RlePacket>>>, RleDecoderError> {
    let data = unfold(decoder, |decoder| async {
        let packets = decoder.next_packets().await.unwrap();
        let mut cloned = Vec::with_capacity(packets.len());

        for packet in &packets {
            cloned.push(RlePacket::from(packet));
        }

        if packets.is_empty() {
            None
        } else {
            Some((cloned, decoder))
        }
    });

    Ok(Box::pin(data.flat_map(|packets| futures::stream::iter(packets))))
}

pub async fn handle_packets(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder: RleDecoder<File> = RleDecoder::file(name).await?;
    let mut packets = decode_packets(&mut decoder).await?;
    let mut packets = packets.as_mut();

    while let Some(packet) = packets.next().await {
        println!("{:?}", packet);
    }

    Ok(())
}

pub async fn decode_rows(decoder: &mut RleDecoder<File>) -> Result<Vec<BitmapRow>, RleDecoderError> {
    let mut counter = 0;
    let mut offset = decoder.file_header.bf_off_bits as usize;

    let mut prev = decoder.file_header.bf_off_bits as usize;
    let mut indices = vec![BitmapRow::default(); decoder.info_header.bi_height as usize];

    let mut streaming = true;
    let height = decoder.info_header.bi_height as usize;
    let width = decoder.info_header.bi_width as usize;

    while streaming  {
        for packet in decoder.next_packets().await? {
            offset += packet.packet_size();

            if let RlePacket::EndOfLine | RlePacket::EndOfBitmap = packet {
                counter += 1;
                indices[height - counter] = BitmapRow {
                    source_offset: prev,
                    bitmap_line: height - counter,
                    target_offset: (height - counter) * width,
                };
                prev = offset;
            }

            if let RlePacket::EndOfBitmap = packet {
                streaming = false;
            }
        }
    }

    Ok(indices)
}

pub async fn handle_rows(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder: RleDecoder<File> = RleDecoder::file(name).await?;
    let rows = decode_rows(&mut decoder).await?;

    for row in &rows {
        println!("{:?}", row);
    }

    Ok(())
}

pub async fn handle_data(name: &str, outcome: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder: RleDecoder<File> = RleDecoder::file(name).await?;
    let rows = decode_rows(&mut decoder).await?;

    let mut buffer = vec![0; decoder.info_header.bi_width as usize];
    let mut buffer = ReadBuf::new(&mut buffer);

    let mut streaming;
    let mut outcome = File::create(outcome).await?;

    for row in &rows {
        streaming = true;
        decoder.seek(row.source_offset).await?;

        println!("{:?}", row);

        while streaming {
            for packet in decoder.next_packets().await? {
                if let RlePacket::EndOfLine | RlePacket::EndOfBitmap = packet {
                    streaming = false;
                    break;
                }

                packet.write(&mut buffer);
            }
        }

        outcome.write_all(buffer.filled()).await?;
        buffer.clear();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    handle_rows("test.bmp").await?;
    handle_packets("test.bmp").await?;
    handle_data("test.bmp", "test.data").await?;

    Ok(())
}
