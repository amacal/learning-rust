use super::bits::*;
use super::array::*;
use super::heap::*;
use super::lexer::*;

struct ElementsMarker;
struct OperatorsMarker;

impl ArrayLike for ElementsMarker {}
impl StackLike for ElementsMarker {}
impl StackLike for OperatorsMarker {}

pub struct RPN<const SIZE: usize>(Array<ElementsMarker, u8, SIZE, GuardSegfault>);

impl<const SIZE: usize> RPN<SIZE> {
    pub fn build(pattern: *const u8) -> Option<Self> {
        let mut builder = Builder::new();

        builder.append(pattern);
        builder.build()
    }

    pub fn builder() -> Builder<SIZE> {
        Builder::new()
    }

    #[cfg(test)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut elements = Array::new();

        for &val in bytes.iter() {
            elements.stack_push_front(val);
        }

        Self(elements)
    }

    #[cfg(test)]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes_front()
    }

    pub fn size(&self) -> u16 {
        self.0.stack_size_front()
    }

    pub fn at(&self, off: u16) -> u8 {
        self.0.array_get(off)
    }
}

enum BuilderState {
    Completed,
    InGroups { concatenation: bool },
    InClasses { negation: bool, bits: Bits },
}

impl BuilderState {
    fn new() -> Self {
        BuilderState::InGroups { concatenation: false }
    }

    fn handle_in_groups<const SIZE: usize>(builder: &mut Builder<SIZE>, lexer: &mut Lexer, concatenation: bool) -> Self {
        let token = match lexer.next_in_group() {
            None => return BuilderState::Completed,
            Some(token) => token,
        };

        match token {
            Token::OpenGroup {} => {
                if concatenation {
                    while builder.operators.stack_size_front() > 0 {
                        match builder.operators.stack_peek_front() {
                            b'&' => {
                                builder.operators.stack_pop_front();
                                builder.elements.stack_push_front(b'&');
                            }
                            _ => break,
                        }
                    }
                }

                builder.operators.stack_push_front(if concatenation { 1 } else { 0 });
                builder.operators.stack_push_front(b'(');

                Self::InGroups { concatenation: false }
            }
            Token::Plus {} => {
                builder.elements.stack_push_front(b'+');
                Self::InGroups { concatenation }
            }
            Token::Optional {} => {
                builder.elements.stack_push_front(b'?');
                Self::InGroups { concatenation }
            }
            Token::Star {} => {
                builder.elements.stack_push_front(b'+');
                builder.elements.stack_push_front(b'?');
                Self::InGroups { concatenation }
            }
            Token::Marker { value } => {
                builder.elements.stack_push_front(b'#');
                builder.elements.stack_push_front(value);
                Self::InGroups { concatenation }
            }
            Token::OpenClass {} => {
                if concatenation {
                    while builder.operators.stack_size_front() > 0 {
                        match builder.operators.stack_peek_front() {
                            b'&' => {
                                builder.operators.stack_pop_front();
                                builder.elements.stack_push_front(b'&');
                            }
                            _ => break,
                        }
                    }
                }

                builder.operators.stack_push_front(if concatenation { 1 } else { 0 });
                builder.operators.stack_push_front(b'[');

                Self::InClasses { negation: false , bits: Bits::new() }
            }
            Token::Either {} => {
                while builder.operators.stack_size_front() > 0 {
                    match builder.operators.stack_peek_front() {
                        b'&' => {
                            let token = builder.operators.stack_pop_front();
                            builder.elements.stack_push_front(token);
                        }
                        _ => break,
                    }
                }

                builder.operators.stack_push_front(b'|');
                Self::InGroups { concatenation: false }
            }
            Token::CloseGroup {} => {
                let mut concatenation = false;

                while builder.operators.stack_size_front() > 0 {
                    match builder.operators.stack_pop_front() {
                        b'(' => {
                            concatenation = builder.operators.stack_pop_front() == 1;
                            break;
                        }
                        value => {
                            builder.elements.stack_push_front(value);
                        }
                    }
                }

                if concatenation {
                    builder.operators.stack_push_front(b'&');
                }

                Self::InGroups { concatenation: true }
            }
            Token::Literal { mut start, mut length } => {
                let mut concatenation = concatenation;

                while length > 0 {
                    let batch = std::cmp::min(length, 9);

                    if concatenation {
                        while builder.operators.stack_size_front() > 0 {
                            match builder.operators.stack_peek_front() {
                                b'&' => {
                                    builder.operators.stack_pop_front();
                                    builder.elements.stack_push_front(b'&');
                                }
                                _ => break,
                            }
                        }
                    }

                    builder.elements.stack_push_front(b'l');
                    builder.elements.stack_push_front(b'0' + batch as u8);

                    for off in 0..batch {
                        unsafe {
                            builder.elements.stack_push_front(*start.add(off));
                        }
                    }

                    if concatenation {
                        builder.operators.stack_push_front(b'&');
                    }

                    unsafe {
                        length -= batch;
                        start = start.add(batch);
                        concatenation = true;
                    }
                }

                Self::InGroups { concatenation: true }
            }
            _ => Self::Completed,
        }
    }

    fn handle_in_classes<const SIZE: usize>(builder: &mut Builder<SIZE>, lexer: &mut Lexer, negation: bool, mut bits: Bits) -> Self {
        let token = match lexer.next_in_class() {
            None => return Self::Completed,
            Some(token) => token,
        };

        match token {
            Token::NegateClass {} => Self::InClasses { negation: true, bits: bits },
            Token::CloseClass {} => {
                let mut alternation = false;
                let (mut min, mut max) = (0u8, 0u8);

                for idx in 1..=255u8 {
                    if bits.get(idx) != negation {
                        if min == 0 {
                            min = idx;
                            max = idx;
                        } else {
                            max = idx;
                        }
                    } else {
                        if min > 0 {
                            builder.elements.stack_push_front(b'-');
                            builder.elements.stack_push_front(min);
                            builder.elements.stack_push_front(max);

                            if alternation {
                                builder.elements.stack_push_front(b'|');
                            } else {
                                alternation = true;
                            }
                        }

                        min = 0;
                        max = 0;
                    }
                }

                if min > 0 {
                    builder.elements.stack_push_front(b'-');
                    builder.elements.stack_push_front(min);
                    builder.elements.stack_push_front(max);

                    if alternation {
                        builder.elements.stack_push_front(b'|');
                    }
                }

                loop {
                    match builder.operators.stack_pop_front() {
                        b'[' => break,
                        value => builder.elements.stack_push_front(value),
                    }
                }

                if builder.operators.stack_pop_front() == 1 {
                    builder.operators.stack_push_front(b'&');
                }

                Self::InGroups { concatenation: true }
            }
            Token::RangeClass { min, max } => {
                for idx in min..=max {
                    bits.set(idx);
                }

                Self::InClasses { negation: negation, bits: bits }
            }
            _ => Self::Completed,
        }
    }

    fn handle<const SIZE: usize>(self, builder: &mut Builder<SIZE>, lexer: &mut Lexer) -> Self {
        match self {
            Self::Completed => Self::Completed,
            Self::InGroups { concatenation } => Self::handle_in_groups(builder, lexer, concatenation),
            Self::InClasses { negation, bits } => Self::handle_in_classes(builder, lexer, negation, bits),
        }
    }
}

pub struct Builder<const SIZE: usize> {
    counter: usize,
    elements: Array<ElementsMarker, u8, SIZE, GuardSegfault>,
    operators: Array<OperatorsMarker, u8, 4096, GuardSegfault>,
}

impl<const SIZE: usize> Builder<SIZE> {
    fn new() -> Self {
        Self { counter: 0, elements: Array::new(), operators: Array::new() }
    }

    pub fn append(&mut self, pattern: *const u8) {
        let mut lexer = Lexer::new(pattern).expect("");
        let mut state = BuilderState::new();

        loop {
            state = match state.handle(self, &mut lexer) {
                BuilderState::Completed => break,
                state => state,
            };
        }

        while self.operators.stack_size_front() > 0 {
            let val = self.operators.stack_pop_front();
            self.elements.stack_push_front(val);
        }

        if self.counter > 0 {
            self.elements.stack_push_front(b'|');
        }

        self.counter = self.counter.wrapping_add(1);
    }

    pub fn build(self) -> Option<RPN<SIZE>> {
        Some(RPN(self.elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_rpn_from_seq_of_one_character() {
        let pattern = b"a\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1a");
    }

    #[test]
    fn handles_rpn_from_seq_of_two_characters() {
        let pattern = b"ab\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab");
    }

    #[test]
    fn handles_rpn_from_seq_of_alphabet() {
        let pattern = b"abcdefghijklmnopqrstuvwxyz\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l9abcdefghil9jklmnopqr&l8stuvwxyz&");
    }

    #[test]
    fn handles_rpn_from_either() {
        let pattern = b"a|b\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b|");
    }

    #[test]
    fn handles_rpn_from_star() {
        let pattern = b"ab*\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b+?&");
    }

    #[test]
    fn handles_rpn_from_plus() {
        let pattern = b"ab+\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b+&");
    }

    #[test]
    fn handles_rpn_from_optional() {
        let pattern = b"ab?\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b?&");
    }

    #[test]
    fn handles_rpn_from_optional_concatenated() {
        let pattern = b"(st)?op\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2st?l2op&");
    }

    #[test]
    fn handles_rpn_from_either_and() {
        let pattern = b"abcdefghijkl|mnopqrstuvwx\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l9abcdefghil3jkl&l9mnopqrstul3vwx&|");
    }

    #[test]
    fn handles_rpn_from_group() {
        let pattern = b"(ab)*|(cd)+\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?l2cd+|");
    }

    #[test]
    fn handles_rpn_from_class() {
        let pattern = b"(ab)*|[a-z]+\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-az+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple() {
        let pattern = b"(ab)*|[a-z0-9]+\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-09-az|+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple_negated() {
        let pattern = b"(ab)*|[^a-z0-9]+\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-\x01\x2f-\x3a\x60|-\x7b\xff|+|");
    }

    #[test]
    fn handles_rpn_from_nested_groups() {
        let pattern = b"(0|(0+1+)+)+\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l10+l11+&+|+");
    }

    #[test]
    fn handles_rpn_from_dividing_by_three_regex() {
        let pattern = b"(0|(1(01*(00)*0)*1)*)*\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l11l10l11+?&l200+?&l10&+?&l11&+?|+?");
    }

    #[test]
    fn handles_rpn_from_zero_followed_by_two_optional_numbers() {
        let pattern = b"01?2?\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l11?&l12?&");
    }

    #[test]
    fn handles_rpn_from_number_regex() {
        let pattern = b"\\-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE](\\+|\\-)?[0-9]+)?\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1-?l10-19-09+?&|&l1.-09+&?&-EE-ee|l1+l1-|?&-09+&?&");
    }

    #[test]
    fn handles_rpn_from_accepting_byte() {
        let pattern = b"ab#x\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#x");
    }

    #[test]
    fn handles_rpn_from_accepting_byte_multiple_group() {
        let pattern = b"(ab#x)|(cd#y)\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#xl2cd#y|");
    }

    #[test]
    fn handles_rpn_from_accepting_byte_multiple_either() {
        let pattern = b"ab#x|cd#y\0".as_ptr();
        let regex = match RPN::<4096>::build(pattern) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#xl2cd#y|");
    }

    #[test]
    fn handles_rpn_from_multiple_appends() {
        let mut builder = RPN::<4096>::builder();

        builder.append(b"ab#x\0".as_ptr());
        builder.append(b"cd#y\0".as_ptr());

        let regex = match builder.build() {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#xl2cd#y|");
    }
}
