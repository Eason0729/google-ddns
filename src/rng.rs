//! Minimal CSPRNG backed by the kernel via /dev/urandom.
//!
//! We avoid the `rand`/`getrandom` crates and implement `rand_core`'s
//! `RngCore` + `CryptoRng` directly over a file handle to `/dev/urandom`.
//! This is the lightweight path the project depends on for RSA blinding.

use rand_core::impls::{next_u32_via_fill, next_u64_via_fill};
use rand_core::{CryptoRng, RngCore};
use std::fs::File;
use std::io::Read;

/// A cryptographically-secure RNG reading from `/dev/urandom`.
pub struct UrandomRng {
    file: File,
}

impl UrandomRng {
    /// Open `/dev/urandom`. Panics if the device cannot be opened.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            file: File::open("/dev/urandom")?,
        })
    }
}

impl RngCore for UrandomRng {
    fn next_u32(&mut self) -> u32 {
        next_u32_via_fill(self)
    }

    fn next_u64(&mut self) -> u64 {
        next_u64_via_fill(self)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.try_fill_bytes(dest)
            .expect("/dev/urandom read failed; the kernel RNG is unavailable")
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.file.read_exact(dest).map_err(rand_core::Error::new)
    }
}

impl CryptoRng for UrandomRng {}
