use super::array::*;
use super::bits::*;
use super::error::*;
use super::lexer::*;

use super::alloc::Allocator;
use super::heap::AllocatorSize;
use super::heap::GuardSegfault;

struct ElementsMarker;
struct OperatorsMarker;

impl ArrayLike for ElementsMarker {}
impl StackLike for ElementsMarker {}
impl StackLike for OperatorsMarker {}

pub struct RPN<ALLOCATOR: Allocator, SIZE: AllocatorSize>(Array<ElementsMarker, u8, ALLOCATOR, SIZE, GuardSegfault>);

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize> RPN<ALLOCATOR, SIZE> {
    pub fn build(allocator: ALLOCATOR, pattern: *const u8) -> Result<Self, Error>
    where
        ALLOCATOR: Copy,
    {
        let mut builder = match Builder::new(allocator) {
            None => return Err(Error::NotEnoughHeap {}),
            Some(builder) => builder,
        };

        builder.append(pattern)?;
        builder.build()
    }

    pub fn builder(allocator: ALLOCATOR) -> Option<Builder<ALLOCATOR, SIZE>>
    where
        ALLOCATOR: Copy,
    {
        Builder::new(allocator)
    }

    #[cfg(test)]
    pub fn from_bytes(allocator: ALLOCATOR, bytes: &[u8]) -> Option<Self> {
        let mut elements = Array::new(allocator)?;

        for &val in bytes.iter() {
            elements.stack_push_front(val);
        }

        Some(Self(elements))
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
    Invalid,
    InGroups { concatenation: bool },
    InClasses { negation: bool, bits: Bits },
}

impl BuilderState {
    fn new() -> Self {
        BuilderState::InGroups { concatenation: false }
    }

    fn pop_concatenation<ALLOCATOR: Allocator, SIZE: AllocatorSize>(builder: &mut Builder<ALLOCATOR, SIZE>, condition: bool) {
        if condition {
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
    }

    fn handle_in_groups<ALLOCATOR: Allocator, SIZE: AllocatorSize>(builder: &mut Builder<ALLOCATOR, SIZE>, token: Token, concatenation: bool) -> Self {
        match token {
            Token::OpenGroup {} => {
                Self::pop_concatenation(builder, concatenation);

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
                Self::pop_concatenation(builder, concatenation);

                builder.operators.stack_push_front(if concatenation { 1 } else { 0 });
                builder.operators.stack_push_front(b'[');

                Self::InClasses { negation: false, bits: Bits::new() }
            }
            Token::Either {} => {
                Self::pop_concatenation(builder, true);

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
                    Self::pop_concatenation(builder, concatenation);

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
            _ => BuilderState::Invalid,
        }
    }

    fn handle_in_classes<ALLOCATOR: Allocator, SIZE: AllocatorSize>(
        builder: &mut Builder<ALLOCATOR, SIZE>,
        token: Token,
        negation: bool,
        mut bits: Bits,
    ) -> Self {
        match token {
            Token::NegateClass {} => {
                if negation || bits.any() {
                    Self::Invalid
                } else {
                    Self::InClasses { negation: true, bits: bits }
                }
            }
            Token::CloseClass {} => {
                let mut alternation = false;
                let mut iterator = bits.area(negation);

                while let Some((min, max)) = iterator.next() {
                    builder.elements.stack_push_front(b'-');
                    builder.elements.stack_push_front(min);
                    builder.elements.stack_push_front(max);

                    if alternation {
                        builder.elements.stack_push_front(b'|');
                    } else {
                        alternation = true;
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
            _ => BuilderState::Invalid,
        }
    }

    fn handle<ALLOCATOR: Allocator, SIZE: AllocatorSize>(self, builder: &mut Builder<ALLOCATOR, SIZE>, token: Token) -> Self {
        match self {
            Self::Invalid => Self::Invalid,
            Self::Completed => Self::Completed,
            Self::InGroups { concatenation } => Self::handle_in_groups(builder, token, concatenation),
            Self::InClasses { negation, bits } => Self::handle_in_classes(builder, token, negation, bits),
        }
    }
}

pub struct Builder<ALLOCATOR: Allocator, SIZE: AllocatorSize> {
    counter: usize,
    elements: Array<ElementsMarker, u8, ALLOCATOR, SIZE, GuardSegfault>,
    operators: Array<OperatorsMarker, u8, ALLOCATOR, SIZE, GuardSegfault>,
}

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize> Builder<ALLOCATOR, SIZE> {
    fn new(allocator: ALLOCATOR) -> Option<Self>
    where
        ALLOCATOR: Copy,
    {
        Some(Self { counter: 0, elements: Array::new(allocator)?, operators: Array::new(allocator)? })
    }

    pub fn append(&mut self, pattern: *const u8) -> Result<(), Error> {
        let mut lexer = Lexer::new(pattern);
        let mut state = BuilderState::new();

        loop {
            let token = match lexer.next() {
                Err(error) => return Err(error),
                Ok(Some(token)) => token,
                Ok(None) => break,
            };

            state = match state.handle(self, token) {
                BuilderState::Completed => break,
                BuilderState::Invalid => return Err(Error::InvalidRegex {}),
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

        self.counter += 1;
        Ok(())
    }

    pub fn build(self) -> Result<RPN<ALLOCATOR, SIZE>, Error> {
        Ok(RPN(self.elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::heap::B4096;
    use super::super::alloc::Naive64Pages;

    #[test]
    fn handles_rpn_from_seq_of_one_character() {
        let allocator = Naive64Pages::new();
        let pattern = b"a\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1a");
    }

    #[test]
    fn handles_rpn_from_seq_of_two_characters() {
        let allocator = Naive64Pages::new();
        let pattern = b"ab\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab");
    }

    #[test]
    fn handles_rpn_from_seq_of_alphabet() {
        let allocator = Naive64Pages::new();
        let pattern = b"abcdefghijklmnopqrstuvwxyz\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l9abcdefghil9jklmnopqr&l8stuvwxyz&");
    }

    #[test]
    fn handles_rpn_from_either() {
        let allocator = Naive64Pages::new();
        let pattern = b"a|b\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b|");
    }

    #[test]
    fn handles_rpn_from_star() {
        let allocator = Naive64Pages::new();
        let pattern = b"ab*\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b+?&");
    }

    #[test]
    fn handles_rpn_from_plus() {
        let allocator = Naive64Pages::new();
        let pattern = b"ab+\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b+&");
    }

    #[test]
    fn handles_rpn_from_optional() {
        let allocator = Naive64Pages::new();
        let pattern = b"ab?\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b?&");
    }

    #[test]
    fn handles_rpn_from_optional_concatenated() {
        let allocator = Naive64Pages::new();
        let pattern = b"(st)?op\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2st?l2op&");
    }

    #[test]
    fn handles_rpn_from_either_and() {
        let allocator = Naive64Pages::new();
        let pattern = b"abcdefghijkl|mnopqrstuvwx\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l9abcdefghil3jkl&l9mnopqrstul3vwx&|");
    }

    #[test]
    fn handles_rpn_from_group() {
        let allocator = Naive64Pages::new();
        let pattern = b"(ab)*|(cd)+\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?l2cd+|");
    }

    #[test]
    fn handles_rpn_from_class() {
        let allocator = Naive64Pages::new();
        let pattern = b"(ab)*|[a-z]+\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-az+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple() {
        let allocator = Naive64Pages::new();
        let pattern = b"(ab)*|[a-z0-9]+\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-09-az|+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple_negated() {
        let allocator = Naive64Pages::new();
        let pattern = b"(ab)*|[^a-z0-9]+\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-\x01\x2f-\x3a\x60|-\x7b\xff|+|");
    }

    #[test]
    fn handles_rpn_from_nested_groups() {
        let allocator = Naive64Pages::new();
        let pattern = b"(0|(0+1+)+)+\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l10+l11+&+|+");
    }

    #[test]
    fn handles_rpn_from_dividing_by_three_regex() {
        let allocator = Naive64Pages::new();
        let pattern = b"(0|(1(01*(00)*0)*1)*)*\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l11l10l11+?&l200+?&l10&+?&l11&+?|+?");
    }

    #[test]
    fn handles_rpn_from_zero_followed_by_two_optional_numbers() {
        let allocator = Naive64Pages::new();
        let pattern = b"01?2?\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l11?&l12?&");
    }

    #[test]
    fn handles_rpn_from_number_regex() {
        let allocator = Naive64Pages::new();
        let pattern = b"\\-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE](\\+|\\-)?[0-9]+)?\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1-?l10-19-09+?&|&l1.-09+&?&-EE-ee|l1+l1-|?&-09+&?&");
    }

    #[test]
    fn handles_rpn_from_accepting_byte() {
        let allocator = Naive64Pages::new();
        let pattern = b"ab#x\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#x");
    }

    #[test]
    fn handles_rpn_from_accepting_byte_multiple_group() {
        let allocator = Naive64Pages::new();
        let pattern = b"(ab#x)|(cd#y)\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#xl2cd#y|");
    }

    #[test]
    fn handles_rpn_from_accepting_byte_multiple_either() {
        let allocator = Naive64Pages::new();
        let pattern = b"ab#x|cd#y\0".as_ptr();

        let regex = match RPN::<_, B4096>::build(&allocator, pattern) {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#xl2cd#y|");
    }

    #[test]
    fn handles_rpn_from_multiple_appends() {
        let allocator = Naive64Pages::new();
        let mut builder = RPN::<_, B4096>::builder(&allocator).unwrap();

        builder.append(b"ab#x\0".as_ptr()).unwrap();
        builder.append(b"cd#y\0".as_ptr()).unwrap();

        let regex = match builder.build() {
            Ok(regex) => regex,
            _ => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab#xl2cd#y|");
    }

    #[test]
    fn handles_rpn_error_from_double_negated_class() {
        let allocator = Naive64Pages::new();
        let pattern = b"[^^]\0".as_ptr();

        match RPN::<_, B4096>::build(&allocator, pattern) {
            Err(Error::InvalidRegex {}) => {}
            _ => return assert!(false),
        };
    }

    #[test]
    fn handles_rpn_error_from_negated_in_the_middle_class() {
        let allocator = Naive64Pages::new();
        let pattern = b"[a-z^]\0".as_ptr();

        match RPN::<_, B4096>::build(&allocator, pattern) {
            Err(Error::InvalidRegex {}) => {}
            _ => return assert!(false),
        };
    }
}
