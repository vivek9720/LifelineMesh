#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Dictionary,
    Template,
    Manifest,
    Route,
    Overlay,
    Script,
    Maintenance,
    Fragment,
    Heartbeat,
    Unknown(u8),
}

impl FrameKind {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => FrameKind::Dictionary,
            0x02 => FrameKind::Template,
            0x03 => FrameKind::Manifest,
            0x04 => FrameKind::Route,
            0x05 => FrameKind::Overlay,
            0x06 => FrameKind::Script,
            0x07 => FrameKind::Maintenance,
            0x08 => FrameKind::Fragment,
            0x09 => FrameKind::Heartbeat,
            other => FrameKind::Unknown(other),
        }
    }

    pub fn as_byte(self) -> u8 {
        match self {
            FrameKind::Dictionary => 0x01,
            FrameKind::Template => 0x02,
            FrameKind::Manifest => 0x03,
            FrameKind::Route => 0x04,
            FrameKind::Overlay => 0x05,
            FrameKind::Script => 0x06,
            FrameKind::Maintenance => 0x07,
            FrameKind::Fragment => 0x08,
            FrameKind::Heartbeat => 0x09,
            FrameKind::Unknown(v) => v,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationClass {
    Shelter,
    Clinic,
    Relay,
    Depot,
    LandingZone,
    WaterPoint,
    MobileCommand,
    EvacHub,
}

impl StationClass {
    pub fn from_nibble(value: u8) -> Self {
        match value & 0x07 {
            0 => StationClass::Shelter,
            1 => StationClass::Clinic,
            2 => StationClass::Relay,
            3 => StationClass::Depot,
            4 => StationClass::LandingZone,
            5 => StationClass::WaterPoint,
            6 => StationClass::MobileCommand,
            _ => StationClass::EvacHub,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub version: u8,
    pub flags: u8,
    pub stream_id: u16,
    pub sequence: u32,
    pub kind: FrameKind,
    pub header_len: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn wants_checksum(&self) -> bool {
        self.flags & 0x40 != 0
    }

    pub fn is_replay(&self) -> bool {
        self.flags & 0x02 != 0
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: u16,
    pub sequence: u32,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub frames_seen: usize,
    pub manifests: Vec<Manifest>,
    pub routes: Vec<RoutePlan>,
    pub scripts: Vec<ScriptBlock>,
    pub diagnostics: Vec<Diagnostic>,
    pub dictionary_terms: usize,
    pub station_count: usize,
}

#[derive(Debug, Clone)]
pub struct TemplateField {
    pub code: u16,
    pub kind: FieldKind,
    pub scale: i8,
    pub optional: bool,
    pub phrase_code: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Quantity,
    Temperature,
    Battery,
    Occupancy,
    Medicine,
    Water,
    Fuel,
    FreeText,
}

impl FieldKind {
    pub fn from_byte(value: u8) -> Self {
        match value & 0x07 {
            0 => FieldKind::Quantity,
            1 => FieldKind::Temperature,
            2 => FieldKind::Battery,
            3 => FieldKind::Occupancy,
            4 => FieldKind::Medicine,
            5 => FieldKind::Water,
            6 => FieldKind::Fuel,
            _ => FieldKind::FreeText,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TemplateLane {
    pub lane_id: u16,
    pub field_code: u16,
    pub reliability: u8,
    pub transform: u8,
}

#[derive(Debug, Clone)]
pub struct SupplyTemplate {
    pub id: u16,
    pub revision: u8,
    pub station: u16,
    pub retention: u8,
    pub fields: Vec<TemplateField>,
    pub lanes: Vec<TemplateLane>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub station: u16,
    pub template_id: u16,
    pub revision: u8,
    pub timestamp: u32,
    pub operator: String,
    pub items: Vec<ManifestItem>,
    pub alerts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ManifestItem {
    pub field_code: u16,
    pub quantity: i32,
    pub quality: u8,
    pub unit: String,
    pub phrase: String,
}

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub route_id: u16,
    pub station: u16,
    pub revision: u8,
    pub waypoints: Vec<Waypoint>,
    pub legs: Vec<RouteLeg>,
    pub overlays: Vec<RouteOverlay>,
    pub quality: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Waypoint {
    pub lat_e7: i32,
    pub lon_e7: i32,
    pub elevation_dm: i16,
    pub kind: u8,
    pub confidence: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct RouteLeg {
    pub from: u16,
    pub to: u16,
    pub minutes: u16,
    pub constraint: u8,
}

#[derive(Debug, Clone)]
pub struct RouteOverlay {
    pub route_id: u16,
    pub anchor_index: u16,
    pub hazard: u8,
    pub detour_minutes: u16,
    pub anchor: Option<Waypoint>,
}

#[derive(Debug, Clone)]
pub struct ScriptBlock {
    pub id: u16,
    pub revision: u8,
    pub priority: u8,
    pub labels: Vec<String>,
    pub instructions: Vec<ScriptInstruction>,
    pub leases: Vec<Lease>,
}

#[derive(Debug, Clone)]
pub struct ScriptInstruction {
    pub opcode: u8,
    pub arg0: u16,
    pub arg1: u16,
    pub phrase: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Lease {
    pub station: u16,
    pub route_id: u16,
    pub expires_at: u32,
    pub flags: u8,
}
