use core::hash::{Hash, Hasher};

/// A non-cryptographic hasher for fast CSS class name generation.
/// Uses FNV-1a algorithm which is faster for small strings.
///
/// # Security
///
/// This is **not** a cryptographic hash function. It is susceptible to collision attacks
/// if used with untrusted input in a security-sensitive context. Only use it for
/// generating stable identifiers (like CSS class names) from trusted source code strings.
pub struct CssHasher(u64);

impl Default for CssHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl CssHasher {
    pub const fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for CssHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
    fn finish(&self) -> u64 {
        self.0
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
}
