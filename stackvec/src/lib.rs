use std::{mem::MaybeUninit, ops::Index};

pub struct StackVecIter<T: Copy, const CAPACITY: usize> {
    vec: StackVec<T, CAPACITY>,
    index: usize,
}

impl<T: Copy, const CAPACITY: usize> Iterator for StackVecIter<T, CAPACITY> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.vec.len() {
            None
        } else {
            let value = self.vec[self.index];
            self.index += 1;
            Some(value)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StackVec<T: Copy, const CAPACITY: usize> {
    buffer: [MaybeUninit<T>; CAPACITY],
    len: usize,
}

impl<T: Copy, const CAPACITY: usize> Default for StackVec<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy, const CAPACITY: usize> StackVec<T, CAPACITY> {
    pub fn new() -> Self {
        Self {
            buffer: [MaybeUninit::uninit(); CAPACITY],
            len: 0,
        }
    }

    pub fn push(&mut self, val: T) -> Result<usize, ()> {
        if self.len() > self.capacity() {
            return Err(());
        }
        self.buffer[self.len()].write(val);
        self.len += 1;
        Ok(self.len())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }
}

impl<T: Copy + PartialEq, const CAPACITY: usize> StackVec<T, CAPACITY> {
    pub fn contains(&self, v: T) -> bool {
        self.into_iter().any(|e| e == v)
    }
}

impl<T: Copy, const CAPACITY: usize> Index<usize> for StackVec<T, CAPACITY> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index > self.len() {
            panic!("Index out of bounds!");
        }
        unsafe { self.buffer[index].assume_init_ref() }
    }
}

impl<T: Copy, const CAPACITY: usize> IntoIterator for StackVec<T, CAPACITY> {
    type Item = T;

    type IntoIter = StackVecIter<T, CAPACITY>;

    fn into_iter(self) -> Self::IntoIter {
        StackVecIter {
            vec: self,
            index: 0,
        }
    }
}

impl<A: Copy, const CAPACITY: usize> FromIterator<A> for StackVec<A, CAPACITY> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let mut result = Self::new();
        for v in iter {
            result.push(v).unwrap();
        }
        result
    }
}

pub trait ToStackVec<Output: Copy, const CAPACITY: usize> {
    fn to_stack_vec(self) -> StackVec<Output, CAPACITY>;
}

impl<T: Copy, const N: usize> ToStackVec<T, N> for [T; N] {
    fn to_stack_vec(self) -> StackVec<T, N> {
        self.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::ToStackVec;

    #[test]
    fn init() {
        let s = [1, 2, 3].to_stack_vec();
        for v in s {
            println!("{v}");
        }
    }
}
