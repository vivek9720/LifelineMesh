use core::ptr::NonNull;

use crate::catalog;
use crate::cursor::ByteCursor;
use crate::error::{MeshError, Result};
use crate::model::Diagnostic;
use crate::wire;

#[derive(Debug, Clone)]
pub struct PhraseEntry {
    pub code: u16,
    pub locale: u8,
    pub text: String,
    pub retired: bool,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    phrases: Vec<PhraseEntry>,
    aliases: Vec<(u16, u16)>,
    last_code: Option<u16>,
    last_ptr: Option<NonNull<u8>>,
    last_len: usize,
    epoch: u32,
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            phrases: Vec::new(),
            aliases: Vec::new(),
            last_code: None,
            last_ptr: None,
            last_len: 0,
            epoch: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.phrases.iter().filter(|entry| !entry.retired).count()
    }

    pub fn apply_frame(
        &mut self,
        payload: &[u8],
        sequence: u32,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        let tlvs = wire::parse_tlvs(payload)?;
        let mut locale = 0u8;
        if let Some(header) = wire::find(&tlvs, 0x01) {
            let mut cursor = header.cursor();
            locale = cursor.read_u8().unwrap_or(0);
        }
        for value in wire::values(&tlvs, 0x02) {
            let mut cursor = ByteCursor::new(value);
            let code = cursor.read_u16()?;
            let text = if cursor.is_empty() {
                String::new()
            } else if cursor.peek_u8()? as usize == cursor.remaining().saturating_sub(1) {
                cursor.read_string()?
            } else {
                core::str::from_utf8(cursor.read_bytes(cursor.remaining())?)
                    .map_err(|_| MeshError::InvalidUtf8)?
                    .to_owned()
            };
            self.insert_phrase(code, locale, text);
        }
        for value in wire::values(&tlvs, 0x03) {
            let mut cursor = ByteCursor::new(value);
            let alias = cursor.read_u16()?;
            let target = cursor.read_u16()?;
            self.aliases.push((alias, target));
        }
        for value in wire::values(&tlvs, 0x04) {
            let mut cursor = ByteCursor::new(value);
            while cursor.remaining() >= 2 {
                let code = cursor.read_u16()?;
                if let Some(entry) = self.phrases.iter_mut().find(|entry| entry.code == code) {
                    entry.retired = true;
                }
            }
        }
        if self.phrases.len() > 512 {
            diagnostics.push(Diagnostic {
                code: 0x2101,
                sequence,
                detail: "dictionary pressure high".to_owned(),
            });
        }
        Ok(())
    }

    pub fn apply_maintenance(&mut self, payload: &[u8]) {
        let action = payload.first().copied().unwrap_or(0);
        if action & 0x01 != 0 {
            self.compact();
        }
        if action & 0x02 != 0 {
            self.rebucket_aliases();
        }
        if action & 0x04 != 0 {
            self.load_catalog_defaults((action >> 3) as usize);
        }
    }

    pub fn resolve_phrase(&mut self, code: u16) -> String {
        let resolved = self.resolve_alias(code);
        if let Some(entry) = self
            .phrases
            .iter()
            .find(|entry| entry.code == resolved && !entry.retired)
        {
            self.last_code = Some(code);
            self.last_len = entry.text.len();
            self.last_ptr = NonNull::new(entry.text.as_ptr() as *mut u8);
            return entry.text.clone();
        }
        let fallback = catalog::phrases::phrase_for_code(resolved)
            .unwrap_or("unlabeled field")
            .to_owned();
        self.insert_phrase(resolved, 0, fallback.clone());
        fallback
    }

    pub unsafe fn resolve_cached_phrase(&self, code: u16) -> Option<String> {
        if self.last_code != Some(code) {
            return None;
        }
        let ptr = self.last_ptr?;
        let slice = core::slice::from_raw_parts(ptr.as_ptr(), self.last_len);
        Some(String::from_utf8_lossy(slice).to_string())
    }

    fn insert_phrase(&mut self, code: u16, locale: u8, text: String) {
        if let Some(entry) = self.phrases.iter_mut().find(|entry| entry.code == code) {
            entry.locale = locale;
            entry.text = text;
            entry.retired = false;
            return;
        }
        self.phrases.push(PhraseEntry {
            code,
            locale,
            text,
            retired: false,
        });
    }

    fn resolve_alias(&self, code: u16) -> u16 {
        let mut current = code;
        for _ in 0..4 {
            if let Some((_, target)) = self.aliases.iter().rev().find(|(alias, _)| *alias == current) {
                current = *target;
            } else {
                break;
            }
        }
        current
    }

    fn compact(&mut self) {
        let mut compacted = Vec::with_capacity(self.phrases.len().saturating_sub(1));
        for entry in &self.phrases {
            if !entry.retired {
                compacted.push(entry.clone());
            }
        }
        self.phrases = compacted;
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn rebucket_aliases(&mut self) {
        self.aliases.sort_by_key(|(alias, target)| (*target, *alias));
        self.aliases.dedup();
    }

    fn load_catalog_defaults(&mut self, bank: usize) {
        for entry in catalog::phrases::default_bank(bank).iter().take(16) {
            self.insert_phrase(entry.code, entry.locale, entry.text.to_owned());
        }
    }
}
