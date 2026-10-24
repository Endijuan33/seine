//! Native full-size runtime-dispatch parity, including dirty reuse and switches.
use argon2::{Algorithm, Argon2, Params, Version};
use blocknet_pow_kernel::{CpuKernel, FixedArgon2id, PowBlock};

fn main() {
    assert!(
        CpuKernel::NeonSha3.is_supported(),
        "requires native ARM SHA3 support"
    );
    const MEMORY_KIB: u32 = 2 * 1024 * 1024;
    let salt = b"runtime-neon-full-size";
    let reference = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(MEMORY_KIB, 1, 1, Some(32)).unwrap(),
    );
    let mut cases = Vec::new();
    for nonce in [0u64, 12345, u64::MAX] {
        let mut output = [0u8; 32];
        reference
            .hash_password_into(&nonce.to_le_bytes(), salt, &mut output)
            .unwrap();
        cases.push((nonce, output));
    }
    let mut arena = vec![PowBlock::default(); MEMORY_KIB as usize];
    let mut hasher = FixedArgon2id::with_kernel(MEMORY_KIB, CpuKernel::Neon).unwrap();
    for (nonce, expected) in cases {
        for kernel in [CpuKernel::Neon, CpuKernel::NeonSha3, CpuKernel::Neon] {
            hasher.set_kernel(kernel).unwrap();
            let mut output = [0u8; 32];
            hasher
                .hash_password_into_with_memory(&nonce.to_le_bytes(), salt, &mut output, &mut arena)
                .unwrap();
            assert_eq!(output, expected, "nonce={nonce},kernel={kernel}");
            println!(
                "PASS nonce={nonce} kernel={kernel} full_size_bytes={} dirty_reuse=true",
                MEMORY_KIB as u64 * 1024
            );
        }
    }
}
