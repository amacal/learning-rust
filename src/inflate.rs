use crate::bitstream::BitStream;
use crate::huffman::HuffmanTable;

enum InflateDecoder {
    Huffman { huffman: InflateHuffman },
}

impl InflateDecoder {
    fn huffman(literals: HuffmanTable<16, 288>, distances: HuffmanTable<16, 32>) -> Self {
        Self::Huffman {
            huffman: InflateHuffman {
                literals: literals,
                distances: distances,
            },
        }
    }
}

struct InflateHuffman {
    literals: HuffmanTable<16, 288>,
    distances: HuffmanTable<16, 32>,
}

pub struct InflateBlock<'a> {
    pub last: bool,
    pub mode: u8,
    reader: &'a mut BitStream,
    decoder: InflateDecoder,
}

#[derive(Debug)]
pub enum InflateSymbol {
    EndBlock,
    Literal { value: u16 },
    Match { length: u16, distance: u16 },
}

#[derive(Debug, thiserror::Error)]
pub enum InflateError {
    #[error("Not Enough Data: {0}")]
    NotEnoughData(String),
    #[error("Invalid Table: {0} / {1}")]
    InvalidTable(String, String),
    #[error("Not Implemented Protocol: {0}")]
    NotImplementedProtocol(String),
}

pub type InflateResult<T> = Result<T, InflateError>;

fn raise_not_enough_data<T>(description: &str) -> InflateResult<T> {
    Err(InflateError::NotEnoughData(description.to_string()))
}

fn raise_invalid_table<T>(table_name: &str, description: String) -> InflateResult<T> {
    Err(InflateError::InvalidTable(table_name.to_string(), description))
}

fn raise_not_implemented_protocol<T>(description: &str) -> InflateResult<T> {
    Err(InflateError::NotImplementedProtocol(description.to_string()))
}

fn build_fixed_tables() -> (HuffmanTable<16, 288>, HuffmanTable<16, 32>) {
    let mut literals = [0; 288];
    let mut distances = [0; 32];

    for i in 0..144 {
        literals[i] = 8;
    }

    for i in 144..256 {
        literals[i] = 9;
    }

    for i in 256..280 {
        literals[i] = 7;
    }

    for i in 280..288 {
        literals[i] = 8;
    }

    for i in 0..distances.len() {
        distances[i] = 5;
    }

    (HuffmanTable::new(literals), HuffmanTable::new(distances))
}

fn build_length_table(hlen: usize, reader: &mut BitStream) -> Option<HuffmanTable<8, 19>> {
    let mut lengths: [u16; 19] = [0; 19];
    let mapping = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

    for i in 0..hlen {
        lengths[mapping[i]] = match reader.next_bits(3) {
            Some(bits) => bits as u16,
            None => return None,
        };
    }

    Some(HuffmanTable::new(lengths))
}

fn build_dynamic_table<const T: usize>(
    reader: &mut BitStream,
    lengths: &HuffmanTable<8, 19>,
    count: usize,
) -> InflateResult<HuffmanTable<16, T>> {
    let mut index: usize = 0;
    let mut symbols = [0; T];

    while index < count {
        let value = match lengths.decode(reader) {
            Some(value) => value,
            None => return raise_not_enough_data("decoding value"),
        };

        let (repeat, times) = match value {
            0..=15 => (0, 0),
            16 => match reader.next_bits(2) {
                Some(value) => (symbols[index - 1], 3 + value),
                None => return raise_not_enough_data("reading zeros"),
            },
            17 => match reader.next_bits(3) {
                Some(value) => (0, 3 + value),
                None => return raise_not_enough_data("reading zeros"),
            },
            18 => match reader.next_bits(7) {
                Some(value) => (0, 11 + value),
                None => return raise_not_enough_data("reading zeros"),
            },
            value => return raise_not_implemented_protocol(format!("symbol {} in the alphabet", value).as_str()),
        };

        if times == 0 {
            symbols[index] = value;
            index += 1;
        }

        for _ in 0..times {
            symbols[index] = repeat;
            index += 1;
        }
    }

    Ok(HuffmanTable::new(symbols))
}

fn build_dynamic_tables(reader: &mut BitStream) -> InflateResult<(HuffmanTable<16, 288>, HuffmanTable<16, 32>)> {
    let hlit = match reader.next_bits(5) {
        Some(bits) => 257 + bits as usize,
        None => return raise_not_enough_data("hlit value"),
    };

    let hdist = match reader.next_bits(5) {
        Some(bits) => 1 + bits as usize,
        None => return raise_not_enough_data("hdist value"),
    };

    let hclen = match reader.next_bits(4) {
        Some(bits) => 4 + bits as usize,
        None => return raise_not_enough_data("hclen value"),
    };

    let lengths = match build_length_table(hclen, reader) {
        Some(lengths) => lengths,
        None => return raise_not_enough_data("building lengths table"),
    };

    let literals = match build_dynamic_table(reader, &lengths, hlit) {
        Ok(table) => table,
        Err(error) => return raise_invalid_table("literals", error.to_string()),
    };

    let distances = match build_dynamic_table(reader, &lengths, hdist) {
        Ok(table) => table,
        Err(error) => return raise_invalid_table("distances", error.to_string()),
    };

    Ok((literals, distances))
}

impl<'a> InflateBlock<'a> {
    pub fn open(reader: &'a mut BitStream) -> InflateResult<Self> {
        let last = match reader.next_bit() {
            Some(bit) => bit == 1,
            None => return raise_not_enough_data("last_block bit"),
        };

        let mode = match reader.next_bits(2) {
            Some(bits) => bits as u8,
            None => return raise_not_enough_data("mode bit"),
        };

        let decoder = match mode {
            1 => {
                let (literals, distances) = build_fixed_tables();
                InflateDecoder::huffman(literals, distances)
            }
            2 => match build_dynamic_tables(reader) {
                Ok((literals, distances)) => InflateDecoder::huffman(literals, distances),
                Err(error) => return Err(error),
            },
            _ => return raise_not_enough_data("unknown mode"),
        };

        Ok(Self {
            last: last,
            mode: mode,
            reader: reader,
            decoder: decoder,
        })
    }

    pub fn next(&mut self) -> Option<InflateSymbol> {
        match &self.decoder {
            InflateDecoder::Huffman { huffman } => huffman.next(&mut self.reader),
        }
    }

    pub fn hungry(&mut self) -> Option<usize> {
        self.reader.hungry()
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.reader.feed(data)
    }
}

impl InflateHuffman {
    fn decode_length(value: u16, extra: u16) -> Option<u16> {
        match value {
            257..=264 => Some((value - 257) + 3 + extra),
            265..=268 => Some((value - 265) * 2 + 11 + extra),
            269..=272 => Some((value - 269) * 4 + 19 + extra),
            273..=276 => Some((value - 273) * 8 + 35 + extra),
            277..=280 => Some((value - 277) * 16 + 67 + extra),
            281..=284 => Some((value - 281) * 32 + 131 + extra),
            285 => Some(258),
            _ => None,
        }
    }

    fn decode_distance(value: u16, extra: u16) -> Option<u16> {
        match value {
            0..=3 => Some(value + 1),
            4..=5 => Some((value - 4) * 2 + 5 + extra),
            6..=7 => Some((value - 6) * 4 + 9 + extra),
            8..=9 => Some((value - 8) * 8 + 17 + extra),
            10..=11 => Some((value - 10) * 16 + 33 + extra),
            12..=13 => Some((value - 12) * 32 + 65 + extra),
            14..=15 => Some((value - 14) * 64 + 129 + extra),
            16..=17 => Some((value - 16) * 128 + 257 + extra),
            18..=19 => Some((value - 18) * 256 + 513 + extra),
            20..=21 => Some((value - 20) * 512 + 1025 + extra),
            22..=23 => Some((value - 22) * 1024 + 2049 + extra),
            24..=25 => Some((value - 24) * 2048 + 4097 + extra),
            26..=27 => Some((value - 26) * 4096 + 8193 + extra),
            28..=29 => Some((value - 28) * 8192 + 16385 + extra),
            _ => None,
        }
    }

    fn decode(&self, reader: &mut BitStream, length: u16) -> Option<InflateSymbol> {
        let length_bits = (std::cmp::max(length, 261) as usize - 261) / 4;
        let length_bits = if length_bits == 6 { 0 } else { length_bits };

        let length_extra = match reader.next_bits(length_bits) {
            Some(bits) => bits as u16,
            None => return None,
        };

        let distance = match self.distances.decode(reader) {
            Some(value) => value,
            None => return None,
        };

        let distance_bits = if distance < 4 { 0 } else { (distance as usize - 2) / 2 };
        let distance_extra = match reader.next_bits(distance_bits) {
            Some(value) => value as u16,
            None => return None,
        };

        let length = match Self::decode_length(length, length_extra) {
            Some(length) => length,
            None => return None,
        };

        let distance = match Self::decode_distance(distance, distance_extra) {
            Some(distance) => distance,
            None => return None,
        };

        Some(InflateSymbol::Match {
            length: length,
            distance: distance,
        })
    }

    fn next(&self, reader: &mut BitStream) -> Option<InflateSymbol> {
        let literal = match self.literals.decode(reader) {
            Some(value) => value,
            None => return None,
        };

        match literal {
            256 => Some(InflateSymbol::EndBlock),
            0..=255 => Some(InflateSymbol::Literal { value: literal }),
            value => self.decode(reader, value),
        }
    }
}
