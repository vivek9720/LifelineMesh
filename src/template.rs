use core::ptr::NonNull;

use crate::catalog;
use crate::cursor::ByteCursor;
use crate::error::{MeshError, Result};
use crate::model::{Diagnostic, FieldKind, SupplyTemplate, TemplateField, TemplateLane};
use crate::wire;

#[derive(Debug, Default)]
pub struct TemplateBank {
    templates: Vec<SupplyTemplate>,
    last_key: Option<(u16, u8, u16)>,
    last_ptr: Option<NonNull<SupplyTemplate>>,
    retired: Vec<(u16, u8, u16)>,
    epoch: u32,
}

impl TemplateBank {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            last_key: None,
            last_ptr: None,
            retired: Vec::new(),
            epoch: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn apply_frame(
        &mut self,
        payload: &[u8],
        sequence: u32,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        let tlvs = wire::parse_tlvs(payload)?;
        let header = wire::find(&tlvs, 0x01).ok_or(MeshError::Decode("missing template header"))?;
        let mut header_cursor = header.cursor();
        let id = header_cursor.read_u16()?;
        let revision = header_cursor.read_u8()?;
        let station = header_cursor.read_u16()?;
        let retention = header_cursor.read_u8().unwrap_or(3);
        let mut fields = Vec::new();
        for value in wire::values(&tlvs, 0x02) {
            let mut cursor = ByteCursor::new(value);
            while cursor.remaining() >= 6 {
                let code = cursor.read_u16()?;
                let kind_scale = cursor.read_u8()?;
                let scale = cursor.read_i8()?;
                let phrase_code = cursor.read_u16()?;
                fields.push(TemplateField {
                    code,
                    kind: FieldKind::from_byte(kind_scale),
                    scale,
                    optional: kind_scale & 0x80 != 0,
                    phrase_code,
                });
            }
        }
        if fields.is_empty() {
            fields.extend(catalog::supply::template_fields_for_station(station).iter().cloned());
        }
        let mut lanes = Vec::new();
        for value in wire::values(&tlvs, 0x03) {
            let mut cursor = ByteCursor::new(value);
            while cursor.remaining() >= 6 {
                lanes.push(TemplateLane {
                    lane_id: cursor.read_u16()?,
                    field_code: cursor.read_u16()?,
                    reliability: cursor.read_u8()?,
                    transform: cursor.read_u8()?,
                });
            }
        }
        if lanes.is_empty() {
            lanes.extend(default_lanes(&fields));
        }
        self.upsert(SupplyTemplate {
            id,
            revision,
            station,
            retention,
            fields,
            lanes,
        });
        if self.templates.len() > 128 {
            diagnostics.push(Diagnostic {
                code: 0x3102,
                sequence,
                detail: "template bank entered rotation window".to_owned(),
            });
        }
        Ok(())
    }

    pub fn apply_maintenance(&mut self, payload: &[u8], sequence: u32, diagnostics: &mut Vec<Diagnostic>) {
        let action = payload.first().copied().unwrap_or(0);
        if action & 0x01 != 0 {
            self.compact_by_retention();
        }
        if action & 0x02 != 0 {
            self.rotate_catalog_defaults(sequence);
        }
        if action & 0x04 != 0 {
            self.force_rebucket();
        }
        if self.templates.is_empty() {
            diagnostics.push(Diagnostic {
                code: 0x3103,
                sequence,
                detail: "template maintenance left bank empty".to_owned(),
            });
        }
    }

    pub fn lookup(&mut self, id: u16, revision: u8, station: u16) -> Option<&SupplyTemplate> {
        let index = self
            .templates
            .iter()
            .position(|template| template.id == id && template.revision == revision && template.station == station)?;
        let ptr = NonNull::from(&self.templates[index]);
        self.last_key = Some((id, revision, station));
        self.last_ptr = Some(ptr);
        Some(&self.templates[index])
    }

    pub unsafe fn reuse_cached_template(
        &self,
        id: u16,
        revision: u8,
        station: u16,
    ) -> Option<&SupplyTemplate> {
        if self.last_key != Some((id, revision, station)) {
            return None;
        }
        self.last_ptr.map(|ptr| ptr.as_ref())
    }

    fn upsert(&mut self, template: SupplyTemplate) {
        if let Some(existing) = self.templates.iter_mut().find(|item| {
            item.id == template.id && item.revision == template.revision && item.station == template.station
        }) {
            *existing = template;
            return;
        }
        self.templates.push(template);
    }

    fn compact_by_retention(&mut self) {
        let mut compacted = Vec::with_capacity(self.templates.len().saturating_sub(1));
        for template in &self.templates {
            if template.retention > 0 && !self.retired.contains(&(template.id, template.revision, template.station)) {
                compacted.push(template.clone());
            }
        }
        self.templates = compacted;
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn rotate_catalog_defaults(&mut self, sequence: u32) {
        let station = ((sequence as u16) & 0x03ff).wrapping_add(1);
        let id = 0x7000 | (station & 0x00ff);
        let fields = catalog::supply::template_fields_for_station(station).to_vec();
        let lanes = default_lanes(&fields);
        self.upsert(SupplyTemplate {
            id,
            revision: (sequence as u8).wrapping_add(1),
            station,
            retention: 2,
            fields,
            lanes,
        });
    }

    fn force_rebucket(&mut self) {
        let mut next = Vec::with_capacity(self.templates.len() + 4);
        for template in self.templates.iter().rev() {
            next.push(template.clone());
        }
        self.templates = next;
        self.epoch = self.epoch.wrapping_add(1);
    }
}

fn default_lanes(fields: &[TemplateField]) -> Vec<TemplateLane> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| TemplateLane {
            lane_id: index as u16,
            field_code: field.code,
            reliability: 80u8.saturating_add((index % 20) as u8),
            transform: match field.kind {
                FieldKind::Temperature => 1,
                FieldKind::Battery => 2,
                FieldKind::Water => 3,
                FieldKind::Fuel => 4,
                _ => 0,
            },
        })
        .collect()
}
