pub struct BitStream {
    buffer: Vec<u8>,
    buffer_end: usize,
    current: u8,
    offset: usize,
    offset_bit: u8,
    completed: bool,
    total: u64,
}

impl BitStream {
    pub fn try_from(data: &[u8]) -> Option<Self> {
        let mut buffer = vec![0; 65_536];
        buffer[0..data.len()].copy_from_slice(data);

        let current = match buffer.get(0) {
            Some(&value) => value,
            None => return None,
        };

        Some(Self {
            buffer: buffer,
            buffer_end: data.len(),
            current: current,
            offset: 0,
            offset_bit: 0x01,
            completed: false,
            total: 0,
        })
    }

    pub fn hungry(&self) -> Option<usize> {
        if self.completed {
            return None;
        }

        if self.buffer_end - self.offset > self.buffer.len() / 2 {
            return None;
        }

        Some(self.buffer.len() - (self.buffer_end - self.offset))
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.copy_within(self.offset..self.buffer_end, 0);
        self.buffer_end -= self.offset;
        self.offset = 0;

        if data.len() == 0 {
            self.completed = true;
        } else {
            self.buffer[self.buffer_end..self.buffer_end + data.len()].copy_from_slice(data);
            self.buffer_end += data.len();
        }
    }

    pub fn next_bit(&mut self) -> Option<u8> {
        let bit_set = self.current & self.offset_bit;
        self.offset_bit = (self.offset_bit << 1) | (self.offset_bit >> 7);

        if self.offset_bit == 0x01 {
            self.offset += 1;
            self.total += 1;
            self.current = match &self.buffer[0..self.buffer_end].get(self.offset) {
                None => return None,
                Some(&value) => value,
            };
        }

        Some(if bit_set != 0 { 1 } else { 0 })
    }

    pub fn next_bits(&mut self, count: usize) -> Option<u16> {
        let mut outcome: u16 = 0;

        for i in 0..count {
            outcome = match self.next_bit() {
                Some(bit) => outcome | ((bit as u16) << i),
                None => return None,
            };
        }

        Some(outcome)
    }
}
