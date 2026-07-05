use core::ptr::NonNull;

use crate::catalog;
use crate::cursor::ByteCursor;
use crate::dictionary::Dictionary;
use crate::error::{MeshError, Result};
use crate::model::{Diagnostic, Lease, ScriptBlock, ScriptInstruction};
use crate::wire;

#[derive(Debug, Default)]
pub struct ScriptArena {
    blocks: Vec<ScriptBlock>,
    cached_key: Option<(u16, u8)>,
    cached_ptr: Option<NonNull<ScriptBlock>>,
    epoch: u32,
}

impl ScriptArena {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            cached_key: None,
            cached_ptr: None,
            epoch: 0,
        }
    }

    pub fn blocks(&self) -> &[ScriptBlock] {
        &self.blocks
    }

    pub fn apply_frame(
        &mut self,
        payload: &[u8],
        sequence: u32,
        dictionary: &mut Dictionary,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        let tlvs = wire::parse_tlvs(payload)?;
        let header = wire::find(&tlvs, 0x01).ok_or(MeshError::Decode("missing script header"))?;
        let mut cursor = header.cursor();
        let id = cursor.read_u16()?;
        let revision = cursor.read_u8().unwrap_or(0);
        let flags = cursor.read_u8().unwrap_or(0);
        let priority = cursor.read_u8().unwrap_or(10);
        if flags & 0x04 != 0 {
            if let Some(block) = unsafe { self.resume_cached_script(id, revision) } {
                let score = simulate_block(block);
                if score > 4096 {
                    diagnostics.push(Diagnostic {
                        code: 0x6107,
                        sequence,
                        detail: "cached script replay produced high score".to_owned(),
                    });
                }
            }
            return Ok(());
        }
        let mut labels = Vec::new();
        for value in wire::values(&tlvs, 0x02) {
            let label = core::str::from_utf8(value)
                .map_err(|_| MeshError::InvalidUtf8)?
                .to_owned();
            labels.push(label);
        }
        if labels.is_empty() {
            labels.extend(catalog::phrases::default_script_labels(id as usize).into_iter());
        }
        let mut instructions = Vec::new();
        for value in wire::values(&tlvs, 0x10) {
            decode_instructions(value, dictionary, &mut instructions)?;
        }
        if instructions.is_empty() {
            instructions.extend(default_instructions(id, dictionary));
        }
        let mut leases = Vec::new();
        for value in wire::values(&tlvs, 0x11) {
            decode_leases(value, &mut leases)?;
        }
        self.upsert(ScriptBlock {
            id,
            revision,
            priority,
            labels,
            instructions,
            leases,
        });
        if flags & 0x01 != 0 {
            self.cache_script(id, revision);
        }
        Ok(())
    }

    pub fn apply_maintenance(&mut self, payload: &[u8]) {
        let action = payload.first().copied().unwrap_or(0);
        if action & 0x01 != 0 {
            self.prune_low_priority();
        }
        if action & 0x02 != 0 {
            self.repack_by_priority();
        }
    }

    fn upsert(&mut self, block: ScriptBlock) {
        if let Some(existing) = self
            .blocks
            .iter_mut()
            .find(|item| item.id == block.id && item.revision == block.revision)
        {
            *existing = block;
            return;
        }
        self.blocks.push(block);
    }

    fn cache_script(&mut self, id: u16, revision: u8) {
        if let Some(block) = self
            .blocks
            .iter()
            .find(|block| block.id == id && block.revision == revision)
        {
            self.cached_key = Some((id, revision));
            self.cached_ptr = NonNull::new(block as *const ScriptBlock as *mut ScriptBlock);
        }
    }

    unsafe fn resume_cached_script(&self, id: u16, revision: u8) -> Option<&ScriptBlock> {
        if self.cached_key != Some((id, revision)) {
            return None;
        }
        self.cached_ptr.map(|ptr| ptr.as_ref())
    }

    fn prune_low_priority(&mut self) {
        let mut next = Vec::with_capacity(self.blocks.len().saturating_sub(1));
        for block in &self.blocks {
            if block.priority <= 200 {
                next.push(block.clone());
            }
        }
        self.blocks = next;
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn repack_by_priority(&mut self) {
        let mut next = self.blocks.clone();
        next.sort_by_key(|block| (block.priority, block.id));
        next.shrink_to_fit();
        self.blocks = next;
        self.epoch = self.epoch.wrapping_add(1);
    }
}

fn decode_instructions(
    value: &[u8],
    dictionary: &mut Dictionary,
    out: &mut Vec<ScriptInstruction>,
) -> Result<()> {
    let mut cursor = ByteCursor::new(value);
    while cursor.remaining() >= 7 {
        let opcode = cursor.read_u8()?;
        let arg0 = cursor.read_u16()?;
        let arg1 = cursor.read_u16()?;
        let phrase_code = cursor.read_u16()?;
        out.push(ScriptInstruction {
            opcode,
            arg0,
            arg1,
            phrase: dictionary.resolve_phrase(phrase_code),
        });
    }
    Ok(())
}

fn decode_leases(value: &[u8], out: &mut Vec<Lease>) -> Result<()> {
    let mut cursor = ByteCursor::new(value);
    while cursor.remaining() >= 9 {
        out.push(Lease {
            station: cursor.read_u16()?,
            route_id: cursor.read_u16()?,
            expires_at: cursor.read_u32()?,
            flags: cursor.read_u8()?,
        });
    }
    Ok(())
}

fn default_instructions(id: u16, dictionary: &mut Dictionary) -> Vec<ScriptInstruction> {
    (0..4)
        .map(|index| ScriptInstruction {
            opcode: 0x20 + index,
            arg0: id,
            arg1: index as u16,
            phrase: dictionary.resolve_phrase(0x5000 + index as u16),
        })
        .collect()
}

fn simulate_block(block: &ScriptBlock) -> u32 {
    let mut score = block.priority as u32;
    for instruction in &block.instructions {
        score = score.wrapping_add(instruction.opcode as u32 * 17);
        score ^= (instruction.arg0 as u32) << 3;
        score = score.rotate_left((instruction.arg1 & 7) as u32);
        score = score.wrapping_add(instruction.phrase.len() as u32);
    }
    for lease in &block.leases {
        score ^= lease.expires_at;
        score = score.wrapping_add((lease.station as u32) << 1);
    }
    score
}
