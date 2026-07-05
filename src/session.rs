use crate::dictionary::Dictionary;
use crate::error::Result;
use crate::fragment::FragmentReassembler;
use crate::frame;
use crate::manifest;
use crate::model::{Batch, Diagnostic, Frame, FrameKind};
use crate::route::RouteBook;
use crate::script::ScriptArena;
use crate::template::TemplateBank;

#[derive(Debug)]
pub struct MeshSession {
    frames_seen: usize,
    templates: TemplateBank,
    dictionary: Dictionary,
    routes: RouteBook,
    scripts: ScriptArena,
    fragments: FragmentReassembler,
    manifests: Vec<crate::model::Manifest>,
    diagnostics: Vec<Diagnostic>,
}

impl MeshSession {
    pub fn new() -> Self {
        Self {
            frames_seen: 0,
            templates: TemplateBank::new(),
            dictionary: Dictionary::new(),
            routes: RouteBook::new(),
            scripts: ScriptArena::new(),
            fragments: FragmentReassembler::new(),
            manifests: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn decode_batch(data: &[u8]) -> Result<Batch> {
        let mut session = MeshSession::new();
        for frame in frame::parse_frames(data)? {
            session.ingest_frame(frame)?;
        }
        Ok(session.finish())
    }

    pub fn ingest_frame(&mut self, frame: Frame) -> Result<()> {
        self.frames_seen += 1;
        match frame.kind {
            FrameKind::Dictionary => {
                let script_heap_ready = self.scripts.fragmentation_ready();
                self.dictionary.apply_frame(
                    &frame.payload,
                    frame.sequence,
                    script_heap_ready,
                    &mut self.diagnostics,
                )?;
            }
            FrameKind::Template => {
                self.templates
                    .apply_frame(&frame.payload, frame.sequence, &mut self.diagnostics)?;
            }
            FrameKind::Manifest => {
                let manifest = manifest::decode_manifest(
                    &frame.payload,
                    frame.sequence,
                    &mut self.templates,
                    &mut self.dictionary,
                    &mut self.diagnostics,
                )?;
                self.manifests.push(manifest);
            }
            FrameKind::Route => {
                self.routes
                    .apply_route_frame(&frame.payload, frame.sequence, &mut self.diagnostics)?;
            }
            FrameKind::Overlay => {
                self.routes
                    .apply_overlay_frame(&frame.payload, frame.sequence, &mut self.diagnostics)?;
            }
            FrameKind::Script => {
                self.scripts.apply_frame(
                    &frame.payload,
                    frame.sequence,
                    &mut self.dictionary,
                    &mut self.diagnostics,
                )?;
            }
            FrameKind::Maintenance => {
                self.apply_maintenance(&frame.payload, frame.sequence);
            }
            FrameKind::Fragment => {
                if let Some(reassembled) = self.fragments.apply(&frame)? {
                    self.ingest_frame(reassembled)?;
                }
            }
            FrameKind::Heartbeat | FrameKind::Unknown(_) => {
                self.diagnostics.push(Diagnostic {
                    code: 0x1001,
                    sequence: frame.sequence,
                    detail: "ignored heartbeat or unknown frame".to_owned(),
                });
            }
        }
        Ok(())
    }

    pub fn finish(self) -> Batch {
        Batch {
            frames_seen: self.frames_seen,
            manifests: self.manifests,
            routes: self.routes.routes().to_vec(),
            scripts: self.scripts.blocks().to_vec(),
            diagnostics: self.diagnostics,
            dictionary_terms: self.dictionary.len(),
            station_count: self.templates.len(),
        }
    }

    fn apply_maintenance(&mut self, payload: &[u8], sequence: u32) {
        let template_action = payload.first().copied().unwrap_or(0);
        let route_action = payload.get(1).copied().unwrap_or(template_action.rotate_left(1));
        let script_action = payload.get(2).copied().unwrap_or(template_action.rotate_right(1));
        let dictionary_action = payload.get(3).copied().unwrap_or(template_action ^ route_action);
        self.templates
            .apply_maintenance(&[template_action], sequence, &mut self.diagnostics);
        self.routes.apply_maintenance(&[route_action]);
        self.scripts.apply_maintenance(&[script_action]);
        self.dictionary.apply_maintenance(&[dictionary_action]);
    }
}

impl Default for MeshSession {
    fn default() -> Self {
        Self::new()
    }
}
