#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(batch) = lifelinemesh::parse(data) {
        let _ = lifelinemesh::analytics::score_batch(&batch);
    }
});
