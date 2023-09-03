pub struct BitStream {
    buffer: Box<[u8]>,
    buffer_end: usize,
    current: u8,
    offset: usize,
    offset_bit: u8,
    completed: bool,
    total: u64,
}

impl BitStream {
    pub fn try_from(data: &[u8]) -> Option<Self> {
        let mut buffer = Box::new([0; 131_072]);
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

    pub fn skip_bits(&mut self) -> Option<()> {
        while self.offset_bit != 0x01 {
            self.next_bit()?;
        }

        Some(())
    }

    pub fn next_bytes(&mut self, count: usize) -> Option<Vec<u8>> {
        if self.offset_bit != 0x01 {
            return None;
        }

        let data = match self.buffer.get(self.offset..self.offset + count) {
            Some(data) => data,
            None => return None,
        };

        let mut target = vec![0; data.len()];
        target[..].copy_from_slice(data);

        self.offset += data.len();
        self.total += data.len() as u64;

        self.current = match &self.buffer[0..self.buffer_end].get(self.offset) {
            None => return None,
            Some(&value) => value,
        };

        Some(target)
    }
}
