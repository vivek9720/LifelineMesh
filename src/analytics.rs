use crate::model::{Batch, FieldKind, RoutePlan, ScriptBlock};

#[derive(Debug, Clone, Copy)]
pub struct BatchScore {
    pub manifest_pressure: u32,
    pub route_pressure: u32,
    pub automation_pressure: u32,
    pub alert_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct RecoveryScore {
    pub reachable_stations: u32,
    pub estimated_minutes: u32,
    pub script_actions: u32,
    pub risk: u32,
}

pub fn score_batch(batch: &Batch) -> BatchScore {
    let mut manifest_pressure = 0u32;
    let mut alert_count = 0u32;
    for manifest in &batch.manifests {
        alert_count += manifest.alerts.len() as u32;
        for item in &manifest.items {
            manifest_pressure = manifest_pressure.wrapping_add(item.quantity.unsigned_abs());
            manifest_pressure = manifest_pressure.wrapping_add(item.quality as u32);
            manifest_pressure ^= (item.field_code as u32) << 2;
        }
    }
    let route_pressure = batch
        .routes
        .iter()
        .fold(0u32, |acc, route| acc.wrapping_add(route_pressure(route)));
    let automation_pressure = batch
        .scripts
        .iter()
        .fold(0u32, |acc, script| acc.wrapping_add(script_pressure(script)));
    BatchScore {
        manifest_pressure,
        route_pressure,
        automation_pressure,
        alert_count,
    }
}

pub fn simulate_recovery(batch: &Batch) -> RecoveryScore {
    let mut reachable_stations = batch.station_count as u32;
    let mut estimated_minutes = 0u32;
    let mut script_actions = 0u32;
    let mut risk = batch.diagnostics.len() as u32;
    for route in &batch.routes {
        reachable_stations = reachable_stations.wrapping_add(route.waypoints.len() as u32);
        let leg_minutes = route
            .legs
            .iter()
            .fold(0u32, |acc, leg| acc.wrapping_add(leg.minutes as u32));
        let detour_minutes = route.overlays.iter().fold(0u32, |acc, overlay| {
            acc.wrapping_add(overlay.detour_minutes as u32)
        });
        estimated_minutes = estimated_minutes.wrapping_add(leg_minutes.wrapping_add(detour_minutes));
        risk = risk.wrapping_add(route.overlays.iter().filter(|overlay| overlay.hazard > 5).count() as u32);
    }
    for script in &batch.scripts {
        script_actions = script_actions.wrapping_add(script.instructions.len() as u32);
        risk ^= script_pressure(script);
    }
    for manifest in &batch.manifests {
        for item in &manifest.items {
            if item.unit == "celsius-x10" && item.quantity.unsigned_abs() > 400 {
                risk = risk.wrapping_add(11);
            }
            if item.phrase.contains("evac") {
                reachable_stations = reachable_stations.wrapping_add(1);
            }
        }
    }
    RecoveryScore {
        reachable_stations,
        estimated_minutes,
        script_actions,
        risk,
    }
}

fn route_pressure(route: &RoutePlan) -> u32 {
    let mut score = route.quality as u32;
    for waypoint in &route.waypoints {
        score = score.wrapping_add(waypoint.confidence as u32);
        score ^= waypoint.lat_e7.unsigned_abs().rotate_left((waypoint.kind & 7) as u32);
        score = score.wrapping_add(waypoint.lon_e7.unsigned_abs() & 0xffff);
    }
    for leg in &route.legs {
        score = score.wrapping_add(leg.minutes as u32);
        score ^= ((leg.from as u32) << 8) | leg.to as u32;
    }
    for overlay in &route.overlays {
        score = score.wrapping_add(overlay.detour_minutes as u32 * 3);
        score ^= overlay.hazard as u32;
    }
    score
}

fn script_pressure(script: &ScriptBlock) -> u32 {
    let mut score = script.priority as u32;
    for instruction in &script.instructions {
        score = score.wrapping_mul(33).wrapping_add(instruction.opcode as u32);
        score ^= ((instruction.arg0 as u32) << 16) | instruction.arg1 as u32;
        if instruction.phrase.contains("water") {
            score = score.wrapping_add(FieldKind::Water as u32);
        }
    }
    for lease in &script.leases {
        score ^= lease.expires_at.rotate_left((lease.flags & 7) as u32);
    }
    score
}
