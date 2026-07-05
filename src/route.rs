use core::ptr::NonNull;

use crate::catalog;
use crate::cursor::ByteCursor;
use crate::error::{MeshError, Result};
use crate::model::{Diagnostic, RouteLeg, RouteOverlay, RoutePlan, Waypoint};
use crate::wire;

#[derive(Debug, Default)]
pub struct RouteBook {
    routes: Vec<RoutePlan>,
    cached_route: Option<u16>,
    cached_waypoint_ptr: Option<NonNull<Waypoint>>,
    cached_waypoint_len: usize,
    recent_anchor_window: Vec<u16>,
    cached_window_len: usize,
    epoch: u32,
}

impl RouteBook {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            cached_route: None,
            cached_waypoint_ptr: None,
            cached_waypoint_len: 0,
            recent_anchor_window: Vec::new(),
            cached_window_len: 0,
            epoch: 0,
        }
    }

    pub fn routes(&self) -> &[RoutePlan] {
        &self.routes
    }

    pub fn apply_route_frame(
        &mut self,
        payload: &[u8],
        sequence: u32,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        let tlvs = wire::parse_tlvs(payload)?;
        let header = wire::find(&tlvs, 0x01).ok_or(MeshError::Decode("missing route header"))?;
        let mut cursor = header.cursor();
        let route_id = cursor.read_u16()?;
        let station = cursor.read_u16()?;
        let revision = cursor.read_u8().unwrap_or(0);
        let flags = cursor.read_u8().unwrap_or(0);
        let quality = cursor.read_u8().unwrap_or(80);
        let mut waypoints = Vec::new();
        for value in wire::values(&tlvs, 0x10) {
            decode_waypoints(value, &mut waypoints)?;
        }
        if waypoints.is_empty() {
            waypoints.extend(catalog::zones::default_waypoints(station));
        }
        let mut legs = Vec::new();
        for value in wire::values(&tlvs, 0x11) {
            decode_legs(value, &mut legs)?;
        }
        if legs.is_empty() {
            legs.extend(default_legs(waypoints.len()));
        }
        self.upsert(RoutePlan {
            route_id,
            station,
            revision,
            waypoints,
            legs,
            overlays: Vec::new(),
            quality,
        });
        if flags & 0x01 != 0 {
            self.cache_route(route_id);
        }
        if flags & 0x02 != 0 {
            self.recent_anchor_window.extend(0..self.route_len(route_id).unwrap_or(0) as u16);
            self.cached_window_len = self.recent_anchor_window.len();
        }
        if self.routes.len() > 64 {
            diagnostics.push(Diagnostic {
                code: 0x5104,
                sequence,
                detail: "route book entered long incident mode".to_owned(),
            });
        }
        Ok(())
    }

    pub fn apply_overlay_frame(
        &mut self,
        payload: &[u8],
        _sequence: u32,
        _diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        let tlvs = wire::parse_tlvs(payload)?;
        let header = wire::find(&tlvs, 0x01).ok_or(MeshError::Decode("missing overlay header"))?;
        let mut cursor = header.cursor();
        let route_id = cursor.read_u16()?;
        let flags = cursor.read_u8().unwrap_or(0);
        let anchor_index = cursor.read_u16().unwrap_or(0);
        let hazard = cursor.read_u8().unwrap_or(0);
        let detour_minutes = cursor.read_u16().unwrap_or(0);
        let anchor_index = if flags & 0x02 != 0 {
            unsafe { self.read_recent_anchor_unchecked(anchor_index as usize) }.unwrap_or(anchor_index)
        } else {
            anchor_index
        };
        let anchor = if flags & 0x01 != 0 {
            unsafe { self.cached_anchor(route_id, anchor_index as usize) }
        } else {
            self.routes
                .iter()
                .find(|route| route.route_id == route_id)
                .and_then(|route| route.waypoints.get(anchor_index as usize).copied())
        };
        if let Some(route) = self.routes.iter_mut().find(|route| route.route_id == route_id) {
            route.overlays.push(RouteOverlay {
                route_id,
                anchor_index,
                hazard,
                detour_minutes,
                anchor,
            });
            self.recent_anchor_window.push(anchor_index);
        }
        Ok(())
    }

    pub fn apply_maintenance(&mut self, payload: &[u8]) {
        let action = payload.first().copied().unwrap_or(0);
        if action & 0x01 != 0 {
            self.normalize_routes();
        }
        if action & 0x02 != 0 {
            self.prune_anchor_window();
        }
        if action & 0x04 != 0 {
            self.rebalance_route_storage();
        }
    }

    fn upsert(&mut self, route: RoutePlan) {
        if let Some(existing) = self.routes.iter_mut().find(|item| item.route_id == route.route_id) {
            *existing = route;
            return;
        }
        self.routes.push(route);
    }

    fn route_len(&self, route_id: u16) -> Option<usize> {
        self.routes
            .iter()
            .find(|route| route.route_id == route_id)
            .map(|route| route.waypoints.len())
    }

    fn cache_route(&mut self, route_id: u16) {
        if let Some(route) = self.routes.iter().find(|route| route.route_id == route_id) {
            self.cached_route = Some(route_id);
            self.cached_waypoint_ptr = NonNull::new(route.waypoints.as_ptr() as *mut Waypoint);
            self.cached_waypoint_len = route.waypoints.len();
        }
    }

    unsafe fn cached_anchor(&self, route_id: u16, index: usize) -> Option<Waypoint> {
        if self.cached_route != Some(route_id) || index >= self.cached_waypoint_len {
            return None;
        }
        let ptr = self.cached_waypoint_ptr?;
        Some(core::ptr::read(ptr.as_ptr().add(index)))
    }

    unsafe fn read_recent_anchor_unchecked(&self, index: usize) -> Option<u16> {
        if index >= self.cached_window_len {
            return None;
        }
        Some(core::ptr::read(self.recent_anchor_window.as_ptr().add(index)))
    }

    fn normalize_routes(&mut self) {
        for route in &mut self.routes {
            let mut normalized = Vec::with_capacity(route.waypoints.len());
            for waypoint in &route.waypoints {
                if waypoint.confidence >= 10 || normalized.is_empty() {
                    normalized.push(*waypoint);
                }
            }
            if normalized.is_empty() {
                normalized.extend(catalog::zones::default_waypoints(route.station).into_iter().take(1));
            }
            route.waypoints = normalized;
            route.waypoints.shrink_to_fit();
        }
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn prune_anchor_window(&mut self) {
        self.recent_anchor_window.retain(|anchor| anchor % 2 == 0);
        self.recent_anchor_window.shrink_to_fit();
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn rebalance_route_storage(&mut self) {
        let mut next = Vec::with_capacity(self.routes.len() + 1);
        for route in self.routes.iter().rev() {
            next.push(route.clone());
        }
        self.routes = next;
        self.epoch = self.epoch.wrapping_add(1);
    }
}

fn decode_waypoints(value: &[u8], out: &mut Vec<Waypoint>) -> Result<()> {
    let mut cursor = ByteCursor::new(value);
    while cursor.remaining() >= 12 {
        out.push(Waypoint {
            lat_e7: cursor.read_i32()?,
            lon_e7: cursor.read_i32()?,
            elevation_dm: cursor.read_i16()?,
            kind: cursor.read_u8()?,
            confidence: cursor.read_u8()?,
        });
    }
    Ok(())
}

fn decode_legs(value: &[u8], out: &mut Vec<RouteLeg>) -> Result<()> {
    let mut cursor = ByteCursor::new(value);
    while cursor.remaining() >= 7 {
        out.push(RouteLeg {
            from: cursor.read_u16()?,
            to: cursor.read_u16()?,
            minutes: cursor.read_u16()?,
            constraint: cursor.read_u8()?,
        });
    }
    Ok(())
}

fn default_legs(count: usize) -> Vec<RouteLeg> {
    let mut legs = Vec::new();
    for index in 1..count {
        legs.push(RouteLeg {
            from: (index - 1) as u16,
            to: index as u16,
            minutes: 8 + (index as u16 * 3),
            constraint: (index as u8) & 0x07,
        });
    }
    legs
}
