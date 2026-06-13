//===- meta.td — TableGen spec du dialecte `meta` ------------*- tablegen -*-===//
//
// Ω-4.6 — Spécification formelle du dialecte `meta` au format TableGen
// LLVM/MLIR. Dialecte L4 meta : Calculus of Constructions minimal.
//
// Cet artifact est consommable par `mlir-tblgen` lorsque la toolchain
// LLVM/MLIR est disponible. Tant qu'elle ne l'est pas (cf. mur Windows
// toolchain documenté dans Git history), ce fichier sert de **spécification
// de référence** : il fixe la sémantique du dialecte que
// `src/meta/mlir_bridge.rs` émet et parse.
//
// Toute divergence entre `src/meta/mlir_bridge.rs` et ce fichier est un
// bug.
//
//===---------------------------------------------------------------------===//

#ifndef META_DIALECT
#define META_DIALECT

include "mlir/IR/OpBase.td"
include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/EnumAttr.td"

//===---------------------------------------------------------------------===//
// Dialect
//===---------------------------------------------------------------------===//

def Meta_Dialect : Dialect {
  let name = "meta";
  let summary = "Calculus of Constructions minimal, content-addressed.";
  let description = [{
    Le dialecte `meta` exprime les termes du Calculus of Constructions
    minimal en MLIR. Cinq constructeurs : `var` (de Bruijn), `sort`
    (universes), `lam` (λ-abstraction), `pi` (Π-type, dépendant), `app`
    (application). Les binders sont représentés via indices de de Bruijn,
    rendant l'α-équivalence structurelle.

    Forme : un module `meta.term` contient une suite linéaire de
    définitions SSA en post-ordre topologique (sous-termes avant
    parents), terminée par une ligne `meta.root %N` qui désigne la
    racine de l'AST.

    Le hash content-addressed (`Term::hash()`) est invariant sous le
    pipeline `emit_meta_mlir → parse_meta_mlir`.
  }];
  let cppNamespace = "::meta";
}

//===---------------------------------------------------------------------===//
// Types — aucun
//===---------------------------------------------------------------------===//

// `meta` n'introduit pas de types MLIR custom. Les `Term` du CoC ne sont
// pas typés au niveau MLIR — leur typage est externe (cf. `meta::infer`).
// Les opérations produisent toutes un même type opaque "term", qu'on
// pourrait modéliser via un `MetaTermType` si on intégrait à un build
// MLIR ; pour l'instant, les SSA values sont type-erased côté codec
// texte.

//===---------------------------------------------------------------------===//
// Op de base
//===---------------------------------------------------------------------===//

class Meta_Op<string mnemonic, list<Trait> traits = []>
    : Op<Meta_Dialect, mnemonic,
         !listconcat([Pure, NoMemoryEffect], traits)>;

//===---------------------------------------------------------------------===//
// Région-module
//===---------------------------------------------------------------------===//

def Meta_TermOp : Meta_Op<"term", [
    SingleBlock,
    NoTerminator,
    IsolatedFromAbove
  ]> {
  let summary = "Bloc qui contient les définitions d'un AST Term du CoC.";
  let description = [{
    Bloc unique qui héberge la suite linéaire des définitions SSA du
    terme, en post-ordre topologique, suivie d'une `meta.root` désignant
    la racine.
  }];
  let regions = (region SizedRegion<1>:$body);
}

//===---------------------------------------------------------------------===//
// Constructeurs — feuilles
//===---------------------------------------------------------------------===//

def Meta_VarOp : Meta_Op<"var"> {
  let summary = "Variable de de Bruijn.";
  let description = [{
    `Var(n)` désigne le binder à profondeur `n` ; `Var(0)` = binder le
    plus interne. L'attribut `index` porte la valeur entière non signée.
  }];
  let arguments = (ins UI32Attr:$index);
  let results   = (outs AnyType:$result);
  let assemblyFormat = "attr-dict";
}

def Meta_SortOp : Meta_Op<"sort"> {
  let summary = "Sort universe (Type, Type1, …).";
  let description = [{
    `Sort(n)` désigne l'universe de niveau `n`. `Sort(0) = Type`.
    L'attribut `level` porte la valeur entière non signée.
  }];
  let arguments = (ins UI32Attr:$level);
  let results   = (outs AnyType:$result);
  let assemblyFormat = "attr-dict";
}

//===---------------------------------------------------------------------===//
// Constructeurs — internes (binders et application)
//===---------------------------------------------------------------------===//

def Meta_LamOp : Meta_Op<"lam"> {
  let summary = "λ-abstraction : λ (_: ty). body.";
  let description = [{
    `Lam { ty, body }` ; `body` référence le paramètre via `Var(0)`.
    Les opérandes sont les SSA values des sous-termes `ty` puis `body`.
  }];
  let arguments = (ins AnyType:$ty, AnyType:$body);
  let results   = (outs AnyType:$result);
  let assemblyFormat = "$ty `,` $body attr-dict";
}

def Meta_PiOp : Meta_Op<"pi"> {
  let summary = "Π-type dépendant : Π (_: ty). body.";
  let description = [{
    `Pi { ty, body }` ; `body` référence le paramètre via `Var(0)`. Si
    `body` n'utilise pas `Var(0)`, alors c'est un type fonction
    ordinaire `ty → body`. La distinction est purement contextuelle.
  }];
  let arguments = (ins AnyType:$ty, AnyType:$body);
  let results   = (outs AnyType:$result);
  let assemblyFormat = "$ty `,` $body attr-dict";
}

def Meta_AppOp : Meta_Op<"app"> {
  let summary = "Application f x.";
  let description = [{
    `App(f, x)`. Aucune contrainte de type ici (le type-check est
    externe au dialecte ; cf. `meta::infer`).
  }];
  let arguments = (ins AnyType:$f, AnyType:$x);
  let results   = (outs AnyType:$result);
  let assemblyFormat = "$f `,` $x attr-dict";
}

//===---------------------------------------------------------------------===//
// Racine du module
//===---------------------------------------------------------------------===//

def Meta_RootOp : Meta_Op<"root"> {
  let summary = "Désigne la racine de l'AST.";
  let description = [{
    Exactement une instance par module `meta.term`. Aucune valeur
    produite ; sert d'ancrage pour le parseur qui reconstruit l'AST en
    déréférençant la SSA value pointée.
  }];
  let arguments = (ins AnyType:$value);
  let results   = (outs);
  let assemblyFormat = "$value attr-dict";
}

#endif // META_DIALECT
