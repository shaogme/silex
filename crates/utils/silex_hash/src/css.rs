use core::hash::{Hash, Hasher};

const K: u64 = 0xf1357aea2e62a9c5;

#[inline]
fn multiply_mix(x: u64, y: u64) -> u64 {
    let full = (x as u128).wrapping_mul(y as u128);
    ((full >> 64) as u64) ^ (full as u64)
}

#[inline]
fn fast_hash_bytes(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    let mut s0 = 0x243f6a8885a308d3u64;
    let mut s1 = 0x13198a2e03707344u64;

    if len <= 16 {
        if len >= 8 {
            s0 ^= u64::from_le(unsafe { (bytes.as_ptr() as *const u64).read_unaligned() });
            s1 ^= u64::from_le(unsafe {
                (bytes.as_ptr().add(len - 8) as *const u64).read_unaligned()
            });
        } else if len >= 4 {
            s0 ^= u32::from_le(unsafe { (bytes.as_ptr() as *const u32).read_unaligned() }) as u64;
            s1 ^= u32::from_le(unsafe {
                (bytes.as_ptr().add(len - 4) as *const u32).read_unaligned()
            }) as u64;
        } else if len > 0 {
            let lo = bytes[0];
            let mid = bytes[len / 2];
            let hi = bytes[len - 1];
            s0 ^= lo as u64;
            s1 ^= ((hi as u64) << 8) | mid as u64;
        }
    } else {
        let mut bulk = &bytes[..(len - 1)];
        while bulk.len() >= 16 {
            let chunk = &bulk[..16];
            let x = u64::from_le(unsafe { (chunk.as_ptr() as *const u64).read_unaligned() });
            let y = u64::from_le(unsafe { (chunk.as_ptr().add(8) as *const u64).read_unaligned() });

            let t = multiply_mix(s0 ^ x, 0xa4093822299f31d0u64 ^ y);
            s0 = s1;
            s1 = t;
            bulk = &bulk[16..];
        }

        let suffix = &bytes[len - 16..];
        s0 ^= u64::from_le(unsafe { (suffix.as_ptr() as *const u64).read_unaligned() });
        s1 ^= u64::from_le(unsafe { (suffix.as_ptr().add(8) as *const u64).read_unaligned() });
    }

    multiply_mix(s0, s1) ^ (len as u64)
}

/// const 编译期计算源码位置字符串、行号、列号的 seed 哈希
const fn const_hash_location(file: &'static str, line: u32, column: u32) -> u64 {
    let bytes = file.as_bytes();
    let mut h = 0xcbf29ce484222325u64 ^ (line as u64) ^ ((column as u64) << 32);
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x100000001b3);
        i += 1;
    }
    h
}

/// A non-cryptographic hasher for fast CSS class name generation.
/// Uses `Fxhash` algorithm for high throughput.
///
/// # Security
///
/// This is **not** a cryptographic hash function. It is susceptible to collision attacks
/// if used with untrusted input in a security-sensitive context. Only use it for
/// generating stable identifiers (like CSS class names) from trusted source code strings.
#[derive(Debug, Clone)]
pub struct CssHasher {
    hash: u64,
}

impl Default for CssHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl CssHasher {
    #[inline]
    pub const fn new() -> Self {
        Self::with_seed(0)
    }

    #[inline]
    pub const fn with_seed(seed: u64) -> Self {
        Self { hash: seed }
    }

    /// 编译期基于 `file!()`、`line!()` 和 `column!()` 生成唯一 Seed 的构造函数
    #[inline]
    pub const fn new_compile_time(file: &'static str, line: u32, column: u32) -> Self {
        let seed = const_hash_location(file, line, column);
        Self::with_seed(seed)
    }

    #[inline]
    fn add_to_hash(&mut self, i: u64) {
        self.hash = self.hash.wrapping_add(i).wrapping_mul(K);
    }
}

/// 快捷宏：在调用点自动捕获 `file!()` / `line!()` / `column!()` 并构造带编译期 Seed 的 `CssHasher`
#[macro_export]
macro_rules! css_hasher {
    () => {
        $crate::css::CssHasher::new_compile_time(file!(), line!(), column!())
    };
}

impl Hasher for CssHasher {
    #[inline]
    fn finish(&self) -> u64 {
        const ROTATE: u32 = 26;
        self.hash.rotate_left(ROTATE)
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.write_u64(fast_hash_bytes(bytes));
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.add_to_hash(i as u64);
        self.add_to_hash((i >> 64) as u64);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }
}

/// A builder for [`CssHasher`].
#[derive(Default, Clone, Copy, Debug)]
pub struct CssBuildHasher;

impl core::hash::BuildHasher for CssBuildHasher {
    type Hasher = CssHasher;
    fn build_hasher(&self) -> Self::Hasher {
        CssHasher::new()
    }
}

/// A fast Base36 encoder for `u64` that doesn't require allocation.
///
/// Max length of a Base36 encoded `u64` is 13 characters.
pub fn encode_base36(mut n: u64, buf: &mut [u8; 13]) -> &str {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        buf[12] = b'0';
        return unsafe { core::str::from_utf8_unchecked(&buf[12..13]) };
    }
    let mut i = 13;
    while n > 0 {
        i -= 1;
        buf[i] = ALPHABET[(n % 36) as usize];
        n /= 36;
    }
    unsafe { core::str::from_utf8_unchecked(&buf[i..13]) }
}

/// Hashes a single value and returns the `u64` hash.
pub fn hash_one<H: Hash>(data: H) -> u64 {
    let mut hasher = CssHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Hashes a value and returns its Base36 encoded string.
#[cfg(feature = "alloc")]
pub fn hash_to_base36<H: Hash>(data: H) -> alloc::string::String {
    let hash = hash_one(data);
    let mut buf = [0u8; 13];
    encode_base36(hash, &mut buf).into()
}

/// Hashes a value and returns an ID string with the given prefix.
#[cfg(feature = "alloc")]
pub fn hash_to_id<H: Hash>(prefix: &str, data: H) -> alloc::string::String {
    let hash = hash_one(data);
    let mut buf = [0u8; 13];
    let mut s = alloc::string::String::with_capacity(prefix.len() + 13);
    s.push_str(prefix);
    s.push_str(encode_base36(hash, &mut buf));
    s
}

/// A wrapper for CSS strings that hashes while normalizing whitespaces.
///
/// It collapses multiple whitespaces into one and ignores whitespaces around
/// common CSS delimiters like `:`, `;`, `{`, `}`, `,`.
pub struct Normalized<'a>(pub &'a str);

impl Hash for Normalized<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut last_was_whitespace = false;
        let mut last_was_symbol = true; // Treat start of string as a symbol to skip leading spaces

        for b in self.0.bytes() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {
                    last_was_whitespace = true;
                }
                b':' | b';' | b'{' | b'}' | b',' => {
                    // Delimiters: discard any pending whitespace
                    state.write_u8(b);
                    last_was_whitespace = false;
                    last_was_symbol = true;
                }
                _ => {
                    if last_was_whitespace && !last_was_symbol {
                        // Internal whitespace: collapse to a single space
                        state.write_u8(b' ');
                    }
                    state.write_u8(b);
                    last_was_whitespace = false;
                    last_was_symbol = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collision_basics() {
        let r1 = hash_one("slx-test-1");
        let r2 = hash_one("slx-test-1");
        assert_eq!(r1, r2);

        let r3 = hash_one("slx-test-2");
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_css_shorthands() {
        let strings = [
            "display: block;",
            "display: flex;",
            "color: red;",
            "color: blue;",
            "margin: 10px;",
            "padding: 10px;",
            "width: 100%;",
            "height: 100%;",
            "--theme-primary: #fff;",
            "--theme-secondary: #000;",
        ];

        for i in 0..strings.len() {
            for j in (i + 1)..strings.len() {
                assert_ne!(
                    hash_one(strings[i]),
                    hash_one(strings[j]),
                    "Collision between '{}' and '{}'",
                    strings[i],
                    strings[j]
                );
            }
        }
    }

    #[test]
    fn test_base36_encoding() {
        let mut buf = [0u8; 13];
        assert_eq!(encode_base36(0, &mut buf), "0");
        assert_eq!(encode_base36(10, &mut buf), "a");
        assert_eq!(encode_base36(35, &mut buf), "z");
        assert_eq!(encode_base36(36, &mut buf), "10");
        assert_eq!(encode_base36(u64::MAX, &mut buf), "3w5e11264sgsf");
    }

    #[test]
    fn test_ergonomics() {
        let h = hash_one("test");
        assert_ne!(h, 0);

        #[cfg(feature = "alloc")]
        {
            let s = hash_to_base36("test");
            assert!(!s.is_empty());

            let id = hash_to_id("slx-", "test");
            assert!(id.starts_with("slx-"));
            assert_eq!(id.len(), 4 + s.len());
        }
    }

    #[test]
    fn test_normalized_hashing() {
        let s1 = "display: flex;";
        let s2 = "  display :  flex ; ";
        let s3 = "display:flex;";

        assert_eq!(hash_one(Normalized(s1)), hash_one(Normalized(s2)));
        assert_eq!(hash_one(Normalized(s1)), hash_one(Normalized(s3)));

        // Verify multi-word properties
        let p1 = "margin: 10px 20px;";
        let p2 = "margin:10px   20px;"; // Internal spaces should collapse to one, but not zero
        assert_eq!(hash_one(Normalized(p1)), hash_one(Normalized(p2)));

        let p3 = "margin: 10px20px;"; // This is semantically different
        assert_ne!(hash_one(Normalized(p1)), hash_one(Normalized(p3)));
    }

    #[test]
    fn test_compile_time_seed() {
        let (mut h1, mut h2) = (css_hasher!(), css_hasher!());
        h1.write(b"abc");
        h2.write(b"abc");
        // 如果在同一行同一列展开或指定相同的 location 参数，Seed 相同，哈希结果一致
        let mut h3 = CssHasher::new_compile_time("test.rs", 10, 5);
        let mut h4 = CssHasher::new_compile_time("test.rs", 10, 5);
        h3.write(b"abc");
        h4.write(b"abc");
        assert_eq!(h3.finish(), h4.finish());

        // 不同行或不同列的 Seed 计算结果不同
        let h_a = CssHasher::new_compile_time("test.rs", 10, 5);
        let h_b = CssHasher::new_compile_time("test.rs", 10, 6);
        assert_ne!(h_a.finish(), h_b.finish());
    }
}
