use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908b0df;
const UPPER_MASK: u32 = 0x80000000;
const LOWER_MASK: u32 = 0x7fffffff;

/// Bit-exact implementation of Python's `random.Random` (MT19937).
/// Ensures 100% deterministic shuffle, seed, and `rng_sha256` parity with Wonderland 0.2/0.3/0.4.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PyRng {
    pub mt: Vec<u32>,
    pub index: usize,
}

impl Default for PyRng {
    fn default() -> Self {
        Self::new(0)
    }
}

impl PyRng {
    pub fn new(seed: u64) -> Self {
        let mut rng = Self {
            mt: vec![0; N],
            index: N + 1,
        };
        rng.seed_u64(seed);
        rng
    }

    pub fn seed_i64(&mut self, seed: i64) {
        let abs = seed.unsigned_abs();
        self.seed_u64(abs);
    }

    pub fn seed_u64(&mut self, seed: u64) {
        let mut key = Vec::new();
        let mut val = seed;
        while val > 0 {
            key.push((val & 0xffffffff) as u32);
            val >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        self.init_by_array(&key);
    }

    pub fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for mti in 1..N {
            self.mt[mti] = 1812433253u32
                .wrapping_mul(self.mt[mti - 1] ^ (self.mt[mti - 1] >> 30))
                .wrapping_add(mti as u32);
        }
        self.index = N;
    }

    pub fn init_by_array(&mut self, init_key: &[u32]) {
        self.init_genrand(19650218u32);
        let mut i: usize = 1;
        let mut j: usize = 0;
        let key_length = init_key.len();
        let mut k = if N > key_length { N } else { key_length };
        while k > 0 {
            self.mt[i] = (self.mt[i]
                ^ ((self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_mul(1664525u32)))
            .wrapping_add(init_key[j])
            .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key_length {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            self.mt[i] = (self.mt[i]
                ^ ((self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_mul(1566083941u32)))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        self.mt[0] = 0x80000000u32;
    }

    pub fn genrand_uint32(&mut self) -> u32 {
        let mag01 = [0u32, MATRIX_A];
        if self.index >= N {
            for kk in 0..(N - M) {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            }
            for kk in (N - M)..(N - 1) {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk.wrapping_add(M).wrapping_sub(N)]
                    ^ (y >> 1)
                    ^ mag01[(y & 1) as usize];
            }
            let y = (self.mt[N - 1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680u32;
        y ^= (y << 15) & 0xefc60000u32;
        y ^= y >> 18;
        y
    }

    pub fn getrandbits(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        let r = self.genrand_uint32();
        r >> (32 - k)
    }

    pub fn randbelow(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        let k = usize::BITS - n.leading_zeros();
        loop {
            let r = self.getrandbits(k) as usize;
            if r < n {
                return r;
            }
        }
    }

    pub fn shuffle<T>(&mut self, x: &mut [T]) {
        for i in (1..x.len()).rev() {
            let j = self.randbelow(i + 1);
            x.swap(i, j);
        }
    }

    /// Formats the RNG state identically to Python's `json.dumps(rng.getstate())`:
    /// `[3, [mt0, mt1, ..., mt623, index], null]` with standard space-after-comma formatting.
    pub fn to_python_json_string(&self) -> String {
        let mut s = String::with_capacity(6800);
        s.push_str("[3, [");
        for v in &self.mt {
            s.push_str(&v.to_string());
            s.push_str(", ");
        }
        s.push_str(&self.index.to_string());
        s.push_str("], null]");
        s
    }

    /// Exact SHA-256 digest matching `transition_support.rng_sha256(g)`.
    pub fn rng_sha256(&self) -> String {
        let json_bytes = self.to_python_json_string();
        let mut hasher = Sha256::new();
        hasher.update(json_bytes.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Exports state as `[3, [mt..., index], null]` `serde_json::Value` for `wonderland-state-1/2`.
    pub fn getstate_json(&self) -> serde_json::Value {
        let mut inner: Vec<serde_json::Value> = self
            .mt
            .iter()
            .map(|&x| serde_json::Value::from(x))
            .collect();
        inner.push(serde_json::Value::from(self.index as u32));
        serde_json::json!([3, inner, serde_json::Value::Null])
    }

    /// Restores state from Python's `[3, [mt..., index], null]` JSON representation.
    pub fn setstate_json(&mut self, val: &serde_json::Value) -> Result<(), String> {
        let arr = val
            .as_array()
            .ok_or_else(|| "rng_state must be a list/tuple of length 3".to_string())?;
        if arr.len() != 3 {
            return Err("rng_state must have 3 elements".to_string());
        }
        if arr[0].as_u64() != Some(3) {
            return Err("rng_state version must be 3".to_string());
        }
        if !arr[2].is_null() {
            return Err("rng_state gauss_next must be null".to_string());
        }
        let internal = arr[1]
            .as_array()
            .ok_or_else(|| "rng_state[1] must be an array of 625 integers".to_string())?;
        if internal.len() != N + 1 {
            return Err(format!("rng_state[1] must have {} ints", N + 1));
        }
        let mut new_mt = Vec::with_capacity(N);
        for item in &internal[0..N] {
            let v = item
                .as_u64()
                .ok_or_else(|| "mt element must be u32".to_string())?;
            if v > u32::MAX as u64 {
                return Err("mt element out of u32 range".to_string());
            }
            new_mt.push(v as u32);
        }
        let idx = internal[N]
            .as_u64()
            .ok_or_else(|| "mt index must be integer".to_string())? as usize;
        if idx > N {
            return Err("mt index out of bounds".to_string());
        }
        self.mt = new_mt;
        self.index = idx;
        Ok(())
    }
}
