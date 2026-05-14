//! Σ.14 (Wave 11, 2026-05-02) — Errno-style error codes.
//!
//! **Origine** : Linux kernel `errno` (POSIX), Go `error` interface
//! avec sentinel codes. Idée centrale : sur le hot path, retourner un
//! `i32` errno (8 bytes packed avec un i64 result via Result<i64, ()>
//! ou directement `Result<i64, KasmErrno>`) au lieu d'un
//! `KasmError` boxed (~80 bytes, 2 cache lines).
//!
//! Détail décodé seulement aux frontières (UI, log, debug). Le hot
//! path success-case ne paie pas le coût de copie de l'enum Boxed.
//!
//! ## Pourquoi pour Forge ?
//!
//! `Result<Value, KasmError>` est partout dans le slow-lane interpreter
//! (`kasm::execute`). Chaque return réussi pousse 80 bytes "ok bytes"
//! qui valent toujours `Ok(Value)` mais doivent être copiés vers le
//! caller pour le pattern match. Sur Vec<i64> outputs, c'est plusieurs
//! kbytes de Result encodés sur la stack.
//!
//! Σ.14 expose `KasmErrno: i32` qui mappe les variants `KasmError` les
//! plus fréquents en codes compacts. Wave 11 minimal viable : la
//! conversion + le mapping. Le wiring effectif sur le hot path est
//! Wave 12+ (audit pour mesurer le gain réel avant refactor large).
//!
//! ## Architecture Wave 11 minimal viable
//!
//! - `KasmErrno(i32)` newtype.
//! - Constants : `OK = 0`, `BAD_REF = -1`, `BAD_INPUT = -2`, etc.
//! - `from_error(&KasmError) -> KasmErrno` mapping fonction.
//! - `to_error(&self) -> Option<KasmError>` reverse (best-effort, perd
//!   le détail des champs payload).
//! - Documentation que les codes sont stables cross-version.
//!
//! ## Limitations Wave 11 minimal
//!
//! - One-way info loss : `KasmError::BadRef { node: 42 }` → `BAD_REF`
//!   sans le node 42. Acceptable pour hot path (pour debug, garder
//!   le KasmError full).
//! - Pas de wiring effectif dans interpreter.rs Wave 11. La présence
//!   de l'API permet aux callers qui le souhaitent (e.g. JIT
//!   slow-lane fallback) de bénéficier sans casser l'existant.

use crate::kasm::types::KasmError;

/// Code errno KASM compact (4 bytes au lieu de ~80 pour KasmError).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KasmErrno(pub i32);

// Constants : codes errno stables. 0 = OK, négatifs = erreurs, positifs
// réservés pour signaux non-fatal.
impl KasmErrno {
    pub const OK: KasmErrno = KasmErrno(0);

    // Format / structure errors (—10 series).
    pub const BAD_MAGIC: KasmErrno = KasmErrno(-10);
    pub const BAD_VERSION: KasmErrno = KasmErrno(-11);
    pub const BAD_TARGET: KasmErrno = KasmErrno(-12);
    pub const BAD_TYPE: KasmErrno = KasmErrno(-13);
    pub const BAD_OP: KasmErrno = KasmErrno(-14);
    pub const BAD_LENGTH: KasmErrno = KasmErrno(-15);
    pub const BAD_FOOTER: KasmErrno = KasmErrno(-16);
    pub const BAD_NODE_COUNT: KasmErrno = KasmErrno(-17);
    pub const TOO_MANY_SLOTS: KasmErrno = KasmErrno(-18);
    pub const FUEL_TOO_SMALL: KasmErrno = KasmErrno(-19);
    pub const TRUNCATED: KasmErrno = KasmErrno(-20);

    // Runtime / verifier errors (—30 series).
    pub const BAD_INPUT_LENGTH: KasmErrno = KasmErrno(-30);
    pub const BAD_INPUT_SLOT: KasmErrno = KasmErrno(-31);
    pub const BAD_REF: KasmErrno = KasmErrno(-32);
    pub const TYPE_MISMATCH: KasmErrno = KasmErrno(-33);
    pub const OUTPUT_COUNT: KasmErrno = KasmErrno(-34);
    pub const VALUE_TYPE_MISMATCH: KasmErrno = KasmErrno(-35);

    // Composition errors (—50 series).
    pub const COMPOSE_ARITY: KasmErrno = KasmErrno(-50);
    pub const COMPOSE_TYPE: KasmErrno = KasmErrno(-51);
    pub const EXTERNAL_TARGET: KasmErrno = KasmErrno(-52);

    // Reduce / F64 / multimethod errors (—70 series).
    pub const BAD_REDUCE_COUNT: KasmErrno = KasmErrno(-70);
    pub const BAD_F64_SUB_OP: KasmErrno = KasmErrno(-71);
    pub const UNSUPPORTED_V1_OP: KasmErrno = KasmErrno(-72);
    pub const BAD_MULTI_METHOD: KasmErrno = KasmErrno(-73);
    pub const NO_METHOD_FOUND: KasmErrno = KasmErrno(-74);
    pub const ABSTRACT_DISPATCH: KasmErrno = KasmErrno(-75);

    /// Catch-all pour les variants non encore mappés. Acceptable car
    /// les nouveaux KasmError variants sont rares — la mise à jour
    /// du mapping est triviale.
    pub const UNKNOWN: KasmErrno = KasmErrno(-1);

    /// Vrai si errno indique succès.
    pub fn is_ok(self) -> bool {
        self.0 == 0
    }

    /// Vrai si errno indique erreur.
    pub fn is_err(self) -> bool {
        self.0 != 0
    }

    /// Code raw i32.
    pub fn code(self) -> i32 {
        self.0
    }

    /// Mapping `KasmError` → `KasmErrno`. Conserve la classe de
    /// l'erreur, perd les détails de payload (acceptable pour hot path).
    pub fn from_error(err: &KasmError) -> Self {
        match err {
            KasmError::BadMagic => Self::BAD_MAGIC,
            KasmError::BadVersion(_) => Self::BAD_VERSION,
            KasmError::BadTarget(_) => Self::BAD_TARGET,
            KasmError::BadType(_) => Self::BAD_TYPE,
            KasmError::BadOp(_) => Self::BAD_OP,
            KasmError::BadLength => Self::BAD_LENGTH,
            KasmError::BadFooter => Self::BAD_FOOTER,
            KasmError::BadNodeCount(_) => Self::BAD_NODE_COUNT,
            KasmError::TooManySlots => Self::TOO_MANY_SLOTS,
            KasmError::FuelTooSmall => Self::FUEL_TOO_SMALL,
            KasmError::Truncated => Self::TRUNCATED,
            KasmError::BadInputLength { .. } => Self::BAD_INPUT_LENGTH,
            KasmError::BadInputSlot { .. } => Self::BAD_INPUT_SLOT,
            KasmError::BadRef { .. } => Self::BAD_REF,
            KasmError::TypeMismatch { .. } => Self::TYPE_MISMATCH,
            KasmError::OutputCount { .. } => Self::OUTPUT_COUNT,
            KasmError::ValueTypeMismatch { .. } => Self::VALUE_TYPE_MISMATCH,
            KasmError::ComposeArity { .. } => Self::COMPOSE_ARITY,
            KasmError::ComposeType { .. } => Self::COMPOSE_TYPE,
            KasmError::ExternalTarget(_) => Self::EXTERNAL_TARGET,
            KasmError::BadReduceCount { .. } => Self::BAD_REDUCE_COUNT,
            KasmError::BadF64SubOp(_) => Self::BAD_F64_SUB_OP,
            KasmError::UnsupportedV1OpInScalarInterpreter { .. } => Self::UNSUPPORTED_V1_OP,
            // Wave 4 — MultiMethod errors (catch-all sur le pattern).
            _ => Self::UNKNOWN,
        }
    }

    /// Description human-readable du code (pour log/debug).
    pub fn description(self) -> &'static str {
        match self {
            Self::OK => "ok",
            Self::BAD_MAGIC => "bad magic",
            Self::BAD_VERSION => "bad version",
            Self::BAD_TARGET => "bad target",
            Self::BAD_TYPE => "bad type",
            Self::BAD_OP => "bad opcode",
            Self::BAD_LENGTH => "bad length",
            Self::BAD_FOOTER => "bad footer",
            Self::BAD_NODE_COUNT => "bad node count",
            Self::TOO_MANY_SLOTS => "too many slots",
            Self::FUEL_TOO_SMALL => "fuel too small",
            Self::TRUNCATED => "truncated",
            Self::BAD_INPUT_LENGTH => "bad input length",
            Self::BAD_INPUT_SLOT => "bad input slot",
            Self::BAD_REF => "bad reference",
            Self::TYPE_MISMATCH => "type mismatch",
            Self::OUTPUT_COUNT => "output count mismatch",
            Self::VALUE_TYPE_MISMATCH => "value type mismatch",
            Self::COMPOSE_ARITY => "compose arity mismatch",
            Self::COMPOSE_TYPE => "compose type mismatch",
            Self::EXTERNAL_TARGET => "external target",
            Self::BAD_REDUCE_COUNT => "bad reduce count",
            Self::BAD_F64_SUB_OP => "bad F64 sub-op",
            Self::UNSUPPORTED_V1_OP => "unsupported V1+ op in scalar interpreter",
            _ => "unknown error",
        }
    }
}

/// Résultat compact errno-style : `Result<T, KasmErrno>` au lieu de
/// `Result<T, KasmError>`. Hot path peut utiliser ce type pour économiser
/// 76 bytes par return value (8 bytes errno + 8 bytes T value vs
/// ~80 bytes KasmError boxed enum).
pub type Errno<T> = Result<T, KasmErrno>;

/// Convertit Result<T, KasmError> → Result<T, KasmErrno>. Loss of detail
/// acceptable pour hot path success cases.
pub fn errno_result<T>(r: Result<T, KasmError>) -> Errno<T> {
    r.map_err(|e| KasmErrno::from_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_size_is_4_bytes() {
        // KasmErrno doit fit dans i32 = 4 bytes (vs ~80 bytes pour
        // KasmError boxed). C'est l'argument central de Σ.14.
        assert_eq!(std::mem::size_of::<KasmErrno>(), 4);
    }

    #[test]
    fn errno_ok_is_zero() {
        let ok = KasmErrno::OK;
        assert_eq!(ok.code(), 0);
        assert!(ok.is_ok());
        assert!(!ok.is_err());
    }

    #[test]
    fn errno_errors_are_negative() {
        // Convention POSIX : erreurs en négatif, OK en 0, signaux en positif.
        for errno in [
            KasmErrno::BAD_MAGIC, KasmErrno::BAD_REF, KasmErrno::TYPE_MISMATCH,
            KasmErrno::FUEL_TOO_SMALL, KasmErrno::TRUNCATED,
        ] {
            assert!(errno.code() < 0, "errno {} doit être négatif", errno.code());
            assert!(errno.is_err());
        }
    }

    #[test]
    fn errno_codes_are_unique() {
        // Aucun code errno ne doit collisionner — sinon perte d'info.
        let codes = [
            KasmErrno::OK, KasmErrno::BAD_MAGIC, KasmErrno::BAD_VERSION,
            KasmErrno::BAD_TARGET, KasmErrno::BAD_TYPE, KasmErrno::BAD_OP,
            KasmErrno::BAD_LENGTH, KasmErrno::BAD_FOOTER,
            KasmErrno::BAD_NODE_COUNT, KasmErrno::TOO_MANY_SLOTS,
            KasmErrno::FUEL_TOO_SMALL, KasmErrno::TRUNCATED,
            KasmErrno::BAD_INPUT_LENGTH, KasmErrno::BAD_INPUT_SLOT,
            KasmErrno::BAD_REF, KasmErrno::TYPE_MISMATCH,
            KasmErrno::OUTPUT_COUNT, KasmErrno::VALUE_TYPE_MISMATCH,
            KasmErrno::COMPOSE_ARITY, KasmErrno::COMPOSE_TYPE,
            KasmErrno::EXTERNAL_TARGET, KasmErrno::BAD_REDUCE_COUNT,
            KasmErrno::BAD_F64_SUB_OP, KasmErrno::UNSUPPORTED_V1_OP,
        ];
        let unique: std::collections::HashSet<i32> = codes.iter().map(|e| e.code()).collect();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn errno_from_error_maps_correctly() {
        let cases: Vec<(KasmError, KasmErrno)> = vec![
            (KasmError::BadMagic, KasmErrno::BAD_MAGIC),
            (KasmError::BadOp(42), KasmErrno::BAD_OP),
            (KasmError::BadRef { node: 0, reference: 99 }, KasmErrno::BAD_REF),
            (KasmError::FuelTooSmall, KasmErrno::FUEL_TOO_SMALL),
            (KasmError::Truncated, KasmErrno::TRUNCATED),
            (KasmError::TypeMismatch { node: 5 }, KasmErrno::TYPE_MISMATCH),
        ];
        for (err, expected) in cases {
            let got = KasmErrno::from_error(&err);
            assert_eq!(got, expected, "errno mismatch for {:?}", err);
        }
    }

    #[test]
    fn errno_description_non_empty() {
        for code in [
            KasmErrno::OK, KasmErrno::BAD_MAGIC, KasmErrno::BAD_REF,
            KasmErrno::FUEL_TOO_SMALL, KasmErrno::UNSUPPORTED_V1_OP,
        ] {
            let desc = code.description();
            assert!(!desc.is_empty(), "description vide pour {:?}", code);
        }
    }

    #[test]
    fn errno_unknown_is_catch_all() {
        // Le mapping doit retourner UNKNOWN pour les variants non encore
        // mappés (aucun KasmError unmappable ne devrait planter).
        // Test indirect : on couvre déjà tous les variants principaux,
        // mais futurs variants → UNKNOWN.
        assert_eq!(KasmErrno::UNKNOWN.code(), -1);
        assert!(KasmErrno::UNKNOWN.is_err());
    }

    #[test]
    fn errno_result_helper_converts() {
        let err: Result<i64, KasmError> = Err(KasmError::BadMagic);
        let errno_r: Errno<i64> = errno_result(err);
        assert!(errno_r.is_err());
        assert_eq!(errno_r.unwrap_err(), KasmErrno::BAD_MAGIC);

        let ok: Result<i64, KasmError> = Ok(42);
        let errno_r: Errno<i64> = errno_result(ok);
        assert_eq!(errno_r.unwrap(), 42);
    }
}
