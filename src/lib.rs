//! LifelineMesh decodes disaster-response mesh logistics traffic.
//!
//! The parser is deliberately layered: a byte stream becomes wire frames, frames
//! become typed payloads, payloads update a long-lived session, and session state
//! is then analyzed for operational consistency.

pub mod analytics;
pub mod catalog;
pub mod checksum;
pub mod cursor;
pub mod dictionary;
pub mod error;
pub mod fragment;
pub mod frame;
pub mod manifest;
pub mod model;
pub mod route;
pub mod script;
pub mod session;
pub mod template;
pub mod wire;

pub use error::{MeshError, Result};
pub use model::{Batch, FrameKind};

/// Decode a complete LifelineMesh batch.
pub fn parse(data: &[u8]) -> Result<Batch> {
    session::MeshSession::decode_batch(data)
}

/// Decode a byte stream and run post-parse analytics.
pub fn parse_and_score(data: &[u8]) -> Result<analytics::BatchScore> {
    let batch = parse(data)?;
    Ok(analytics::score_batch(&batch))
}

/// Decode only the manifest path. Fuzzers use this to spend more time inside
/// template and dictionary lifecycles while still accepting complete frames.
pub fn parse_manifest_focus(data: &[u8]) -> Result<Batch> {
    let mut session = session::MeshSession::new();
    for frame in frame::parse_frames(data)? {
        if matches!(
            frame.kind,
            FrameKind::Dictionary | FrameKind::Template | FrameKind::Manifest | FrameKind::Maintenance
        ) {
            session.ingest_frame(frame)?;
        }
    }
    Ok(session.finish())
}

/// Decode route and script frames, then run a recovery simulation over the
/// accumulated state.
pub fn parse_route_script_focus(data: &[u8]) -> Result<analytics::RecoveryScore> {
    let mut session = session::MeshSession::new();
    for frame in frame::parse_frames(data)? {
        if matches!(
            frame.kind,
            FrameKind::Route | FrameKind::Overlay | FrameKind::Script | FrameKind::Maintenance
        ) {
            session.ingest_frame(frame)?;
        }
    }
    let batch = session.finish();
    Ok(analytics::simulate_recovery(&batch))
}
