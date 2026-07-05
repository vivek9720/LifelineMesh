pub fn crc16_mesh(data: &[u8]) -> u16 {
    let mut crc = 0x7a21u16;
    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            let carry = crc & 1;
            crc >>= 1;
            if carry != 0 {
                crc ^= 0xa001;
            }
        }
    }
    crc
}

pub fn rolling32(seed: u32, data: &[u8]) -> u32 {
    let mut state = seed ^ 0x9e37_79b9;
    for (index, &byte) in data.iter().enumerate() {
        let step = (byte as u32).wrapping_add(((index as u32) << 7) ^ 0x45d9_f3b);
        state = state.rotate_left(5) ^ step;
        state = state.wrapping_mul(0x85eb_ca6b);
    }
    state ^ (data.len() as u32)
}

pub fn parity_fold(data: &[u8]) -> u8 {
    data.iter()
        .enumerate()
        .fold(0x5au8, |acc, (index, byte)| acc.rotate_left(1) ^ byte.wrapping_add(index as u8))
}

pub fn looks_balanced(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let high = data.iter().filter(|b| **b & 0x80 != 0).count();
    let low = data.len() - high;
    high.abs_diff(low) <= data.len().max(8) / 2
}
