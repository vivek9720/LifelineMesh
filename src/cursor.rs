use crate::error::{MeshError, Result};

#[derive(Clone, Copy)]
pub struct ByteCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        if self.remaining() < 1 {
            return Err(MeshError::Truncated);
        }
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_array::<2>()?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        let bytes = self.read_array::<2>()?;
        Ok(i16::from_le_bytes(bytes))
    }

    pub fn read_u24(&mut self) -> Result<u32> {
        let a = self.read_u8()? as u32;
        let b = self.read_u8()? as u32;
        let c = self.read_u8()? as u32;
        Ok(a | (b << 8) | (c << 16))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_array::<4>()?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.read_array::<4>()?;
        Ok(i32::from_le_bytes(bytes))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_array::<8>()?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub fn read_var_u32(&mut self) -> Result<u32> {
        let mut shift = 0;
        let mut value = 0u32;
        for _ in 0..5 {
            let byte = self.read_u8()?;
            value |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
        Err(MeshError::InvalidVarint)
    }

    pub fn read_svar_i32(&mut self) -> Result<i32> {
        let value = self.read_var_u32()?;
        Ok(((value >> 1) as i32) ^ (-((value & 1) as i32)))
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            return Err(MeshError::Truncated);
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.data[start..start + len])
    }

    pub fn read_len_prefixed(&mut self) -> Result<&'a [u8]> {
        let len = self.read_var_u32()? as usize;
        self.read_bytes(len)
    }

    pub fn read_string(&mut self) -> Result<String> {
        let bytes = self.read_len_prefixed()?;
        core::str::from_utf8(bytes)
            .map(|s| s.to_owned())
            .map_err(|_| MeshError::InvalidUtf8)
    }

    pub fn skip(&mut self, len: usize) -> Result<()> {
        self.read_bytes(len).map(|_| ())
    }

    pub fn peek_u8(&self) -> Result<u8> {
        if self.remaining() < 1 {
            return Err(MeshError::Truncated);
        }
        Ok(self.data[self.pos])
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.read_bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }
}
