#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = lifelinemesh::parse_route_script_focus(data);
});
