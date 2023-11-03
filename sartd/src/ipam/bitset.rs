use thiserror::Error;

#[derive(Debug)]
pub(super) struct BitSet {
    inner: u128,
    size: usize,
}

impl BitSet {
    fn new(size: usize) -> BitSet {
        BitSet { inner: 0, size }
    }

    fn with_value(value: u128, size: usize) -> BitSet {
        BitSet { inner: value, size }
    }

    // index starts from 1
    fn set(&mut self, index: usize, value: bool) -> Result<(), BitSetError> {
        if index > self.size || index == 0 {
            return Err(BitSetError::InvalidIndex);
        }
        match value {
            true => self.inner |= 1 << (index - 1),
            false => self.inner &= !(1 << (index - 1)),
        }
        Ok(())
    }

    fn set_next(&mut self) -> Result<(), BitSetError> {
        let index = self.get_min_unset_index()?;
        self.set(index, true)
    }

    fn get_min_unset_index(&self) -> Result<usize, BitSetError> {
        let mut b = self.inner;
        for i in 0..self.size {
            if b % 2 == 0 {
                return Ok(i + 1);
            }
            b >>= 1
        }
        Err(BitSetError::Full)
    }

    fn clear_all(&mut self) {
        self.inner = 0;
    }
}

impl From<&BitSet> for u128 {
    fn from(value: &BitSet) -> Self {
        value.inner
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum BitSetError {
    #[error("Invalid index")]
    InvalidIndex,
    #[error("BitSet is full")]
    Full,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(
        base,
        index,
        value,
        expected,
        case(BitSet::with_value(0, 32), 1, true, 1),
        case(BitSet::with_value(1, 32), 1, true, 1),
        case(BitSet::with_value(0, 32), 1, false, 0),
        case(BitSet::with_value(1, 32), 1, false, 0),
        case(BitSet::with_value(0, 32), 32, true, 1 << 31),
        case(BitSet::with_value(0b1111_1011, 8), 3, true, 0b1111_1111),
        case(BitSet::with_value(0b1111_1011, 8), 4, true, 0b1111_1011),
        case(BitSet::with_value(0b1111_1011, 8), 4, false, 0b1111_0011),
        case(BitSet::with_value(0b1111_1011, 8), 8, false, 0b0111_1011)
    )]
    fn work_bitset_set(mut base: BitSet, index: usize, value: bool, expected: u128) {
        base.set(index, value).unwrap();
        let a: u128 = (&base).into();
        assert_eq!(a, expected);
    }

    #[rstest(
        base,
        index,
        value,
        expected,
        case(BitSet::with_value(0, 32), 33, true, BitSetError::InvalidIndex),
        case(
            BitSet::with_value(0b1111_1011, 8),
            9,
            false,
            BitSetError::InvalidIndex
        ),
        case(
            BitSet::with_value(0b1111_1011, 8),
            0,
            false,
            BitSetError::InvalidIndex
        )
    )]
    fn fail_bitset_set(mut base: BitSet, index: usize, value: bool, expected: BitSetError) {
        let res = base.set(index, value);
        match res {
            Ok(_) => panic!("this test should be failed"),
            Err(e) => assert_eq!(e, expected),
        }
    }

    #[rstest(
        base,
        expected,
        case(BitSet::with_value(0b0111_1011, 8), 3),
        case(BitSet::with_value(0b0111_0110, 8), 1),
        case(BitSet::with_value(0b0111_0111, 8), 4),
        case(BitSet::with_value(0b0111_1111, 8), 8)
    )]
    fn work_bitset_get_min_unset_index(base: BitSet, expected: usize) {
        let index = base.get_min_unset_index().unwrap();
        assert_eq!(index, expected);
    }

    #[rstest(
        base,
        expected,
        case(BitSet::with_value(0b1111_1111, 8), BitSetError::Full),
        case(BitSet::with_value(0b1, 1), BitSetError::Full)
    )]
    fn fail_bitset_get_min_unset_index(base: BitSet, expected: BitSetError) {
        let res = base.get_min_unset_index();
        match res {
            Ok(_) => panic!("this test should be failed"),
            Err(e) => assert_eq!(e, expected),
        }
    }

    #[rstest(
        base,
        expected,
        case(BitSet::with_value(0b0111_0110, 8), 0b0111_0111),
        case(BitSet::with_value(0b0111_0111, 8), 0b0111_1111)
    )]
    fn work_bitset_set_next(mut base: BitSet, expected: u128) {
        base.set_next().unwrap();
        let a: u128 = (&base).into();
        assert_eq!(a, expected);
    }

    #[rstest(
        base,
        expected,
        case(BitSet::with_value(0b1111_1111, 8), BitSetError::Full)
    )]
    fn fail_bitset_set_next(mut base: BitSet, expected: BitSetError) {
        let res = base.set_next();
        match res {
            Ok(_) => panic!("this test should be failed"),
            Err(e) => assert_eq!(e, expected),
        }
    }
}
