//! Deterministic Perlin noise (2D/3D), fBm layering and a small PRNG.

#[derive(Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

impl Perlin {
    pub fn new(seed: u64) -> Perlin {
        let mut p: [u8; 256] = [0; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        let mut rng = Rng::new(seed);
        for i in (1..256).rev() {
            let j = (rng.next_u32() % (i as u32 + 1)) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Perlin { perm }
    }

    #[inline]
    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }
    #[inline]
    fn lerp(a: f64, b: f64, t: f64) -> f64 {
        a + t * (b - a)
    }
    #[inline]
    fn grad2(h: u8, x: f64, y: f64) -> f64 {
        match h & 7 {
            0 => x + y,
            1 => -x + y,
            2 => x - y,
            3 => -x - y,
            4 => x,
            5 => -x,
            6 => y,
            _ => -y,
        }
    }
    #[inline]
    fn grad3(h: u8, x: f64, y: f64, z: f64) -> f64 {
        match h & 15 {
            0 => x + y,
            1 => -x + y,
            2 => x - y,
            3 => -x - y,
            4 => x + z,
            5 => -x + z,
            6 => x - z,
            7 => -x - z,
            8 => y + z,
            9 => -y + z,
            10 => y - z,
            11 => -y - z,
            12 => y + x,
            13 => -y + z,
            14 => y - x,
            _ => -y - z,
        }
    }

    /// 2D Perlin noise in roughly [-1, 1].
    pub fn get2(&self, x: f64, y: f64) -> f64 {
        let xi = x.floor();
        let yi = y.floor();
        let xf = x - xi;
        let yf = y - yi;
        let xi = (xi as i64 & 255) as usize;
        let yi = (yi as i64 & 255) as usize;
        let u = Self::fade(xf);
        let v = Self::fade(yf);
        let p = &self.perm;
        let aa = p[p[xi] as usize + yi];
        let ab = p[p[xi] as usize + yi + 1];
        let ba = p[p[xi + 1] as usize + yi];
        let bb = p[p[xi + 1] as usize + yi + 1];
        let x1 = Self::lerp(Self::grad2(aa, xf, yf), Self::grad2(ba, xf - 1.0, yf), u);
        let x2 = Self::lerp(Self::grad2(ab, xf, yf - 1.0), Self::grad2(bb, xf - 1.0, yf - 1.0), u);
        Self::lerp(x1, x2, v) * 1.41
    }

    /// 3D Perlin noise in roughly [-1, 1].
    pub fn get3(&self, x: f64, y: f64, z: f64) -> f64 {
        let xi0 = x.floor();
        let yi0 = y.floor();
        let zi0 = z.floor();
        let xf = x - xi0;
        let yf = y - yi0;
        let zf = z - zi0;
        let xi = (xi0 as i64 & 255) as usize;
        let yi = (yi0 as i64 & 255) as usize;
        let zi = (zi0 as i64 & 255) as usize;
        let u = Self::fade(xf);
        let v = Self::fade(yf);
        let w = Self::fade(zf);
        let p = &self.perm;
        let a = p[xi] as usize + yi;
        let aa = p[a] as usize + zi;
        let ab = p[a + 1] as usize + zi;
        let b = p[xi + 1] as usize + yi;
        let ba = p[b] as usize + zi;
        let bb = p[b + 1] as usize + zi;
        let g = Self::grad3;
        let x1 = Self::lerp(g(p[aa], xf, yf, zf), g(p[ba], xf - 1.0, yf, zf), u);
        let x2 = Self::lerp(g(p[ab], xf, yf - 1.0, zf), g(p[bb], xf - 1.0, yf - 1.0, zf), u);
        let y1 = Self::lerp(x1, x2, v);
        let x3 = Self::lerp(g(p[aa + 1], xf, yf, zf - 1.0), g(p[ba + 1], xf - 1.0, yf, zf - 1.0), u);
        let x4 = Self::lerp(g(p[ab + 1], xf, yf - 1.0, zf - 1.0), g(p[bb + 1], xf - 1.0, yf - 1.0, zf - 1.0), u);
        let y2 = Self::lerp(x3, x4, v);
        Self::lerp(y1, y2, w)
    }

    /// Fractal Brownian motion, 2D. Output roughly in [-1, 1].
    pub fn fbm2(&self, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += self.get2(x * freq, y * freq) * amp;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }

    pub fn fbm3(&self, x: f64, y: f64, z: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += self.get3(x * freq, y * freq, z * freq) * amp;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }

    /// Ridged noise in [0, 1]: 1 - |n|, sharp ridges. Used for rivers and mountains.
    pub fn ridge2(&self, x: f64, y: f64, octaves: u32) -> f64 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            let n = 1.0 - self.get2(x * freq, y * freq).abs();
            sum += n * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

/// SplitMix64-seeded xorshift PRNG. Deterministic and cheap.
#[derive(Clone)]
pub struct Rng {
    s: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut r = Rng { s: seed ^ 0x9E3779B97F4A7C15 };
        // warm up with splitmix so nearby seeds diverge
        r.s = splitmix(r.s);
        if r.s == 0 {
            r.s = 0x1234_5678_9ABC_DEF1;
        }
        r
    }
    /// Deterministic per-position RNG (chunk decoration etc.).
    pub fn at(seed: u64, x: i64, z: i64, salt: u64) -> Rng {
        let h = splitmix(seed ^ splitmix((x as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ splitmix((z as u64).wrapping_mul(0xC2B2AE3D27D4EB4F) ^ salt)));
        Rng::new(h)
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.s = x;
        x
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    /// Uniform float in [0, 1).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16777216.0
    }
    #[inline]
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / 9007199254740992.0
    }
    /// Uniform integer in [0, n).
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        ((self.next_u64() >> 32) * n as u64 >> 32) as u32
    }
    #[inline]
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo) as u32) as i32
    }
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}

#[inline]
pub fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Hash a string seed the way the menu does ("123" parses as a number, anything else is hashed).
pub fn seed_from_str(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return splitmix(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42));
    }
    if let Ok(n) = s.parse::<i64>() {
        return n as u64;
    }
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_is_deterministic_and_bounded() {
        let a = Perlin::new(1234);
        let b = Perlin::new(1234);
        let c = Perlin::new(1235);
        let mut differs = false;
        for i in 0..200 {
            let x = i as f64 * 0.37;
            let y = i as f64 * 0.11 + 3.0;
            let va = a.get2(x, y);
            assert_eq!(va, b.get2(x, y));
            assert!(va >= -1.5 && va <= 1.5);
            if (va - c.get2(x, y)).abs() > 1e-9 {
                differs = true;
            }
            let v3 = a.get3(x, y, i as f64 * 0.21);
            assert_eq!(v3, b.get3(x, y, i as f64 * 0.21));
            assert!(v3 >= -1.5 && v3 <= 1.5);
        }
        assert!(differs, "different seeds must give different noise");
    }

    #[test]
    fn fbm_stays_in_range() {
        let p = Perlin::new(7);
        for i in 0..500 {
            let v = p.fbm2(i as f64 * 0.013, i as f64 * 0.029, 6, 2.0, 0.5);
            assert!(v.abs() <= 1.0 + 1e-6);
            let r = p.ridge2(i as f64 * 0.02, 5.0, 4);
            assert!(r >= 0.0 && r <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn positional_rng_is_deterministic() {
        let mut a = Rng::at(99, 10, -5, 1);
        let mut b = Rng::at(99, 10, -5, 1);
        let mut c = Rng::at(99, 11, -5, 1);
        let va: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let vb: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        let vc: Vec<u32> = (0..8).map(|_| c.next_u32()).collect();
        assert_eq!(va, vb);
        assert_ne!(va, vc);
        let mut r = Rng::new(5);
        for _ in 0..1000 {
            assert!(r.below(10) < 10);
            let f = r.f32();
            assert!(f >= 0.0 && f < 1.0);
        }
    }

    #[test]
    fn seed_parsing() {
        assert_eq!(seed_from_str("42"), 42);
        assert_eq!(seed_from_str("-1"), u64::MAX);
        assert_eq!(seed_from_str("hello"), seed_from_str("hello"));
        assert_ne!(seed_from_str("hello"), seed_from_str("hellp"));
    }
}
