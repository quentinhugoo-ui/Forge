//! Signed Distance Field primitives — Banger Frontier, étape 2.
//!
//! Frontier hypothesis: traiter chaque opération booléenne SDF comme un
//! calcul tropical (min/+) avec un opérateur softmin différentiable
//! disponible d'emblée. Cela fusionne deux étapes prévues d'INGEN
//! COMPUTE (section 1 booléens + section 20.1 Log-Sum-Exp) en une seule
//! primitive, et donne à Banger un smooth-blend organique dès le jour 1.
//!
//! Wall poussé : qualité d'expression. Avant cette abstraction la
//! composition de formes n'existait pas dans le crate. Après, un agent
//! (LLM ou KASM lowering) peut décrire des champs implicites en boîte
//! noire derrière `dyn GenerativeField`, et les futures étapes
//! (raymarching wgpu, lowering KASM, optimiseur tropical) plug dessus.

/// Point dans R³. On reste sur `[f32; 3]` pour éviter d'importer glam /
/// nalgebra — la doctrine Forge interdit les dépendances tierces qui
/// n'apportent rien que le compilateur ne sache déjà.
pub type Vec3 = [f32; 3];

/// Champ implicite : `distance(p)` rend la distance signée du point `p`
/// à la surface du volume.
///
/// Convention : négatif à l'intérieur, positif à l'extérieur, zéro sur
/// la surface. C'est l'invariant que tout primitive et combinateur doit
/// préserver pour rester un SDF valide (lipschitz-1 idéalement, mais
/// le smooth union le viole légèrement comme prévu par la littérature).
pub trait GenerativeField: Send + Sync {
    fn distance(&self, p: Vec3) -> f32;
}

// ---------- Primitives -----------------------------------------------------

/// Sphère centrée à l'origine.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub radius: f32,
}

impl GenerativeField for Sphere {
    fn distance(&self, p: Vec3) -> f32 {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        len - self.radius
    }
}

/// Boîte axis-aligned centrée à l'origine, demi-extents `half`.
#[derive(Clone, Copy, Debug)]
pub struct AaBox {
    pub half: Vec3,
}

impl GenerativeField for AaBox {
    fn distance(&self, p: Vec3) -> f32 {
        // Forme classique d'Inigo Quilez : distance d'un point à une AABB.
        let q = [
            p[0].abs() - self.half[0],
            p[1].abs() - self.half[1],
            p[2].abs() - self.half[2],
        ];
        let outside = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2) + q[2].max(0.0).powi(2)).sqrt();
        let inside = q[0].max(q[1].max(q[2])).min(0.0);
        outside + inside
    }
}

// ---------- Combinateurs ---------------------------------------------------

/// Translation rigide d'un champ.
pub struct Translate<F: GenerativeField> {
    pub inner: F,
    pub offset: Vec3,
}

impl<F: GenerativeField> GenerativeField for Translate<F> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance([
            p[0] - self.offset[0],
            p[1] - self.offset[1],
            p[2] - self.offset[2],
        ])
    }
}

/// Union booléenne classique (tropicale min).
pub struct Union<A: GenerativeField, B: GenerativeField> {
    pub a: A,
    pub b: B,
}

impl<A: GenerativeField, B: GenerativeField> GenerativeField for Union<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        self.a.distance(p).min(self.b.distance(p))
    }
}

/// Intersection booléenne (tropicale max).
pub struct Intersection<A: GenerativeField, B: GenerativeField> {
    pub a: A,
    pub b: B,
}

impl<A: GenerativeField, B: GenerativeField> GenerativeField for Intersection<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        self.a.distance(p).max(self.b.distance(p))
    }
}

/// Différence : A privé de B.
pub struct Difference<A: GenerativeField, B: GenerativeField> {
    pub a: A,
    pub b: B,
}

impl<A: GenerativeField, B: GenerativeField> GenerativeField for Difference<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        self.a.distance(p).max(-self.b.distance(p))
    }
}

/// Smooth union différentiable via log-sum-exp (softmin).
///
/// `sharpness` (k) contrôle la transition : `k → ∞` redonne le min dur,
/// `k → 0` donne un blend très diffus. Implémentation numériquement
/// stable (factorisation par `min`).
pub struct SmoothUnion<A: GenerativeField, B: GenerativeField> {
    pub a: A,
    pub b: B,
    pub sharpness: f32,
}

impl<A: GenerativeField, B: GenerativeField> GenerativeField for SmoothUnion<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        let da = self.a.distance(p);
        let db = self.b.distance(p);
        let k = self.sharpness.max(f32::EPSILON);
        // softmin(a, b; k) = -1/k * log(exp(-k a) + exp(-k b))
        // = m - 1/k * log(exp(-k(a-m)) + exp(-k(b-m))), m = min(a, b)
        let m = da.min(db);
        let s = ((-k * (da - m)).exp() + (-k * (db - m)).exp()).ln() / k;
        m - s
    }
}

// ---------- Helpers de composition dynamique --------------------------------

/// Wrapper pour stocker des champs hétérogènes derrière `Box<dyn ...>`.
/// Utile pour les futurs lowering KASM → arbre SDF où le type concret
/// n'est connu qu'à la runtime.
pub type DynField = Box<dyn GenerativeField>;

/// Union dynamique sur un slice de champs : c'est l'opération naturelle
/// d'un registre KASM/Banger qui agrège plusieurs intentions.
pub fn union_all(fields: &[DynField], p: Vec3) -> f32 {
    fields
        .iter()
        .map(|f| f.distance(p))
        .fold(f32::INFINITY, f32::min)
}

// ---------- Tests ----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn sphere_distance_signs() {
        let s = Sphere { radius: 1.0 };
        assert!(approx(s.distance([0.0, 0.0, 0.0]), -1.0, 1e-6));
        assert!(approx(s.distance([1.0, 0.0, 0.0]), 0.0, 1e-6));
        assert!(approx(s.distance([2.0, 0.0, 0.0]), 1.0, 1e-6));
        assert!(approx(s.distance([0.0, 3.0, 0.0]), 2.0, 1e-6));
    }

    #[test]
    fn box_distance_inside_outside_and_corner() {
        let b = AaBox { half: [1.0, 1.0, 1.0] };
        // Centre : la distance signée d'une AABB au centre vaut -min(half_i).
        assert!(approx(b.distance([0.0, 0.0, 0.0]), -1.0, 1e-6));
        // Face : juste à l'extérieur du x+, 0.5.
        assert!(approx(b.distance([1.5, 0.0, 0.0]), 0.5, 1e-6));
        // Coin : à (2,2,2), distance = sqrt(3) au coin (1,1,1).
        assert!(approx(b.distance([2.0, 2.0, 2.0]), (3.0_f32).sqrt(), 1e-6));
    }

    #[test]
    fn translate_shifts_origin() {
        let s = Translate {
            inner: Sphere { radius: 1.0 },
            offset: [5.0, 0.0, 0.0],
        };
        assert!(approx(s.distance([5.0, 0.0, 0.0]), -1.0, 1e-6));
        assert!(approx(s.distance([6.0, 0.0, 0.0]), 0.0, 1e-6));
    }

    #[test]
    fn union_picks_nearest_surface() {
        let u = Union {
            a: Sphere { radius: 1.0 },
            b: Translate {
                inner: Sphere { radius: 1.0 },
                offset: [4.0, 0.0, 0.0],
            },
        };
        // Au milieu : distance au plus proche, la sphère 2 (x=3) ou la 1 (x=1).
        // En (2, 0, 0) : d_a = 1, d_b = |2-4|-1 = 1 → min = 1.
        assert!(approx(u.distance([2.0, 0.0, 0.0]), 1.0, 1e-6));
        // À l'intérieur de la sphère 2 :
        assert!(approx(u.distance([4.0, 0.0, 0.0]), -1.0, 1e-6));
    }

    #[test]
    fn intersection_takes_max() {
        let i = Intersection {
            a: Sphere { radius: 1.0 },
            b: Translate {
                inner: Sphere { radius: 1.0 },
                offset: [0.5, 0.0, 0.0],
            },
        };
        // (0.25, 0, 0) est dans les deux sphères ; distance = max des deux.
        let d = i.distance([0.25, 0.0, 0.0]);
        let da: f32 = -0.75;
        let db: f32 = (0.25_f32 - 0.5).abs() - 1.0; // -0.75
        assert!(approx(d, da.max(db), 1e-6));
    }

    #[test]
    fn difference_carves() {
        let d = Difference {
            a: Sphere { radius: 1.0 },
            b: Sphere { radius: 0.5 },
        };
        // Au centre : intérieur de A (-1), mais aussi intérieur de B (-0.5)
        // → différence = max(-1, +0.5) = 0.5. Le creux est bien vide.
        assert!(approx(d.distance([0.0, 0.0, 0.0]), 0.5, 1e-6));
        // Sur la surface externe de A : distance = 0.
        assert!(approx(d.distance([1.0, 0.0, 0.0]), 0.0, 1e-6));
    }

    #[test]
    fn smooth_union_converges_to_min_for_large_k() {
        let hard = Union {
            a: Sphere { radius: 1.0 },
            b: Translate { inner: Sphere { radius: 1.0 }, offset: [2.5, 0.0, 0.0] },
        };
        let soft = SmoothUnion {
            a: Sphere { radius: 1.0 },
            b: Translate { inner: Sphere { radius: 1.0 }, offset: [2.5, 0.0, 0.0] },
            sharpness: 100.0,
        };
        for &x in &[-0.5_f32, 0.5, 1.25, 2.0, 3.0] {
            let h = hard.distance([x, 0.0, 0.0]);
            let s = soft.distance([x, 0.0, 0.0]);
            assert!(
                approx(h, s, 5e-2),
                "softmin (k=100) doit approcher min: x={x} hard={h} soft={s}"
            );
        }
    }

    #[test]
    fn smooth_union_bridges_disconnected_shapes() {
        // Deux sphères trop loin pour se toucher : l'union dure laisse
        // un vide entre elles, le smooth union à k modéré crée un pont
        // (distance plus petite que celle de chaque sphère prise seule).
        let mid = [1.25_f32, 0.0, 0.0];
        let hard = Union {
            a: Sphere { radius: 1.0 },
            b: Translate { inner: Sphere { radius: 1.0 }, offset: [2.5, 0.0, 0.0] },
        };
        let soft = SmoothUnion {
            a: Sphere { radius: 1.0 },
            b: Translate { inner: Sphere { radius: 1.0 }, offset: [2.5, 0.0, 0.0] },
            sharpness: 2.0,
        };
        let h = hard.distance(mid);
        let s = soft.distance(mid);
        assert!(s < h, "smooth union doit rapprocher la surface au milieu: hard={h} soft={s}");
    }

    #[test]
    fn dyn_union_all_matches_pairwise() {
        let fields: Vec<DynField> = vec![
            Box::new(Sphere { radius: 1.0 }),
            Box::new(Translate { inner: Sphere { radius: 1.0 }, offset: [3.0, 0.0, 0.0] }),
            Box::new(Translate { inner: Sphere { radius: 1.0 }, offset: [0.0, 3.0, 0.0] }),
        ];
        let p = [1.5_f32, 1.5, 0.0];
        let dyn_d = union_all(&fields, p);
        let manual = fields[0].distance(p).min(fields[1].distance(p)).min(fields[2].distance(p));
        assert!(approx(dyn_d, manual, 1e-6));
    }
}
