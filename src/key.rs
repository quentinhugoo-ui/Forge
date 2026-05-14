use sha2::{Digest, Sha256};

use crate::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallKey([u8; 32]);

impl CallKey {
    pub fn new(func: &Hash, args: &Hash) -> Self {
        Self::from_program_identity(func.as_bytes(), args)
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 64 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, slot) in bytes.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }

    /// Persistent identity used for the on-disk memo key. SHA-256 is
    /// kept here on purpose: `CallKey` shows up in git refs and we
    /// can't change its on-disk format without breaking every existing
    /// memo. Hot-path lookups go through `RamKey` instead and don't
    /// pay this cost.
    pub fn from_program_identity(program_identity: &[u8], args: &Hash) -> Self {
        let mut h = Sha256::new();
        h.update(b"kasm-call-v1\0");
        h.update(program_identity);
        h.update(args.as_bytes());
        Self(h.finalize().into())
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Hex-encode the 32-byte CallKey. Hot path : appelé sur chaque
    /// miss du slow path L5/L6 dans `dispatch_impl` (key_hex pour
    /// `lookup_memo` / `write_memo`).
    ///
    /// Coalescing : remplace 32 × `format!("{b:02x}")` (= 32 mini-allocs
    /// + 32 push_str ≈ 3 µs/call) par 1 alloc Vec[64] + table HEX
    /// statique + cast unsafe valid-utf8 (≈ 30 ns/call). Même pattern
    /// que les syscall-batching : N opérations identiques → 1
    /// allocation + boucle plate.
    pub fn hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = vec![0u8; 64];
        for (i, &b) in self.0.iter().enumerate() {
            out[i * 2] = HEX[(b >> 4) as usize];
            out[i * 2 + 1] = HEX[(b & 0x0f) as usize];
        }
        // SAFETY: `out` ne contient que des octets ASCII hex
        // (0-9 / a-f), produits par `HEX` qui est lui-même ASCII.
        // String::from_utf8_unchecked est valide par construction.
        unsafe { String::from_utf8_unchecked(out) }
    }
}
