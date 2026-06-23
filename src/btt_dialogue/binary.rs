use std::collections::HashMap;
use std::error::Error;

#[derive(Default)]
pub(super) struct StringPool {
    ids: HashMap<String, u32>,
    values: Vec<String>,
}

impl StringPool {
    pub(super) fn add(&mut self, value: &str) -> Result<u32, Box<dyn Error>> {
        if let Some(id) = self.ids.get(value) {
            return Ok(*id);
        }

        let id = checked_u32(self.values.len(), "string pool entry count")?;
        let owned = value.to_string();
        self.values.push(owned.clone());
        self.ids.insert(owned, id);
        Ok(id)
    }

    pub(super) fn len(&self) -> usize {
        self.values.len()
    }

    pub(super) fn to_buffers(&self) -> (Vec<u8>, Vec<u8>) {
        let mut offsets = Vec::with_capacity((self.values.len() + 1) * 8);
        let mut bytes = Vec::new();
        for value in &self.values {
            write_u64(&mut offsets, bytes.len() as u64);
            bytes.extend_from_slice(value.as_bytes());
        }
        write_u64(&mut offsets, bytes.len() as u64);
        (offsets, bytes)
    }
}

pub(super) fn align8(value: usize) -> usize {
    (value + 7) & !7
}

pub(super) fn pad_to(output: &mut Vec<u8>, target_len: usize) {
    if output.len() < target_len {
        output.resize(target_len, 0);
    }
}

pub(super) fn checked_u16(value: usize, name: &str) -> Result<u16, Box<dyn Error>> {
    u16::try_from(value).map_err(|_| format!("{name} exceeds u16 range: {value}").into())
}

pub(super) fn checked_u32(value: usize, name: &str) -> Result<u32, Box<dyn Error>> {
    u32::try_from(value).map_err(|_| format!("{name} exceeds u32 range: {value}").into())
}

pub(super) fn checked_u64(value: usize, name: &str) -> Result<u64, Box<dyn Error>> {
    u64::try_from(value).map_err(|_| format!("{name} exceeds u64 range: {value}").into())
}

pub(super) fn write_u8(output: &mut Vec<u8>, value: u8) {
    output.push(value);
}

pub(super) fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn write_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
