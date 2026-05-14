//! Properties of the git-backed substrate, proved end-to-end.

use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use scan::{Hash, Store};

fn fresh_store(tag: &str) -> (Store, std::path::PathBuf) {
    let mut tmp = std::env::current_dir().unwrap();
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    tmp.push(".codex-tmp");
    tmp.push(format!("scan-test-{tag}-{nanos}"));
    let store = Store::open(&tmp).expect("open");
    (store, tmp)
}

fn cleanup(p: &std::path::Path) {
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn identical_bytes_dedupe() {
    let (store, root) = fresh_store("dedupe");
    let h1 = store.store(b"hello").unwrap();
    let h2 = store.store(b"hello").unwrap();
    assert_eq!(h1, h2);
    cleanup(&root);
}

#[test]
fn different_bytes_distinct_hashes() {
    let (store, root) = fresh_store("distinct");
    let h1 = store.store(b"hello").unwrap();
    let h2 = store.store(b"world").unwrap();
    assert_ne!(h1, h2);
    cleanup(&root);
}

#[test]
fn round_trip_load_returns_exact_bytes() {
    let (store, root) = fresh_store("roundtrip");
    let payload = (0u8..=255u8).collect::<Vec<u8>>();
    let h = store.store(&payload).unwrap();
    let back = store.load(&h).expect("known hash");
    assert_eq!(payload, back);
    cleanup(&root);
}

#[test]
fn unknown_hash_loads_to_none() {
    let (store, root) = fresh_store("unknown");
    let phantom = Hash::from_hex("0000000000000000000000000000000000000000").unwrap();
    assert!(store.load(&phantom).is_none());
    cleanup(&root);
}

#[test]
fn parallel_stores_of_same_content_dedupe() {
    let (store, root) = fresh_store("parallel-same");
    let store = Arc::new(store);
    let payload: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let store = Arc::clone(&store);
            let payload = payload.clone();
            thread::spawn(move || store.store(&payload).unwrap())
        })
        .collect();

    let hashes: Vec<Hash> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let first = hashes[0].clone();
    assert!(hashes.iter().all(|h| *h == first));
    cleanup(&root);
}

#[test]
fn parallel_distinct_payloads_all_persist() {
    let (store, root) = fresh_store("parallel-distinct");
    let store = Arc::new(store);

    let handles: Vec<_> = (0..32u32)
        .map(|i| {
            let store = Arc::clone(&store);
            thread::spawn(move || {
                let payload = format!("payload number {i}");
                store.store(payload.as_bytes()).unwrap()
            })
        })
        .collect();

    let hashes: Vec<Hash> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for h in &hashes {
        assert!(store.load(h).is_some(), "all 32 distinct payloads must be present");
    }
    cleanup(&root);
}

#[test]
fn store_survives_handle_drop() {
    let (store, root) = fresh_store("survives");
    let h = store.store(b"persistent").unwrap();
    drop(store);

    let store2 = Store::open(&root).unwrap();
    assert_eq!(store2.load(&h).unwrap(), b"persistent");
    cleanup(&root);
}

#[test]
fn user_space_composition_is_just_bytes() {
    let (store, root) = fresh_store("compose");
    let a = store.store(b"alpha").unwrap();
    let b = store.store(b"beta").unwrap();

    let pair = format!("pair\n{}\n{}\n", a.as_hex(), b.as_hex());
    let h_pair = store.store(pair.as_bytes()).unwrap();
    let h_pair_again = store.store(pair.as_bytes()).unwrap();
    assert_eq!(h_pair, h_pair_again);

    let pair_rev = format!("pair\n{}\n{}\n", b.as_hex(), a.as_hex());
    let h_rev = store.store(pair_rev.as_bytes()).unwrap();
    assert_ne!(h_pair, h_rev);
    cleanup(&root);
}
