use crate::cursor::ByteCursor;
use crate::error::{MeshError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tlv {
    pub tag: u8,
    pub flags: u8,
    pub value: Vec<u8>,
}

impl Tlv {
    pub fn cursor(&self) -> ByteCursor<'_> {
        ByteCursor::new(&self.value)
    }

    pub fn is_critical(&self) -> bool {
        self.flags & 0x80 != 0
    }

    pub fn is_replayable(&self) -> bool {
        self.flags & 0x40 != 0
    }
}

pub fn parse_tlvs(payload: &[u8]) -> Result<Vec<Tlv>> {
    let mut cursor = ByteCursor::new(payload);
    let mut tlvs = Vec::new();
    while !cursor.is_empty() {
        let tag = cursor.read_u8()?;
        let flags = cursor.read_u8()?;
        let len = cursor.read_var_u32()? as usize;
        if len > cursor.remaining() {
            return Err(MeshError::InvalidLength);
        }
        let value = cursor.read_bytes(len)?.to_vec();
        tlvs.push(Tlv { tag, flags, value });
    }
    Ok(tlvs)
}

pub fn find<'a>(tlvs: &'a [Tlv], tag: u8) -> Option<&'a Tlv> {
    tlvs.iter().find(|tlv| tlv.tag == tag)
}

pub fn values<'a>(tlvs: &'a [Tlv], tag: u8) -> impl Iterator<Item = &'a [u8]> + 'a {
    tlvs.iter()
        .filter(move |tlv| tlv.tag == tag)
        .map(|tlv| tlv.value.as_slice())
}

pub fn read_u16_value(tlvs: &[Tlv], tag: u8, default: u16) -> Result<u16> {
    let Some(tlv) = find(tlvs, tag) else {
        return Ok(default);
    };
    let mut cursor = tlv.cursor();
    cursor.read_u16()
}

pub fn read_u32_value(tlvs: &[Tlv], tag: u8, default: u32) -> Result<u32> {
    let Some(tlv) = find(tlvs, tag) else {
        return Ok(default);
    };
    let mut cursor = tlv.cursor();
    cursor.read_u32()
}

pub fn read_string_value(tlvs: &[Tlv], tag: u8) -> Result<Option<String>> {
    let Some(tlv) = find(tlvs, tag) else {
        return Ok(None);
    };
    let mut cursor = tlv.cursor();
    if cursor.is_empty() {
        return Ok(Some(String::new()));
    }
    if cursor.peek_u8()? as usize == cursor.remaining().saturating_sub(1) {
        return cursor.read_string().map(Some);
    }
    core::str::from_utf8(&tlv.value)
        .map(|s| Some(s.to_owned()))
        .map_err(|_| MeshError::InvalidUtf8)
}
