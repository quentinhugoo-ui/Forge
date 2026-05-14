//===- kasm.td — TableGen spec du dialecte `kasm` -------------*- tablegen -*-===//
//
// Ω-1.1 — Spécification formelle du dialecte `kasm` au format TableGen
// LLVM/MLIR. Cet artifact est consommable par `mlir-tblgen` lorsque la
// toolchain LLVM/MLIR est disponible. Tant qu'elle ne l'est pas (cf. mur
// Windows toolchain documenté dans Git history), ce fichier sert de
// **spécification de référence** : il fixe la sémantique du dialecte que
// `src/kasm/mlir.rs` émet et parse.
//
// Toute divergence entre `src/kasm/mlir.rs` et ce fichier est un bug.
//
//===---------------------------------------------------------------------===//

#ifndef KASM_DIALECT
#define KASM_DIALECT

include "mlir/IR/OpBase.td"
include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/EnumAttr.td"
include "mlir/Interfaces/InferTypeOpInterface.td"
include "mlir/Interfaces/SideEffectInterfaces.td"

//===---------------------------------------------------------------------===//
// Dialect
//===---------------------------------------------------------------------===//

def Kasm_Dialect : Dialect {
  let name = "kasm";
  let summary = "KASM — bytecode pur, terminant, content-addressed.";
  let description = [{
    Le dialecte `kasm` exprime les programmes KASM en MLIR. Un programme
    est un DAG borné (≤ 4096 nœuds) de calcul pur sur i64/i1 ; les
    références sont strictement arrière, l'exécution termine par
    construction. Voir `KASM.md` pour les invariants détaillés.

    Forme canonique : un programme est dit *canonique* lorsqu'il est issu
    de `kasm::canonicalize` (élimination de code mort output-driven, CSE
    par fingerprint sémantique, normalisation des ops commutatives). La
    forme canonique est **idempotente** par construction et stable sous
    le pipeline `emit_mlir → parse_mlir → canonicalize → emit_mlir`.
  }];
  let cppNamespace = "::kasm";
}

//===---------------------------------------------------------------------===//
// Types
//===---------------------------------------------------------------------===//

// KASM utilise les types built-in MLIR `i64` (entier signé 64 bits) et
// `i1` (booléen). Aucun type custom n'est déclaré — la promesse Ω-1.0 est
// que `kasm` est un sous-ensemble strict de MLIR sans extensions de types.

//===---------------------------------------------------------------------===//
// Attributs (énumérations de Target)
//===---------------------------------------------------------------------===//

def Kasm_TargetAuto   : I32EnumAttrCase<"Auto",   0, "auto">;
def Kasm_TargetCpu    : I32EnumAttrCase<"Cpu",    1, "cpu">;
def Kasm_TargetKernel : I32EnumAttrCase<"Kernel", 2, "kernel">;
def Kasm_TargetGpu    : I32EnumAttrCase<"Gpu",    3, "gpu">;
def Kasm_TargetQpu    : I32EnumAttrCase<"Qpu",    4, "qpu">;

def Kasm_TargetAttr : I32EnumAttr<
  "Target", "Cible d'exécution KASM",
  [Kasm_TargetAuto, Kasm_TargetCpu, Kasm_TargetKernel,
   Kasm_TargetGpu, Kasm_TargetQpu]> {
  let cppNamespace = "::kasm";
}

//===---------------------------------------------------------------------===//
// Op de base
//===---------------------------------------------------------------------===//

class Kasm_Op<string mnemonic, list<Trait> traits = []>
    : Op<Kasm_Dialect, mnemonic,
         !listconcat([Pure, NoMemoryEffect], traits)>;

//===---------------------------------------------------------------------===//
// Programme (région)
//===---------------------------------------------------------------------===//

def Kasm_ProgramOp : Kasm_Op<"program", [
    SingleBlock,
    NoTerminator,
    IsolatedFromAbove
  ]> {
  let summary = "Région-programme KASM (DAG ≤ 4096 nœuds).";
  let description = [{
    Bloc qui contient la suite de nœuds KASM. Les attributs `target`,
    `inputs`, `outputs`, `fuel` correspondent au header binaire (cf.
    `kasm/types.rs`). Les contraintes de borne (`inputs ≤ 16`,
    `outputs ≤ 16`, `fuel ≥ #nodes`, `#nodes ≤ 4096`) sont vérifiées
    par la passe `kasm-verify`.
  }];

  let arguments = (ins
    Kasm_TargetAttr:$target,
    UI8Attr:$inputs,
    UI8Attr:$outputs,
    UI32Attr:$fuel
  );

  let regions = (region SizedRegion<1>:$body);
}

//===---------------------------------------------------------------------===//
// Ops "value-producing"
//===---------------------------------------------------------------------===//

def Kasm_InputOp : Kasm_Op<"input"> {
  let summary = "Lit un slot d'entrée du programme (i64).";
  let arguments = (ins UI8Attr:$slot);
  let results = (outs I64:$result);
  let assemblyFormat = "attr-dict `:` type($result)";
}

def Kasm_ConstOp : Kasm_Op<"const", [ConstantLike]> {
  let summary = "Constante immédiate i16 promue en i64.";
  let arguments = (ins SI16Attr:$value);
  let results = (outs I64:$result);
  let assemblyFormat = "attr-dict `:` type($result)";
}

class Kasm_BinaryI64<string mnemonic, list<Trait> traits = []>
    : Kasm_Op<mnemonic, traits> {
  let arguments = (ins I64:$a, I64:$b);
  let results = (outs I64:$result);
  let assemblyFormat = "$a `,` $b attr-dict `:` type($result)";
}

class Kasm_BinaryI64Bool<string mnemonic, list<Trait> traits = []>
    : Kasm_Op<mnemonic, traits> {
  let arguments = (ins I64:$a, I64:$b);
  let results = (outs I1:$result);
  let assemblyFormat = "$a `,` $b attr-dict `:` type($result)";
}

class Kasm_BinaryBool<string mnemonic, list<Trait> traits = []>
    : Kasm_Op<mnemonic, traits> {
  let arguments = (ins I1:$a, I1:$b);
  let results = (outs I1:$result);
  let assemblyFormat = "$a `,` $b attr-dict `:` type($result)";
}

// Arithmétique i64 → i64
def Kasm_AddOp     : Kasm_BinaryI64<"add",     [Commutative]>;
def Kasm_SubOp     : Kasm_BinaryI64<"sub">;
def Kasm_MulOp     : Kasm_BinaryI64<"mul",     [Commutative]>;
def Kasm_DivCOp    : Kasm_BinaryI64<"divc">;
def Kasm_MinOp     : Kasm_BinaryI64<"min",     [Commutative]>;
def Kasm_MaxOp     : Kasm_BinaryI64<"max",     [Commutative]>;
def Kasm_BandOp    : Kasm_BinaryI64<"band",    [Commutative]>;
def Kasm_BorOp     : Kasm_BinaryI64<"bor",     [Commutative]>;
def Kasm_BxorOp    : Kasm_BinaryI64<"bxor",    [Commutative]>;
def Kasm_ShlOp     : Kasm_BinaryI64<"shl">;
def Kasm_ShrOp     : Kasm_BinaryI64<"shr">;
def Kasm_SatAddOp  : Kasm_BinaryI64<"satadd",  [Commutative]>;
def Kasm_SatSubOp  : Kasm_BinaryI64<"satsub">;
def Kasm_ModCOp    : Kasm_BinaryI64<"modc">;

// Comparaisons i64 → i1
def Kasm_EqOp  : Kasm_BinaryI64Bool<"eq",  [Commutative]>;
def Kasm_LtOp  : Kasm_BinaryI64Bool<"lt">;
def Kasm_LeOp  : Kasm_BinaryI64Bool<"le">;

// Logique booléenne
def Kasm_AndbOp : Kasm_BinaryBool<"andb", [Commutative]>;
def Kasm_OrbOp  : Kasm_BinaryBool<"orb",  [Commutative]>;

def Kasm_NotbOp : Kasm_Op<"notb"> {
  let arguments = (ins I1:$a);
  let results   = (outs I1:$result);
  let assemblyFormat = "$a attr-dict `:` type($result)";
}

// Hash 64 bits
def Kasm_HashOp : Kasm_Op<"hash"> {
  let arguments = (ins I64:$a);
  let results   = (outs I64:$result);
  let assemblyFormat = "$a attr-dict `:` type($result)";
}

// Sélection ternaire (cond:i1, t:i64, f:i64) → i64
def Kasm_SelectOp : Kasm_Op<"select"> {
  let arguments = (ins I1:$cond, I64:$t, I64:$f);
  let results   = (outs I64:$result);
  let assemblyFormat = "$cond `,` $t `,` $f attr-dict `:` type($result)";
}

// Clamp ternaire (a:i64, lo:i64, hi:i64) → i64
def Kasm_ClampOp : Kasm_Op<"clamp"> {
  let arguments = (ins I64:$a, I64:$lo, I64:$hi);
  let results   = (outs I64:$result);
  let assemblyFormat = "$a `,` $lo `,` $hi attr-dict `:` type($result)";
}

// Réduction sur range contigu [base, base + count)
class Kasm_ReduceOp<string mnemonic>
    : Kasm_Op<mnemonic> {
  let arguments = (ins I64:$base, SI16Attr:$count);
  let results   = (outs I64:$result);
  let assemblyFormat = "$base attr-dict `:` type($result)";
}

def Kasm_ReduceAddOp : Kasm_ReduceOp<"reduce_add">;
def Kasm_ReduceMulOp : Kasm_ReduceOp<"reduce_mul">;

// --- Ω-6.1 — opcodes unaires bijectifs (i64 → i64, Landauer-cost zero) ---
class Kasm_BijectiveUnaryI64<string mnemonic>
    : Kasm_Op<mnemonic, [Pure]> {
  let arguments = (ins I64:$a);
  let results   = (outs I64:$result);
  let assemblyFormat = "$a attr-dict `:` type($result)";
}

def Kasm_BitFlipOp : Kasm_BijectiveUnaryI64<"bit_flip">;
def Kasm_NegOp     : Kasm_BijectiveUnaryI64<"neg">;
def Kasm_RevBitsOp : Kasm_BijectiveUnaryI64<"rev_bits">;
def Kasm_BswapOp   : Kasm_BijectiveUnaryI64<"bswap">;

// Output (référence un nœud antérieur, ne consomme pas le résultat)
def Kasm_OutputOp : Kasm_Op<"output"> {
  let arguments = (ins AnyTypeOf<[I64, I1]>:$value);
  let results   = (outs AnyTypeOf<[I64, I1]>:$result);
  let assemblyFormat = "$value attr-dict `:` type($result)";
}

#endif // KASM_DIALECT
