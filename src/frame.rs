use crate::checksum;
use crate::cursor::ByteCursor;
use crate::error::{MeshError, Result};
use crate::model::{Frame, FrameKind};

pub const MAGIC: &[u8; 4] = b"LMSH";
pub const MIN_HEADER_LEN: u8 = 18;

pub fn parse_frames(data: &[u8]) -> Result<Vec<Frame>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let mut cursor = ByteCursor::new(data);
    let mut frames = Vec::new();
    while cursor.remaining() >= MIN_HEADER_LEN as usize {
        let start = cursor.position();
        let magic = cursor.read_bytes(4)?;
        if magic != MAGIC {
            return parse_legacy_stream(data);
        }
        let version = cursor.read_u8()?;
        if version == 0 || version > 3 {
            return Err(MeshError::UnsupportedVersion(version));
        }
        let flags = cursor.read_u8()?;
        let kind = FrameKind::from_byte(cursor.read_u8()?);
        let header_len = cursor.read_u8()?;
        if header_len < MIN_HEADER_LEN {
            return Err(MeshError::InvalidLength);
        }
        let stream_id = cursor.read_u16()?;
        let sequence = cursor.read_u32()?;
        let payload_len = cursor.read_u16()? as usize;
        let expected_crc = cursor.read_u16()?;
        let extra = header_len as usize - MIN_HEADER_LEN as usize;
        if extra > cursor.remaining() {
            return Err(MeshError::Truncated);
        }
        cursor.skip(extra)?;
        if payload_len > cursor.remaining() {
            return Err(MeshError::Truncated);
        }
        let payload = cursor.read_bytes(payload_len)?.to_vec();
        if flags & 0x40 != 0 {
            let actual = checksum::crc16_mesh(&data[start + header_len as usize..cursor.position()]);
            if actual != expected_crc {
                return Err(MeshError::ChecksumMismatch);
            }
        }
        frames.push(Frame {
            version,
            flags,
            stream_id,
            sequence,
            kind,
            header_len,
            payload,
        });
    }
    if cursor.remaining() != 0 {
        return Err(MeshError::Truncated);
    }
    Ok(frames)
}

fn parse_legacy_stream(data: &[u8]) -> Result<Vec<Frame>> {
    let mut cursor = ByteCursor::new(data);
    let mut frames = Vec::new();
    let mut sequence = 0u32;
    while cursor.remaining() >= 4 {
        let kind = FrameKind::from_byte(cursor.read_u8()? & 0x0f);
        let flags = cursor.read_u8()?;
        let len = cursor.read_u16()? as usize;
        if len > cursor.remaining() {
            return Err(MeshError::InvalidLength);
        }
        let payload = cursor.read_bytes(len)?.to_vec();
        frames.push(Frame {
            version: 1,
            flags,
            stream_id: 0,
            sequence,
            kind,
            header_len: 4,
            payload,
        });
        sequence = sequence.wrapping_add(1);
    }
    Ok(frames)
}

pub fn encode_frame(kind: FrameKind, sequence: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MIN_HEADER_LEN as usize + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(1);
    out.push(0);
    out.push(kind.as_byte());
    out.push(MIN_HEADER_LEN);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(payload);
    out
}
