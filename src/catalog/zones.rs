use crate::model::Waypoint;

pub fn default_waypoints(station: u16) -> Vec<Waypoint> {
    let base_lat = 250_000_000i32.wrapping_add((station as i32).wrapping_mul(1_731));
    let base_lon = -970_000_000i32.wrapping_sub((station as i32).wrapping_mul(1_453));
    let count = 4 + (station as usize % 4);
    let mut waypoints = Vec::with_capacity(count);
    for index in 0..count {
        waypoints.push(Waypoint {
            lat_e7: base_lat.wrapping_add((index as i32).wrapping_mul(971)),
            lon_e7: base_lon.wrapping_sub((index as i32).wrapping_mul(733)),
            elevation_dm: 20 + ((station as i16).wrapping_add(index as i16 * 7) % 480),
            kind: ((station as usize + index) % 9) as u8,
            confidence: 35 + ((station as usize * 5 + index * 11) % 64) as u8,
        });
    }
    waypoints
}

pub fn waypoint_count(station: u16) -> usize {
    4 + (station as usize % 4)
}
