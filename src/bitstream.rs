pub struct BitStream<'a> {
    data: &'a [u8],
    current: &'a u8,
    offset: usize,
    offset_bit: u8,
}

impl<'a> BitStream<'a> {
    pub fn try_from(data: &'a [u8]) -> Option<Self> {
        let current = match data.get(0) {
            None => return None,
            Some(value) => value,
        };

        Some(Self {
            data: data,
            current: current,
            offset: 0,
            offset_bit: 0x01,
        })
    }

    pub fn next_bit(&mut self) -> Option<u8> {
        let bit_set = self.current & self.offset_bit;
        self.offset_bit = (self.offset_bit << 1) | (self.offset_bit >> 7);
        
        if self.offset_bit == 0x01 {
            self.offset += 1;
            self.current = match self.data.get(self.offset) {
                None => return None,
                Some(value) => value,
            };
        }

        Some(if bit_set != 0 { 1 }  else { 0 })
    }
}
