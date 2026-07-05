use crate::cursor::ByteCursor;
use crate::dictionary::Dictionary;
use crate::error::{MeshError, Result};
use crate::model::{Diagnostic, FieldKind, Manifest, ManifestItem, SupplyTemplate};
use crate::template::TemplateBank;
use crate::wire;

pub fn decode_manifest(
    payload: &[u8],
    sequence: u32,
    bank: &mut TemplateBank,
    dictionary: &mut Dictionary,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Manifest> {
    let tlvs = wire::parse_tlvs(payload)?;
    let header = wire::find(&tlvs, 0x01).ok_or(MeshError::Decode("missing manifest header"))?;
    let mut cursor = header.cursor();
    let station = cursor.read_u16()?;
    let template_id = cursor.read_u16()?;
    let revision = cursor.read_u8()?;
    let flags = cursor.read_u8().unwrap_or(0);
    let timestamp = cursor.read_u32().unwrap_or(sequence);
    let operator = wire::read_string_value(&tlvs, 0x02)?.unwrap_or_else(|| "field-operator".to_owned());
    let template = if flags & 0x01 != 0 {
        unsafe { bank.reuse_cached_template(template_id, revision, station) }
    } else {
        bank.lookup(template_id, revision, station)
    }
    .ok_or(MeshError::MissingTemplate)?;
    let mut items = Vec::new();
    for value in wire::values(&tlvs, 0x10) {
        decode_item_block(value, flags, template, dictionary, &mut items)?;
    }
    if items.is_empty() {
        synthesize_empty_items(template, dictionary, &mut items);
    }
    let mut alerts = Vec::new();
    for item in &items {
        if item.quality < 20 {
            alerts.push(format!("low confidence {}", item.field_code));
        }
        if matches!(template_kind(template, item.field_code), Some(FieldKind::Temperature))
            && item.quantity.unsigned_abs() > 800
        {
            alerts.push(format!("temperature outlier {}", item.quantity));
        }
    }
    if flags & 0x08 != 0 && alerts.len() > 3 {
        diagnostics.push(Diagnostic {
            code: 0x4108,
            sequence,
            detail: "manifest carried many alertable items".to_owned(),
        });
    }
    Ok(Manifest {
        station,
        template_id,
        revision,
        timestamp,
        operator,
        items,
        alerts,
    })
}

fn decode_item_block(
    value: &[u8],
    flags: u8,
    template: &SupplyTemplate,
    dictionary: &mut Dictionary,
    items: &mut Vec<ManifestItem>,
) -> Result<()> {
    let mut cursor = ByteCursor::new(value);
    while cursor.remaining() >= 8 {
        let field_index = cursor.read_u8()? as usize;
        let raw = cursor.read_svar_i32().unwrap_or(0);
        let quality = cursor.read_u8().unwrap_or(100);
        let phrase_code = cursor.read_u16().unwrap_or(0);
        let unit_code = cursor.read_u8().unwrap_or(0);
        let field = if template.fields.is_empty() {
            None
        } else {
            template.fields.get(field_index % template.fields.len())
        };
        let field_code = field.map(|field| field.code).unwrap_or(field_index as u16);
        let scale = field.map(|field| field.scale).unwrap_or(0);
        let quantity = apply_scale(raw, scale);
        let phrase = if flags & 0x04 != 0 {
            unsafe { dictionary.resolve_cached_phrase(phrase_code) }
                .unwrap_or_else(|| dictionary.resolve_phrase(phrase_code))
        } else if phrase_code == 0 {
            field.map(|field| dictionary.resolve_phrase(field.phrase_code))
                .unwrap_or_else(|| "unlabeled field".to_owned())
        } else {
            dictionary.resolve_phrase(phrase_code)
        };
        items.push(ManifestItem {
            field_code,
            quantity,
            quality,
            unit: unit_for(unit_code),
            phrase,
        });
    }
    Ok(())
}

fn synthesize_empty_items(
    template: &SupplyTemplate,
    dictionary: &mut Dictionary,
    items: &mut Vec<ManifestItem>,
) {
    for field in template.fields.iter().take(4) {
        items.push(ManifestItem {
            field_code: field.code,
            quantity: 0,
            quality: 1,
            unit: unit_for(field.code as u8),
            phrase: dictionary.resolve_phrase(field.phrase_code),
        });
    }
}

fn template_kind(template: &SupplyTemplate, field_code: u16) -> Option<FieldKind> {
    template
        .fields
        .iter()
        .find(|field| field.code == field_code)
        .map(|field| field.kind)
}

fn apply_scale(value: i32, scale: i8) -> i32 {
    if scale > 0 {
        let places = scale.unsigned_abs().min(9) as u32;
        value.saturating_mul(10i32.saturating_pow(places))
    } else if scale < 0 {
        let places = scale.unsigned_abs().min(9) as u32;
        let divisor = 10i32.saturating_pow(places).max(1);
        value / divisor
    } else {
        value
    }
}

fn unit_for(code: u8) -> String {
    match code & 0x0f {
        0 => "count",
        1 => "liters",
        2 => "kg",
        3 => "celsius-x10",
        4 => "percent",
        5 => "minutes",
        6 => "kits",
        7 => "beds",
        8 => "amp-hours",
        9 => "meters",
        10 => "vials",
        11 => "packs",
        12 => "routes",
        13 => "alerts",
        14 => "people",
        _ => "units",
    }
    .to_owned()
}
