use crate::error::{MeshError, Result};
use crate::frame;
use crate::model::{Frame, FrameKind};

#[derive(Debug, Clone)]
struct FragmentSlot {
    stream_id: u16,
    sequence_base: u32,
    total: u8,
    seen: Vec<Option<Vec<u8>>>,
    kind: FrameKind,
}

#[derive(Debug, Default)]
pub struct FragmentReassembler {
    slots: Vec<FragmentSlot>,
}

impl FragmentReassembler {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    pub fn apply(&mut self, frame: &Frame) -> Result<Option<Frame>> {
        if frame.payload.len() < 7 {
            return Err(MeshError::Truncated);
        }
        let stream_id = u16::from_le_bytes([frame.payload[0], frame.payload[1]]);
        let sequence_base = u32::from_le_bytes([
            frame.payload[2],
            frame.payload[3],
            frame.payload[4],
            frame.payload[5],
        ]);
        let index = frame.payload[6];
        let total = frame.payload.get(7).copied().unwrap_or(index.saturating_add(1)).max(1);
        let kind = frame
            .payload
            .get(8)
            .map(|byte| FrameKind::from_byte(*byte))
            .unwrap_or(FrameKind::Manifest);
        let body = if frame.payload.len() > 9 {
            frame.payload[9..].to_vec()
        } else {
            Vec::new()
        };
        let slot_index = self.slot_index(stream_id, sequence_base, total, kind);
        if index as usize >= self.slots[slot_index].seen.len() {
            return Err(MeshError::InvalidLength);
        }
        self.slots[slot_index].seen[index as usize] = Some(body);
        if self.slots[slot_index].seen.iter().all(|part| part.is_some()) {
            let slot = self.slots.remove(slot_index);
            let mut payload = Vec::new();
            for part in slot.seen.into_iter().flatten() {
                payload.extend_from_slice(&part);
            }
            return Ok(Some(Frame {
                version: 1,
                flags: 0,
                stream_id: slot.stream_id,
                sequence: slot.sequence_base,
                kind: slot.kind,
                header_len: frame::MIN_HEADER_LEN,
                payload,
            }));
        }
        Ok(None)
    }

    fn slot_index(&mut self, stream_id: u16, sequence_base: u32, total: u8, kind: FrameKind) -> usize {
        if let Some(index) = self
            .slots
            .iter()
            .position(|slot| slot.stream_id == stream_id && slot.sequence_base == sequence_base)
        {
            return index;
        }
        self.slots.push(FragmentSlot {
            stream_id,
            sequence_base,
            total,
            seen: vec![None; total as usize],
            kind,
        });
        self.slots.len() - 1
    }
}
