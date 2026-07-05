use core::fmt;

pub type Result<T> = core::result::Result<T, MeshError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u8),
    InvalidLength,
    InvalidVarint,
    InvalidUtf8,
    ChecksumMismatch,
    UnknownFrame(u8),
    UnknownTag(u8),
    MissingTemplate,
    MissingRoute,
    MissingScript,
    Decode(&'static str),
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshError::Truncated => f.write_str("truncated input"),
            MeshError::BadMagic => f.write_str("bad LifelineMesh magic"),
            MeshError::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            MeshError::InvalidLength => f.write_str("invalid length"),
            MeshError::InvalidVarint => f.write_str("invalid varint"),
            MeshError::InvalidUtf8 => f.write_str("invalid utf-8"),
            MeshError::ChecksumMismatch => f.write_str("checksum mismatch"),
            MeshError::UnknownFrame(k) => write!(f, "unknown frame kind {k:#04x}"),
            MeshError::UnknownTag(t) => write!(f, "unknown tag {t:#04x}"),
            MeshError::MissingTemplate => f.write_str("missing template"),
            MeshError::MissingRoute => f.write_str("missing route"),
            MeshError::MissingScript => f.write_str("missing script"),
            MeshError::Decode(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for MeshError {}
