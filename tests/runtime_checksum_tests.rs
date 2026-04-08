use barred_luau::serializer::fnv1a32;

fn luau_mul32(a: u32, b: u32) -> u32 {
    let a_lo = a & 0xFFFF;
    let a_hi = a >> 16;
    let b_lo = b & 0xFFFF;
    let b_hi = b >> 16;
    let low = a_lo * b_lo;
    let cross = a_hi * b_lo + a_lo * b_hi;
    low.wrapping_add((cross & 0xFFFF) << 16)
}

fn luau_add32(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

fn luau_runtime_fnv32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811C_9DC5u32;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = luau_mul32(hash, 0x0100_0193);
    }
    hash
}

#[test]
fn luau_mul32_matches_wrapping_mul() {
    let cases = [
        (0u32, 0u32),
        (1, 1),
        (0xFFFF, 0xFFFF),
        (0xFFFF_FFFF, 0x0100_0193),
        (0x811C_9DC5, 0x0100_0193),
        (0xDEAD_BEEF, 1664525),
        (0xA5A5_5A5A, 1664525),
    ];

    for (lhs, rhs) in cases {
        assert_eq!(luau_mul32(lhs, rhs), lhs.wrapping_mul(rhs));
    }
}

#[test]
fn luau_add32_matches_wrapping_add() {
    let cases = [
        (0u32, 0u32),
        (1, 2),
        (0xFFFF_FFFF, 1),
        (0xDEAD_BEEF, 0x1111_2222),
    ];

    for (lhs, rhs) in cases {
        assert_eq!(luau_add32(lhs, rhs), lhs.wrapping_add(rhs));
    }
}

#[test]
fn luau_runtime_fnv_matches_rust_checksum() {
    let payload = b"roblox-runtime-checksum-regression-payload";
    assert_eq!(luau_runtime_fnv32(payload), fnv1a32(payload));
}
