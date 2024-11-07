use super::array::*;
use super::heap::*;
use super::lexer::*;

struct ElementsMarker;
struct OperatorsMarker;

impl StackLike for ElementsMarker {}
impl StackLike for OperatorsMarker {}

struct RegexRpn<const SIZE: usize>(Array<ElementsMarker, u8, SIZE, GuardSegfault>);

impl<const SIZE: usize> RegexRpn<SIZE> {
    #[cfg(test)]
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

struct Builder<const SIZE: usize> {
    elements: Array<ElementsMarker, u8, SIZE, GuardSegfault>,
    operators: Array<OperatorsMarker, u8, 4096, GuardSegfault>,
}

impl<const SIZE: usize> Builder<SIZE> {
    fn new() -> Self {
        Self {
            elements: Array::new(),
            operators: Array::new(),
        }
    }

    fn build(mut self, input: *const u8) -> Option<RegexRpn<SIZE>> {
        let mut tokenizer = Tokenizer::new(input);
        let mut in_character = false;
        let mut in_classes = false;
        let mut classes_depth = 0;

        loop {
            if in_classes {
                let token = tokenizer.next_in_class();

                match token {
                    (0, _) => break,
                    (b'^', _) => {
                        self.operators.stack_push(b'^');
                    }
                    (b']', _) => {
                        while self.operators.stack_size() > 0 {
                            match self.operators.stack_peek() {
                                b'^' => {
                                    self.operators.stack_pop();
                                    self.elements.stack_push(b'^');
                                }
                                b'[' => {
                                    self.operators.stack_pop();
                                    break;
                                }
                                _ => break,
                            }
                        }

                        in_classes = false;
                    }
                    (from, to) => {
                        self.elements.stack_push(from);
                        self.elements.stack_push(to);
                        self.elements.stack_push(b'-');
                        classes_depth += 1;

                        if classes_depth > 1 {
                            self.elements.stack_push(b'@');
                        }
                    }
                }
            } else {
                let token = if let Some(token) = tokenizer.next_in_group() {
                    unsafe { (*token.0, token.0, token.1) }
                } else {
                    break;
                };

                match token.0 {
                    b'(' | b'*' | b'+' | b'?' => {
                        self.operators.stack_push(token.0);
                        in_character = false;
                    }
                    b'[' => {
                        self.operators.stack_push(token.0);
                        in_character = false;
                        in_classes = true;
                        classes_depth = 0;
                    }
                    b'|' => {
                        while self.operators.stack_size() > 0 {
                            match self.operators.stack_peek() {
                                b'*' | b'+' | b'?' | b'&' => {
                                    let token = self.operators.stack_pop();
                                    self.elements.stack_push(token);
                                }
                                b'(' => break,
                                _ => {}
                            }
                        }

                        self.operators.stack_push(b'|');
                        in_character = false;
                    }
                    b')' => {
                        while self.operators.stack_size() > 0 {
                            match self.operators.stack_peek() {
                                b'*' | b'+' | b'?' | b'&' => {
                                    let token = self.operators.stack_pop();
                                    self.elements.stack_push(token);
                                }
                                b'(' => {
                                    self.operators.stack_pop();
                                    break;
                                }
                                _ => break,
                            }
                        }

                        in_character = false;
                    }
                    _ if in_character => {
                        for off in 0..token.2 {
                            unsafe {
                                self.elements.stack_push(*token.1.add(off.into()));
                            }
                        }

                        self.operators.stack_push(b'&');
                        self.elements.stack_push(b'0' + token.2);
                        self.elements.stack_push(b'l');
                    }
                    _ => {
                        for off in 0..token.2 {
                            unsafe {
                                self.elements.stack_push(*token.1.add(off.into()));
                            }
                        }
                        in_character = true;
                        self.elements.stack_push(b'0' + token.2);
                        self.elements.stack_push(b'l');
                    }
                }
            }
        }

        while self.operators.stack_size() > 0 {
            let val = self.operators.stack_pop();
            self.elements.stack_push(val);
        }

        Some(RegexRpn(self.elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_rpn_from_seq_one_character() {
        let regex = b"a\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"a1l");
    }

    #[test]
    fn handles_rpn_from_seq_two_characters() {
        let regex = b"ab\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"ab2l");
    }

    #[test]
    fn handles_rpn_from_seq_alphabet() {
        let regex = b"abcdefghijklmnopqrstuvwxyz\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"abcdefghi9ljklmnopqr9lstuvwxyz8l&&");
    }

    #[test]
    fn handles_rpn_from_either() {
        let regex = b"a|b\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"a1lb1l|");
    }

    #[test]
    fn handles_rpn_from_star() {
        let regex = b"ab*\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"a1lb1l*&");
    }

    #[test]
    fn handles_rpn_from_plus() {
        let regex = b"ab+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"a1lb1l+&");
    }

    #[test]
    fn handles_rpn_from_optional() {
        let regex = b"ab?\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"a1lb1l?&");
    }

    #[test]
    fn handles_rpn_from_either_and() {
        let regex = b"abcdefghijkl|mnopqrstuvwx\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"abcdefghi9ljkl3l&mnopqrstu9lvwx3l&|");
    }

    #[test]
    fn handles_rpn_from_group() {
        let regex = b"(ab)*|(cd)+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"ab2l*cd2l+|");
    }

    #[test]
    fn handles_rpn_from_class() {
        let regex = b"(ab)*|[a-z]+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"ab2l*az-+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple() {
        let regex = b"(ab)*|[a-z0-9]+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"ab2l*az-09-@+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple_negated() {
        let regex = b"(ab)*|[^a-z0-9]+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"ab2l*az-09-@^+|");
    }
}
