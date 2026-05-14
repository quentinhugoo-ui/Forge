//! Inter-node memo exchange (wire encoding + I/O glue).
//!
//! Φ.μ.7 : fusion de l'ancien `src/swarm.rs` (types + encode/decode wire)
//! et `src/monster/swarm_io.rs` (impl `MonsterNode`). Les deux étaient
//! tightly coupled — la séparation top-level/sub-module n'apportait rien.
//!
//! Phase γ.X prévoit de remplacer ce wire format par mmap shared CAS,
//! auquel cas ce fichier est candidat à fusion dans `store.rs`.

use std::io;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::{pack_lossless, unpack_lossless, CallKey, Hash, MonsterStats};

use super::cache::RamKey;
use super::MonsterNode;

const SWARM_MAGIC: &[u8; 8] = b"SWARMv1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmPresence {
    pub node_id: Hash,
    pub ram_budget_bytes: usize,
    pub ram_used_bytes: usize,
    pub memo_cache_entries: usize,
    pub result_cache_entries: usize,
    pub stats: MonsterStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmKnowledgeFrame {
    pub origin: Hash,
    pub memos: Vec<SwarmMemo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmMemo {
    pub call_key: CallKey,
    pub result: Hash,
    pub result_bytes: Vec<u8>,
}

impl SwarmKnowledgeFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(SWARM_MAGIC);
        out.extend_from_slice(self.origin.as_bytes());
        out.extend_from_slice(&(self.memos.len() as u32).to_le_bytes());
        for memo in &self.memos {
            let packed = pack_lossless(&memo.result_bytes);
            out.extend_from_slice(&memo.call_key.as_bytes());
            out.extend_from_slice(memo.result.as_bytes());
            out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            out.extend_from_slice(&packed);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < SWARM_MAGIC.len() + 20 + 4 || &bytes[..SWARM_MAGIC.len()] != SWARM_MAGIC {
            return Err(io::Error::other("bad swarm frame"));
        }

        let mut cursor = SWARM_MAGIC.len();
        let origin = Hash::from_hex(&hex(&bytes[cursor..cursor + 20]))
            .ok_or_else(|| io::Error::other("bad swarm origin"))?;
        cursor += 20;
        let count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        let mut memos = Vec::with_capacity(count);
        for _ in 0..count {
            if cursor + 32 + 20 + 4 > bytes.len() {
                return Err(io::Error::other("truncated swarm memo"));
            }
            let call_key = CallKey::from_hex(&hex(&bytes[cursor..cursor + 32]))
                .ok_or_else(|| io::Error::other("bad swarm call key"))?;
            cursor += 32;
            let result = Hash::from_hex(&hex(&bytes[cursor..cursor + 20]))
                .ok_or_else(|| io::Error::other("bad swarm result hash"))?;
            cursor += 20;
            let packed_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + packed_len > bytes.len() {
                return Err(io::Error::other("truncated swarm payload"));
            }
            let result_bytes = unpack_lossless(&bytes[cursor..cursor + packed_len])
                .map_err(|err| io::Error::other(format!("swarm payload: {err}")))?;
            cursor += packed_len;
            memos.push(SwarmMemo { call_key, result, result_bytes });
        }

        if cursor != bytes.len() {
            return Err(io::Error::other("swarm frame has trailing bytes"));
        }

        Ok(Self { origin, memos })
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ----- I/O glue : impl MonsterNode -----

impl MonsterNode {
    pub fn swarm_presence(&self) -> SwarmPresence {
        SwarmPresence {
            node_id: self.node_id(),
            ram_budget_bytes: self.governor.budget_bytes(),
            ram_used_bytes: self.governor.used_bytes(),
            memo_cache_entries: self.memo_cache_len(),
            result_cache_entries: self.result_cache_len(),
            stats: self.stats(),
        }
    }

    pub fn export_swarm_frame(&self, limit: usize) -> io::Result<SwarmKnowledgeFrame> {
        let memos = self.recent_swarm_memos(limit)?;
        Ok(SwarmKnowledgeFrame {
            origin: self.node_id(),
            memos,
        })
    }

    pub fn import_swarm_frame(&self, frame: &SwarmKnowledgeFrame) -> io::Result<usize> {
        self.import_swarm_memos(&frame.memos)
    }

    pub fn sync_direct_from(&self, peer: &Self, limit: usize) -> io::Result<usize> {
        let memos = peer.recent_swarm_memos(limit)?;
        self.import_swarm_memos(&memos)
    }

    pub(super) fn node_id(&self) -> Hash {
        Hash::for_blob(self.store.path().to_string_lossy().as_bytes())
    }

    pub(super) fn recent_swarm_memos(&self, limit: usize) -> io::Result<Vec<SwarmMemo>> {
        let memo_heads = self.programs_recent_keys(limit);
        memo_heads
            .into_iter()
            .map(|(call_key, result)| {
                // Wave 9 — memo result hash referenced by the index
                // but absent from the CAS surfaces as NotFound. This
                // happens if a swarm peer pruned the result blob
                // after publishing the memo head — different signal
                // than a disk I/O fault on this node.
                let result_bytes = self.store().load(&result).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("unknown memo result: {result}"),
                    )
                })?;
                Ok(SwarmMemo {
                    call_key,
                    result,
                    result_bytes,
                })
            })
            .collect()
    }

    fn import_swarm_memos(&self, memos: &[SwarmMemo]) -> io::Result<usize> {
        let mut imported = 0usize;
        if !memos.is_empty() {
            self.wire_seen_ever.store(true, Ordering::Relaxed);
        }
        for memo in memos {
            let key_hex = memo.call_key.hex();
            let arc_bytes: Arc<[u8]> =
                Arc::from(memo.result_bytes.clone().into_boxed_slice());
            let ram_key = RamKey::from_call_key(&memo.call_key);
            if let Some(existing) = self.store().lookup_memo(&key_hex) {
                if existing != memo.result {
                    return Err(io::Error::other(format!(
                        "conflicting swarm memo for key {key_hex}"
                    )));
                }
                self.remember_call(ram_key, existing, arc_bytes);
                continue;
            }

            let stored = self.store().store(&memo.result_bytes)?;
            if stored != memo.result {
                return Err(io::Error::other(format!(
                    "swarm payload hash mismatch: expected {}, got {}",
                    memo.result, stored
                )));
            }
            self.store().write_memo(&key_hex, &stored)?;
            self.remember_call(ram_key, stored, arc_bytes);
            imported += 1;
        }
        Ok(imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swarm_frame_roundtrips_losslessly() {
        let origin = Hash::for_blob(b"node-a");
        let frame = SwarmKnowledgeFrame {
            origin,
            memos: vec![SwarmMemo {
                call_key: CallKey::from_hex(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )
                .unwrap(),
                result: Hash::for_blob(b"result"),
                result_bytes: vec![1, 2, 3, 3, 3, 4, 5, 6],
            }],
        };

        let wire = frame.encode();
        let decoded = SwarmKnowledgeFrame::decode(&wire).unwrap();
        assert_eq!(decoded, frame);
    }
}
