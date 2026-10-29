//! Faraday BLAKE2b-512 PoW on NVIDIA GPUs. The host builds the template
//! midstate with `blocknet_pow_kernel`; the device hashes the final block per
//! nonce and reports nonces whose first output limb passes the target. The
//! host re-checks every candidate with the CPU kernel, so only exact hits leave
//! `scan`. Not wired into the mining runtime yet.
#![cfg_attr(not(test), allow(dead_code))]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use blocknet_pow_kernel::{Blake2bPowMidstate, Blake2bPowSingleBlock};
use cudarc::driver::{
    sys::CUdevice_attribute, CudaContext, CudaFunction, CudaSlice, CudaStream, DeviceRepr,
    LaunchConfig, PushKernelArg,
};
use cudarc::nvrtc::Ptx;

use super::{
    build_cubin_cache_key, compile_cuda_module_with_nvrtc_fallback, cubin_cache_file_path,
    cuda_driver_err, load_cached_cubin, nvrtc_compiler_identity, persist_cached_cubin,
    require_cuda_libraries, NvrtcModuleArtifact,
};

const KERNEL_SRC: &str = include_str!("blake2b_kernel.cu");
const KERNEL_NAME: &str = "blake2b_pow_scan_kernel";
const PROGRAM_NAME: &str = "seine_blake2b_pow.cu";
const THREADS_PER_BLOCK: u32 = 256;
const BLOCKS_PER_SM: u32 = 16;
const CANDIDATE_CAPACITY: usize = 4096;

/// Mirrors `Blake2bPowParams` in the kernel; passed by value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KernelParams {
    h: [u64; 8],
    m: [u64; 16],
    counter_lo: u64,
    counter_hi: u64,
    target_limb0: u64,
}

unsafe impl DeviceRepr for KernelParams {}

/// Compile-time layout; one module per distinct header layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KernelShape {
    nonce_word: usize,
    nonce_shift: u32,
    message_words: usize,
}

impl KernelShape {
    fn of(layout: &Blake2bPowSingleBlock) -> Self {
        Self {
            nonce_word: layout.nonce_word,
            nonce_shift: layout.nonce_shift,
            message_words: layout.message_words,
        }
    }
}

pub(super) struct CudaBlake2bEngine {
    ctx: Arc<CudaContext>,
    stream: Arc<CudaStream>,
    arch: (i32, i32),
    cubin_cache_dir: PathBuf,
    kernels: Vec<(KernelShape, CudaFunction)>,
    candidates: CudaSlice<u64>,
    candidate_count: CudaSlice<u32>,
    grid_blocks: u32,
}

impl CudaBlake2bEngine {
    pub(super) fn new(device_index: u32, cubin_cache_dir: &Path) -> Result<Self> {
        require_cuda_libraries()?;
        let ctx = CudaContext::new(device_index as usize).map_err(|err| {
            anyhow!("failed to open CUDA context on device {device_index}: {err:?}")
        })?;
        let stream = ctx.default_stream();
        let arch = ctx
            .compute_capability()
            .map_err(|err| anyhow!("failed to query compute capability: {err:?}"))?;
        let sm_count = ctx
            .attribute(CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT)
            .map_err(cuda_driver_err)?;
        let candidates = stream
            .alloc_zeros::<u64>(CANDIDATE_CAPACITY)
            .map_err(cuda_driver_err)?;
        let candidate_count = stream.alloc_zeros::<u32>(1).map_err(cuda_driver_err)?;
        Ok(Self {
            ctx,
            stream,
            arch,
            cubin_cache_dir: cubin_cache_dir.to_path_buf(),
            kernels: Vec::new(),
            candidates,
            candidate_count,
            grid_blocks: u32::try_from(sm_count.max(1)).unwrap_or(1) * BLOCKS_PER_SM,
        })
    }

    /// Nonces per full launch wave; `scan` counts that are multiples of this
    /// keep every thread equally busy.
    pub(super) fn wave_nonces(&self) -> u64 {
        u64::from(self.grid_blocks) * u64::from(THREADS_PER_BLOCK)
    }

    /// Hashes `count` nonces from `start_nonce` (wrapping) and returns every
    /// nonce whose hash meets `target`, in scan order.
    pub(super) fn scan(
        &mut self,
        midstate: &Blake2bPowMidstate,
        start_nonce: u64,
        count: u64,
        target: &[u8; 32],
    ) -> Result<Vec<u64>> {
        let layout = midstate
            .single_block_layout()
            .ok_or_else(|| anyhow!("BLAKE2b header layout needs two blocks per nonce"))?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let kernel = self.kernel(KernelShape::of(&layout))?;
        let params = KernelParams {
            h: layout.h,
            m: layout.message,
            counter_lo: layout.counter as u64,
            counter_hi: (layout.counter >> 64) as u64,
            target_limb0: u64::from_be_bytes(target[..8].try_into().expect("8 bytes")),
        };

        self.stream
            .memset_zeros(&mut self.candidate_count)
            .map_err(cuda_driver_err)?;
        let blocks = u64::from(self.grid_blocks)
            .min(count.div_ceil(u64::from(THREADS_PER_BLOCK)))
            .max(1) as u32;
        let cfg = LaunchConfig {
            grid_dim: (blocks, 1, 1),
            block_dim: (THREADS_PER_BLOCK, 1, 1),
            shared_mem_bytes: 0,
        };
        let capacity = CANDIDATE_CAPACITY as u32;
        unsafe {
            let mut launch = self.stream.launch_builder(&kernel);
            launch
                .arg(&params)
                .arg(&start_nonce)
                .arg(&count)
                .arg(&mut self.candidates)
                .arg(&mut self.candidate_count)
                .arg(&capacity);
            launch.launch(cfg).map_err(cuda_driver_err)?;
        }
        let found = self
            .stream
            .clone_dtoh(&self.candidate_count)
            .map_err(cuda_driver_err)?[0] as usize;

        let mut hits = Vec::new();
        if found > CANDIDATE_CAPACITY {
            // A target this easy is a test or a misconfiguration; stay exact.
            midstate.scan(start_nonce, count, target, |nonce| hits.push(nonce));
            return Ok(hits);
        }
        if found > 0 {
            let candidates = self
                .stream
                .clone_dtoh(&self.candidates)
                .map_err(cuda_driver_err)?;
            hits.extend(
                candidates[..found]
                    .iter()
                    .copied()
                    .filter(|&nonce| midstate.target_bytes(nonce) <= *target),
            );
            hits.sort_unstable_by_key(|&nonce| nonce.wrapping_sub(start_nonce));
        }
        Ok(hits)
    }

    fn kernel(&mut self, shape: KernelShape) -> Result<CudaFunction> {
        if let Some((_, kernel)) = self.kernels.iter().find(|(known, _)| *known == shape) {
            return Ok(kernel.clone());
        }
        let common = vec![
            "--std=c++14".to_string(),
            "--restrict".to_string(),
            format!("-DSEINE_B2_NONCE_WORD={}", shape.nonce_word),
            format!("-DSEINE_B2_NONCE_SHIFT={}", shape.nonce_shift),
            format!("-DSEINE_B2_MSG_WORDS={}", shape.message_words),
        ];
        let (major, minor) = self.arch;
        let mut cubin_options = common.clone();
        cubin_options.push(format!("--gpu-architecture=sm_{major}{minor}"));
        let mut ptx_options = common;
        ptx_options.push(format!("--gpu-architecture=compute_{major}{minor}"));

        let cache_key = build_cubin_cache_key(
            KERNEL_SRC,
            PROGRAM_NAME,
            &cubin_options,
            &nvrtc_compiler_identity(),
        );
        let cache_path = cubin_cache_file_path(&self.cubin_cache_dir, &cache_key);
        let cached = load_cached_cubin(&cache_path)
            .and_then(|cubin| self.ctx.load_module(Ptx::from_binary(cubin)).ok());
        let module = match cached {
            Some(module) => module,
            None => {
                let artifact = compile_cuda_module_with_nvrtc_fallback(
                    KERNEL_SRC,
                    PROGRAM_NAME,
                    &cubin_options,
                    &ptx_options,
                )?;
                match artifact {
                    NvrtcModuleArtifact::Cubin(cubin) => {
                        let _ = persist_cached_cubin(&cache_path, &cubin);
                        self.ctx.load_module(Ptx::from_binary(cubin))
                    }
                    NvrtcModuleArtifact::Ptx(ptx) => self.ctx.load_module(Ptx::from_src(ptx)),
                }
                .map_err(|err| anyhow!("failed to load BLAKE2b CUDA module: {err:?}"))?
            }
        };
        let kernel = module
            .load_function(KERNEL_NAME)
            .map_err(|err| anyhow!("failed to load {KERNEL_NAME}: {err:?}"))?;
        self.kernels.push((shape, kernel.clone()));
        Ok(kernel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blocknet_pow_spec::{BLAKE2B_POW_PERSONAL_MAINNET, FARADAY_HEADER_BASE_LEN};
    use std::time::Instant;

    fn engine(name: &str) -> CudaBlake2bEngine {
        let dir = std::env::temp_dir().join(format!("seine-blake2b-{name}-{}", std::process::id()));
        CudaBlake2bEngine::new(0, &dir).expect("CUDA BLAKE2b engine on device 0")
    }

    fn midstate(len: usize) -> Blake2bPowMidstate {
        let header: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
        Blake2bPowMidstate::new(&BLAKE2B_POW_PERSONAL_MAINNET, &header).unwrap()
    }

    fn target_with_limb0(limb0: u64) -> [u8; 32] {
        let mut target = [0xffu8; 32];
        target[..8].copy_from_slice(&limb0.to_be_bytes());
        target
    }

    #[test]
    #[ignore = "requires NVIDIA GPU + CUDA runtime; run manually under gpuq"]
    fn gpu_blake2b_hits_match_cpu_kernel() {
        let mut engine = engine("parity");
        // The Faraday header, the legacy header and an aligned nonce word.
        for len in [FARADAY_HEADER_BASE_LEN, 92, 152] {
            let midstate = midstate(len);
            let count = 1u64 << 22;
            // About 64 hits per range, wrapping past u64::MAX in the second.
            for start in [0u64, u64::MAX - (count / 2)] {
                for target in [
                    target_with_limb0(u64::MAX >> 16),
                    // Same limb 0; the host must reject on the later limbs.
                    {
                        let mut t = target_with_limb0(u64::MAX >> 16);
                        t[8..].fill(0);
                        t
                    },
                    [0u8; 32],
                ] {
                    let mut expected = Vec::new();
                    midstate.scan(start, count, &target, |nonce| expected.push(nonce));
                    let gpu = engine.scan(&midstate, start, count, &target).unwrap();
                    assert_eq!(gpu, expected, "len={len} start={start:#x}");
                }
            }
        }
    }

    #[test]
    #[ignore = "requires NVIDIA GPU + CUDA runtime; run manually under gpuq"]
    fn gpu_blake2b_rate() {
        let mut engine = engine("rate");
        let midstate = midstate(FARADAY_HEADER_BASE_LEN);
        let target = target_with_limb0(0);
        let launch = engine.wave_nonces() * 256;
        engine.scan(&midstate, 0, launch, &target).unwrap();
        let started = Instant::now();
        let (mut nonce, mut hashes) = (0u64, 0u64);
        while started.elapsed().as_secs_f64() < 5.0 {
            engine.scan(&midstate, nonce, launch, &target).unwrap();
            nonce = nonce.wrapping_add(launch);
            hashes += launch;
        }
        let secs = started.elapsed().as_secs_f64();
        println!(
            "blake2b-rate | launch={launch} hashes={hashes} secs={secs:.3} ghs={:.3}",
            hashes as f64 / secs / 1e9
        );
    }
}
