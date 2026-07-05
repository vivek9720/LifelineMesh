pub mod phrases;
pub mod stations;
pub mod supply;
pub mod zones;

use crate::model::StationClass;

#[derive(Debug, Clone, Copy)]
pub struct StationRecord {
    pub code: u16,
    pub region: &'static str,
    pub class: StationClass,
    pub capacity: u16,
    pub callsign: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct PhraseRecord {
    pub code: u16,
    pub locale: u8,
    pub text: &'static str,
}

pub fn station(code: u16) -> Option<&'static StationRecord> {
    stations::station_by_code(code)
}
