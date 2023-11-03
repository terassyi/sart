use thiserror::Error;


#[derive(Debug)]
pub(super) struct BitSet {
    inner: Vec<u128>,
    size: u128,
}

impl BitSet {
    pub(super) fn new(size: u128) -> BitSet {
        let l = (size / 128) + if size % 128 > 0 { 1 } else { 0 };
        println!("{size}");
        println!("{l}");
        BitSet {
            inner: vec![0; l as usize],
            size,
        }
    }

    fn with_value(value: Vec<u128>, size: u128) -> BitSet {
        BitSet { inner: value, size }
    }

    pub(super) fn size(&self) -> u128 {
        self.size
    }

    // index starts from 0
    pub(super) fn is_set(&self, index: u128) -> bool {
        if index > self.size {
            return false
        }
        let (v, i) = BitSet::get_inner_index(index);
        let a: u128 = 1 << i;
        self.inner[v] & a != 0
    }

    // index starts from 0
    pub(super) fn set(&mut self, index: u128, value: bool) -> Result<u128, BitSetError> {
        if index > self.size {
            return Err(BitSetError::InvalidIndex)
        }
        let (v, n) = BitSet::get_inner_index(index);

        match value {
            true => self.inner[v] |= 1 << n,
            false => self.inner[v] &= !(1 << n),
        }
        Ok(index)
    }

    pub(super) fn set_true(&mut self, index: u128) -> Result<u128, BitSetError> {
        if self.is_set(index) {
            return Err(BitSetError::AlreadySet)
        }
        self.set(index, true)
    }

    pub(super) fn set_next(&mut self) -> Result<u128, BitSetError> {
        let index = self.get_min_unset_index()?;
        self.set(index, true)
    }

    fn get_min_unset_index(&self) -> Result<u128, BitSetError> {
        let (max_vec_idx, max_idx) = BitSet::get_inner_index(self.size);
        for (n, v) in self.inner.iter().enumerate() {
            let mut b = *v;
            let idx = if max_vec_idx == n {
                max_idx as u128
            } else {
                128
            };
            for i in 0..idx {
                if b % 2 == 0 {
                    return Ok((n as u128) * 128 + i);
                }
                b >>= 1
            }
        }
        Err(BitSetError::Full)
    }

    pub(super) fn clear_all(&mut self) {
        let l = self.inner.len();
        self.inner = vec![0; l];
    }

    fn get_inner_index(index: u128) -> (usize, usize) {
        ((index as usize / 128), (index as usize % 128))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum BitSetError {
    #[error("Invalid index")]
    InvalidIndex,
    #[error("BitSet is full")]
    Full,

    #[error("Already set")]
    AlreadySet,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(base, index, expected, 
        case(BitSet::with_value(vec![0], 64), 0, false),
        case(BitSet::with_value(vec![1], 64), 0, true),
        case(BitSet::with_value(vec![1 << 32], 64), 32, true),
        case(BitSet::with_value(vec![1 << 63], 64), 63, true),
        case(BitSet::with_value(vec![(1 << 63) + (1 << 64)], 65), 64, true),
        case(BitSet::with_value(vec![1 << 63, 1 << 33, 0, 0, 0, 1 << 22], 128*6), 128*5 + 22, true),
        case(BitSet::with_value(vec![0, 0], 256), 64, false),
        case(BitSet::with_value(vec![0, 1], 256), 128, true),
        case(BitSet::with_value(vec![0, 1<<3], 256), 131, true),
        case(BitSet::with_value(vec![0, 1<<3, 1, 1 << 127], 128 * 4), 128 * 4 - 1, true),
    )]
    fn works_bitset_is_set(base: BitSet, index: u128, expected: bool) {
        assert_eq!(base.is_set(index), expected)
    }

    #[rstest(
        base,
        index,
        value,
        case(BitSet::with_value(vec![0], 32), 0, true),
        case(BitSet::with_value(vec![0], 32), 16, true),
        case(BitSet::with_value(vec![0], 64), 16, true),
        case(BitSet::with_value(vec![0, 0, 0, 0], 128 * 4), 255, true),
        case(BitSet::with_value(vec![0, 0, 0, 1 << 63], 128 * 4), 128 * 3 + 63, true),
        case(BitSet::with_value(vec![0, 0, 1 << 33, 1 << 63], 128 * 4), 67, true),
        case(BitSet::with_value(vec![0, 0, 0, 1 << 63], 128 * 4), 511, false),
        case(BitSet::with_value(vec![0, 0, 0, 0], 128 * 4), 511, false),
        case(BitSet::with_value(vec![0, 0, 1 << 33, 1 << 63], 128 * 4), 289, false),
    )]
    fn work_bitset_set(mut base: BitSet, index: u128, value: bool) {
        base.set(index, value).unwrap();
        assert_eq!(value, base.is_set(index));
    }

    #[rstest(
        base,
        index,
        expected,
        case(BitSet::with_value(vec![0], 32), 33, BitSetError::InvalidIndex),
    )]
    fn fail_bitset_set(mut base: BitSet, index: u128, expected: BitSetError) {
        let res = base.set(index, true);
        match res {
            Ok(_) => panic!("This case should be returned error"),
            Err(e) => assert_eq!(e, expected),
        }
    }

    #[rstest(
        base,
        expected,
        case(BitSet::with_value(vec![0], 8), 0),
        case(BitSet::with_value(vec![0, 0, 0, 0], 128 * 4), 0),
        case(BitSet::with_value(vec![0x0000_00ff, 0, 0, 0], 128 * 4), 8),
        case(BitSet::with_value(vec![0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 0xffff_ffff_ffff_0fff, 0, 0], 128 * 4), 140),
        case(BitSet::with_value(vec![0xffff_ffff_ffff_dfff, 0xffff_ffff_ffff_0fff, 0, 0], 256), 13),
    )]
    fn work_bitset_get_min_unset_index(base: BitSet, expected: u128) {
        let index = base.get_min_unset_index().unwrap();
        assert_eq!(index, expected);
    }

    #[rstest(
        base,
        expected,
        case(BitSet::with_value(vec![0x0000_00ff], 8), BitSetError::Full),
        case(BitSet::with_value(vec![1], 1), BitSetError::Full)
    )]
    fn fail_bitset_get_min_unset_index(base: BitSet, expected: BitSetError) {
        let res = base.get_min_unset_index();
        match res {
            Ok(i) => panic!("this test should be failed {i}"),
            Err(e) => assert_eq!(e, expected),
        }
    }

    #[rstest(
        base,
        expected_index,
        case(BitSet::with_value(vec![0b0111_0110], 8), 0),
        case(BitSet::with_value(vec![0b0111_0111], 8), 3),
        case(BitSet::with_value(vec![0x0000_00ff, 0, 0, 0], 128 * 4), 8),
        case(BitSet::with_value(vec![0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 0xffff_ffff_ffff_0fff, 0, 0], 128 * 4), 140),
        case(BitSet::with_value(vec![0xffff_ffff_ffff_dfff, 0xffff_ffff_ffff_0fff, 0, 0], 128 * 4), 13),
    )]
    fn work_bitset_set_next(mut base: BitSet, expected_index: u128) {
        assert!(!base.is_set(expected_index));
        base.set_next().unwrap();
        assert!(base.is_set(expected_index))
    }
}
