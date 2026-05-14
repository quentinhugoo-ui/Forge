use std::fmt;

const MAGIC: &[u8; 4] = b"SCZ1";
const RAW: u8 = 0;
const RLE: u8 = 1;
const I64_DELTA: u8 = 2;

// ----- Nanocube polynomial recipe (NCB1) -----
// Format binaire compact pour séries i64 polynomiales (degré ≤ 3) :
// au lieu de stocker N×8 octets, on stocke (degree, 4×i64 coeffs,
// 3 témoins [0, mid, last]) = 60 octets, indépendant de N.
// Validation par évaluation directe sur les indices témoins.
const NANOCUBE_MAGIC: &[u8; 4] = b"NCB1";
const NANOCUBE_KIND_POLY_I64: u8 = 1;
const NANOCUBE_MAX_POLY_DEGREE: usize = 3;
const NANOCUBE_WITNESSES: usize = 3;
const NANOCUBE_POLY_LEN: usize = 4 + 1 + 1 + 2 + 8 + (4 * 8) + (NANOCUBE_WITNESSES * 16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    BadMagic,
    BadLength,
    BadMethod(u8),
    Truncated,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::BadMagic => write!(f, "bad SCAN codec magic"),
            CodecError::BadLength => write!(f, "bad SCAN codec length"),
            CodecError::BadMethod(method) => write!(f, "bad SCAN codec method {method}"),
            CodecError::Truncated => write!(f, "truncated SCAN codec payload"),
        }
    }
}

impl std::error::Error for CodecError {}

/// Tente d'encoder une série i64 comme un polynôme degré ≤ 3.
/// Retourne `None` si aucun polynôme ne fitte parfaitement la série
/// (auquel cas il faut retomber sur `pack_lossless`).
///
/// Format binaire (NCB1, 60 octets, indépendant de la longueur) :
/// `magic(4) | kind(1) | degree(1) | _pad(2) | len(8) | coeffs(4*8) | witnesses(3*16)`.
/// Chaque témoin est un couple `(index_u64, valeur_i64)` ré-évalué au
/// décodage : toute corruption d'un coefficient est détectée.
pub fn nanocube_pack_recipe_i64(outputs: &[i64]) -> Option<Vec<u8>> {
    if outputs.is_empty() {
        return None;
    }
    let recipe = fit_i64_poly_recipe(outputs)?;
    Some(encode_i64_poly_recipe(outputs, &recipe))
}

/// Décode un recipe NCB1 et reconstruit la série complète.
/// Vérifie l'en-tête, la longueur attendue, et les 3 témoins
/// [0, mid, last] avant d'évaluer le polynôme.
pub fn nanocube_unpack_recipe_i64(
    bytes: &[u8],
    expected_len: usize,
) -> Result<Vec<i64>, CodecError> {
    decode_i64_poly_recipe(bytes, expected_len)
}

pub fn pack_lossless(bytes: &[u8]) -> Vec<u8> {
    // Φ.μ.7.4 — court-circuit : pour les petits buffers (<32 B), le
    // header (13 B) + tentatives encoding RLE/delta dominent toujours
    // sur le raw. Skip directement.
    if bytes.len() < 32 {
        return frame(RAW, bytes);
    }

    let raw = frame(RAW, bytes);
    let mut best = raw;

    // Cheap pre-check : si les 16 premiers octets sont tous distincts,
    // RLE ne peut pas gagner de place (chaque byte = 3 octets en RLE).
    // Évite l'alloc + scan complet pour data haute-entropie.
    let rle_might_help = bytes.len() >= 16 && {
        let head = &bytes[..16];
        let mut seen = [false; 256];
        let mut distinct = 0u32;
        for b in head {
            if !seen[*b as usize] {
                seen[*b as usize] = true;
                distinct += 1;
            }
        }
        // Si <12 bytes distincts en prefix de 16 → run-friendly probable.
        distinct < 12
    };

    if rle_might_help {
        let rle = frame(RLE, &encode_rle(bytes));
        if rle.len() < best.len() {
            best = rle;
        }
    }

    // i64_delta nécessite au moins 16 octets (2 i64). Skip plus tôt
    // qu'avant pour économiser le check %8 + alloc.
    if bytes.len() >= 16 && bytes.len() % 8 == 0 {
        if let Some(delta) = encode_i64_delta(bytes) {
            let delta = frame(I64_DELTA, &delta);
            if delta.len() < best.len() {
                best = delta;
            }
        }
    }

    best
}

pub fn unpack_lossless(bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    if bytes.len() < 13 {
        return Err(CodecError::Truncated);
    }
    if &bytes[..4] != MAGIC {
        return Err(CodecError::BadMagic);
    }
    let method = bytes[4];
    let original_len = u64::from_le_bytes(bytes[5..13].try_into().unwrap()) as usize;
    let payload = &bytes[13..];

    let out = match method {
        RAW => payload.to_vec(),
        RLE => decode_rle(payload, original_len)?,
        I64_DELTA => decode_i64_delta(payload, original_len)?,
        other => return Err(CodecError::BadMethod(other)),
    };
    if out.len() == original_len {
        Ok(out)
    } else {
        Err(CodecError::BadLength)
    }
}

fn frame(method: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(13 + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(method);
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    if method != RAW {
        out[5..13].copy_from_slice(&original_len_for_payload(method, payload).unwrap_or(payload.len() as u64).to_le_bytes());
    }
    out.extend_from_slice(payload);
    out
}

fn original_len_for_payload(method: u8, payload: &[u8]) -> Option<u64> {
    match method {
        RAW => Some(payload.len() as u64),
        RLE => {
            let mut len = 0u64;
            let mut i = 0;
            while i + 3 <= payload.len() {
                len += u16::from_le_bytes([payload[i], payload[i + 1]]) as u64;
                i += 3;
            }
            Some(len)
        }
        I64_DELTA => payload.get(0..8).map(|_| {
            let mut count = 8u64;
            let mut i = 8;
            while i < payload.len() {
                count += 8;
                while i < payload.len() {
                    let b = payload[i];
                    i += 1;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
            }
            count
        }),
        _ => None,
    }
}

fn encode_rle(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        let mut run = 1usize;
        while i + run < bytes.len() && bytes[i + run] == byte && run < u16::MAX as usize {
            run += 1;
        }
        out.extend_from_slice(&(run as u16).to_le_bytes());
        out.push(byte);
        i += run;
    }
    out
}

fn decode_rle(bytes: &[u8], original_len: usize) -> Result<Vec<u8>, CodecError> {
    if bytes.len() % 3 != 0 {
        return Err(CodecError::Truncated);
    }
    let mut out = Vec::with_capacity(original_len);
    for chunk in bytes.chunks_exact(3) {
        let run = u16::from_le_bytes([chunk[0], chunk[1]]) as usize;
        out.resize(out.len() + run, chunk[2]);
    }
    Ok(out)
}

fn encode_i64_delta(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() || bytes.len() % 8 != 0 {
        return None;
    }
    let mut values = bytes
        .chunks_exact(8)
        .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()));
    let first = values.next()?;
    let mut out = first.to_le_bytes().to_vec();
    let mut prev = first;
    for value in values {
        let delta = value.wrapping_sub(prev);
        write_varint(zigzag(delta), &mut out);
        prev = value;
    }
    Some(out)
}

fn decode_i64_delta(bytes: &[u8], original_len: usize) -> Result<Vec<u8>, CodecError> {
    if original_len == 0 || original_len % 8 != 0 || bytes.len() < 8 {
        return Err(CodecError::BadLength);
    }
    let mut out = Vec::with_capacity(original_len);
    let mut prev = i64::from_le_bytes(bytes[0..8].try_into().unwrap());
    out.extend_from_slice(&prev.to_le_bytes());
    let mut i = 8;
    while out.len() < original_len {
        let value = read_varint(bytes, &mut i)?;
        prev = prev.wrapping_add(unzigzag(value));
        out.extend_from_slice(&prev.to_le_bytes());
    }
    if i != bytes.len() {
        return Err(CodecError::BadLength);
    }
    Ok(out)
}

fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn unzigzag(value: u64) -> i64 {
    ((value >> 1) as i64) ^ (-((value & 1) as i64))
}

fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint(bytes: &[u8], i: &mut usize) -> Result<u64, CodecError> {
    let mut out = 0u64;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(*i).ok_or(CodecError::Truncated)?;
        *i += 1;
        out |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(out);
        }
        shift += 7;
        if shift > 63 {
            return Err(CodecError::BadLength);
        }
    }
}

// ----- nanocube poly recipe internals -----

#[derive(Clone, Copy)]
struct PolyRecipe {
    degree: u8,
    coeffs: [i64; 4],
}

fn fit_i64_poly_recipe(outputs: &[i64]) -> Option<PolyRecipe> {
    let mut coeffs = [0i64; 4];
    let mut diffs: Vec<i128> = outputs.iter().map(|v| *v as i128).collect();
    for degree in 0..=NANOCUBE_MAX_POLY_DEGREE {
        let first = *diffs.first()?;
        coeffs[degree] = i64::try_from(first).ok()?;
        if validates_poly(outputs, degree, &coeffs) {
            return Some(PolyRecipe {
                degree: degree as u8,
                coeffs,
            });
        }
        if diffs.len() < 2 {
            break;
        }
        diffs = diffs.windows(2).map(|p| p[1] - p[0]).collect();
    }
    None
}

fn validates_poly(outputs: &[i64], degree: usize, coeffs: &[i64; 4]) -> bool {
    outputs
        .iter()
        .enumerate()
        .all(|(idx, want)| eval_poly_i64(idx, degree, coeffs) == Some(*want))
}

fn eval_poly_i64(index: usize, degree: usize, coeffs: &[i64; 4]) -> Option<i64> {
    let n = index as i128;
    let mut value = coeffs[0] as i128;
    if degree >= 1 {
        value += n * coeffs[1] as i128;
    }
    if degree >= 2 {
        value += (n * (n - 1) / 2) * coeffs[2] as i128;
    }
    if degree >= 3 {
        value += (n * (n - 1) * (n - 2) / 6) * coeffs[3] as i128;
    }
    i64::try_from(value).ok()
}

fn nanocube_witness_indexes(len: usize) -> [usize; NANOCUBE_WITNESSES] {
    [0, len / 2, len.saturating_sub(1)]
}

fn encode_i64_poly_recipe(outputs: &[i64], poly: &PolyRecipe) -> Vec<u8> {
    let mut out = Vec::with_capacity(NANOCUBE_POLY_LEN);
    out.extend_from_slice(NANOCUBE_MAGIC);
    out.push(NANOCUBE_KIND_POLY_I64);
    out.push(poly.degree);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(outputs.len() as u64).to_le_bytes());
    for c in poly.coeffs {
        out.extend_from_slice(&c.to_le_bytes());
    }
    for idx in nanocube_witness_indexes(outputs.len()) {
        out.extend_from_slice(&(idx as u64).to_le_bytes());
        out.extend_from_slice(&outputs[idx].to_le_bytes());
    }
    out
}

fn decode_i64_poly_recipe(bytes: &[u8], expected_len: usize) -> Result<Vec<i64>, CodecError> {
    if bytes.len() != NANOCUBE_POLY_LEN {
        return Err(CodecError::BadLength);
    }
    if &bytes[..4] != NANOCUBE_MAGIC {
        return Err(CodecError::BadMagic);
    }
    if bytes[4] != NANOCUBE_KIND_POLY_I64 {
        return Err(CodecError::BadMethod(bytes[4]));
    }
    let degree = bytes[5] as usize;
    if degree > NANOCUBE_MAX_POLY_DEGREE {
        return Err(CodecError::BadLength);
    }
    let len = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    if len != expected_len {
        return Err(CodecError::BadLength);
    }
    let mut coeffs = [0i64; 4];
    let mut cursor = 16;
    for c in &mut coeffs {
        *c = i64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;
    }
    // Vérification des 3 témoins avant évaluation : toute corruption
    // d'un coefficient sera révélée par un mismatch ici.
    for _ in 0..NANOCUBE_WITNESSES {
        let idx = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap()) as usize;
        let val = i64::from_le_bytes(bytes[cursor + 8..cursor + 16].try_into().unwrap());
        cursor += 16;
        if idx >= len {
            return Err(CodecError::BadLength);
        }
        let want = eval_poly_i64(idx, degree, &coeffs).ok_or(CodecError::BadLength)?;
        if want != val {
            return Err(CodecError::BadLength);
        }
    }
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(eval_poly_i64(i, degree, &coeffs).ok_or(CodecError::BadLength)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_roundtrip_is_lossless() {
        let data = b"no magic corruption allowed";
        assert_eq!(unpack_lossless(&pack_lossless(data)).unwrap(), data);
    }

    #[test]
    fn rle_crushes_repeated_bytes() {
        let data = vec![7u8; 4096];
        let packed = pack_lossless(&data);
        assert!(packed.len() < 64);
        assert_eq!(unpack_lossless(&packed).unwrap(), data);
    }

    #[test]
    fn i64_delta_crushes_monotonic_training_traces() {
        let mut data = Vec::new();
        for i in 0..1024i64 {
            data.extend_from_slice(&(i * 3).to_le_bytes());
        }
        let packed = pack_lossless(&data);
        assert!(packed.len() < data.len() / 4);
        assert_eq!(unpack_lossless(&packed).unwrap(), data);
    }

    #[test]
    fn nanocube_recipe_fits_cubic_and_rejects_chaos() {
        let cubic: Vec<i64> = (0..2048)
            .map(|x| {
                let x = x as i64;
                x * x * x - 7 * x + 11
            })
            .collect();
        let packed = nanocube_pack_recipe_i64(&cubic).unwrap();
        // 60 octets quel que soit la longueur de la série.
        assert_eq!(packed.len(), NANOCUBE_POLY_LEN);
        let decoded = nanocube_unpack_recipe_i64(&packed, cubic.len()).unwrap();
        assert_eq!(decoded, cubic);

        // Chaos non-polynomial : pas de fit.
        let mut state = 0x1234_5678_9abc_def0u64;
        let chaos: Vec<i64> = (0..2048)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as i64
            })
            .collect();
        assert!(nanocube_pack_recipe_i64(&chaos).is_none());
    }

    #[test]
    fn nanocube_recipe_witness_corruption_rejected() {
        let outputs: Vec<i64> = (0..1024).map(|x| (x as i64) * 9 + 1).collect();
        let mut packed = nanocube_pack_recipe_i64(&outputs).unwrap();
        // Corruption d'un coefficient : les témoins doivent rejeter.
        packed[16] ^= 0x55;
        assert!(nanocube_unpack_recipe_i64(&packed, outputs.len()).is_err());
    }

    #[test]
    fn nanocube_recipe_compresses_better_than_raw_for_long_series() {
        let outputs: Vec<i64> = (0..10_000).map(|x| (x as i64) * 7 - 3).collect();
        let recipe = nanocube_pack_recipe_i64(&outputs).unwrap();
        // Recipe est constant en taille (60 B), peu importe la longueur.
        assert_eq!(recipe.len(), NANOCUBE_POLY_LEN);
        assert!(recipe.len() < outputs.len() * 8 / 100);
    }
}
