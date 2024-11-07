pub struct Tokenizer {
    input: *const u8,
}

impl Tokenizer {
    pub fn new(input: *const u8) -> Self {
        Self { input: input }
    }

    fn at(&self, off: i8) -> u8 {
        unsafe { *self.input.offset(off.into()) }
    }

    pub fn next_in_group(&mut self) -> Option<(*const u8, u8)> {
        let mut off = 0i8;
        let mut value = self.at(0);

        loop {
            match value {
                0 if off == 0 => return None,
                0 => break,
                b'(' | b')' | b'[' | b'|' if off == 0 => break off = 1,
                b'(' | b')' | b'[' | b'|' => break,
                b'*' | b'+' | b'?' if off == 0 => break off = 1,
                b'*' | b'+' | b'?' if off == 1 => break,
                b'*' | b'+' | b'?' => break off = off.wrapping_sub(1),
                _ => (),
            }

            off = off.wrapping_add(1);
            value = self.at(off);

            if off == 9 {
                break;
            }
        }

        unsafe {
            let value = self.input;
            self.input = self.input.offset(off.into());
            Some((value, off as u8))
        }
    }

    pub fn next_in_class(&mut self) -> (u8, u8) {
        let mut result = match self.at(0) {
            0 => (0, 0),
            b'^' => (b'^', 1),
            b']' => (b']', 1),
            value => (value, 3),
        };

        let expanded = result.1 == 3;

        if result.1 == 3 {
            if self.at(1) != b'-' || self.at(2) == 0 || self.at(2) == b']' {
                result = (result.0, 1);
            }
        }

        unsafe {
            self.input = self.input.add(result.1);
            (result.0, if expanded == false { 0 } else { self.at(-1) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_regex_tokenization() {
        let input = b"(ab)*|[a-z_]+|[^abc0-9]+|ab+\0".as_ptr();
        let mut tokenizer = Tokenizer::new(input);

        unsafe {
            assert_eq!(tokenizer.next_in_group(), Some((input.add(0), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(1), 2)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(3), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(4), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(5), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(6), 1)));
            assert_eq!(tokenizer.next_in_class(), (b'a', b'z'));
            assert_eq!(tokenizer.next_in_class(), (b'_', b'_'));
            assert_eq!(tokenizer.next_in_class(), (b']', 0));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(12), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(13), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(14), 1)));
            assert_eq!(tokenizer.next_in_class(), (b'^', 0));
            assert_eq!(tokenizer.next_in_class(), (b'a', b'a'));
            assert_eq!(tokenizer.next_in_class(), (b'b', b'b'));
            assert_eq!(tokenizer.next_in_class(), (b'c', b'c'));
            assert_eq!(tokenizer.next_in_class(), (b'0', b'9'));
            assert_eq!(tokenizer.next_in_class(), (b']', 0));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(23), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(24), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(25), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(26), 1)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(27), 1)));

            assert_eq!(tokenizer.next_in_group(), None);
            assert_eq!(tokenizer.next_in_group(), None);
            assert_eq!(tokenizer.next_in_group(), None);
        }
    }

    #[test]
    fn handles_regex_tokenization_long() {
        let input = b"abcdefghijklmnopqrstuvwxyz\0".as_ptr();
        let mut tokenizer = Tokenizer::new(input);

        unsafe {
            assert_eq!(tokenizer.next_in_group(), Some((input.add(0), 9)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(9), 9)));
            assert_eq!(tokenizer.next_in_group(), Some((input.add(18), 8)));
        }
    }
}
