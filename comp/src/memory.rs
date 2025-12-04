use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct Bound;

pub trait Stage: std::fmt::Debug + Clone {}

impl Stage for Bound {}

#[derive(Debug)]
pub struct Metadata {
    start: usize,
    len: usize,
}

#[derive(Debug)]
pub struct Memoryblock<S: Stage> {
    buffer: Vec<u8>,
    metadata: Vec<Metadata>,
    _marker: PhantomData<S>,
}
impl<S: Stage> Memoryblock<S> {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            metadata: Vec::new(),
            _marker: Default::default(),
        }
    }

    pub fn allocate(&mut self, size: usize) -> usize {
        let ptr = self.buffer.len();
        for _ in 0..size {
            self.buffer.push(0);
        }
        self.metadata.push(Metadata {
            start: ptr,
            len: size,
        });
        ptr
    }

    pub(crate) fn write_value(&mut self, ptr: usize, value: crate::value::Value) {
        match value {
            crate::value::Value::Error => {}
            crate::value::Value::UnsignedInteger8(it) => self.buffer[ptr] = it,
            crate::value::Value::UnsignedInteger16(it) => {
                for (offset, byte) in it.to_le_bytes().into_iter().enumerate() {
                    self.buffer[ptr + offset] = byte;
                }
            }
            crate::value::Value::UnsignedInteger32(it) => {
                for (offset, byte) in it.to_le_bytes().into_iter().enumerate() {
                    self.buffer[ptr + offset] = byte;
                }
            }
            crate::value::Value::Bool(value) => self.buffer[ptr] = if value { 1 } else { 0 },
            crate::value::Value::Pointer(it) => {
                for (offset, byte) in (it as u32).to_le_bytes().into_iter().enumerate() {
                    self.buffer[ptr + offset] = byte;
                }
            }
            crate::value::Value::CompileTimeFunction(_) => {}
            crate::value::Value::DependentOn(_) => {}
            crate::value::Value::Type(_) => {}
        }
    }

    pub(crate) fn read<const N: usize>(&self, base: usize, buffer: &mut [u8; N]) -> usize {
        for offset in 0..N {
            if base + offset >= self.buffer.len() {
                return offset;
            }
            buffer[offset] = self.buffer[base + offset];
        }
        return N;
    }
}
