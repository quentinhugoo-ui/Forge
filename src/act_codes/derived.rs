//! Derived analyses — the second tier of the atlas.
//!
//! Where an `ActCode` is a heavy SDF computation worth content-addressing
//! (inertia Monte-Carlo, modal Lanczos, thermal CG), a *derived analysis*
//! consumes the cheap outputs of those act codes plus a configuration and
//! returns a verdict in microseconds. No voxelisation, no GPU, no ledger —
//! caching a 20-flop formula would cost more than recomputing it.
//!
//! Keeping these separate from the `ActCode` trait is deliberate Via
//! Negativa : not every calculation belongs in the cache layer, and forcing
//! a control-authority formula through the SDF→artifact→hash machinery would
//! add a middleman for nothing.
//!
//! First entry : `cg_envelope` — the static stability margin of a multirotor.

use super::Vec3;

const G: f64 = 9.81;

/// A rotor mount : world position of the thrust application point and the
/// maximum thrust that prop can produce (N).
#[derive(Clone, Debug)]
pub struct PropMount {
    pub pos: Vec3,
    pub max_thrust_n: f64,
}

/// Static-stability verdict for a given centre of mass and rotor set.
#[derive(Clone, Debug)]
pub struct StabilityReport {
    /// Horizontal COM offset from the thrust centroid (m).
    pub offset_xy: f64,
    /// How far the COM sits below the rotor plane (m, positive = below =
    /// pendulum-stable). Negative means top-heavy.
    pub pendulum_margin: f64,
    /// Corrective-moment / gravity-moment ratio about the roll axis. >1 means
    /// the rotors can hold the offset at hover ; f64::INFINITY when the COM is
    /// laterally balanced (no gravity moment to fight → full maneuver budget).
    pub roll_authority: f64,
    /// Same about the pitch axis.
    pub pitch_authority: f64,
    /// Absolute maximum corrective moment the rotor set can apply at hover
    /// (N·m) — the maneuver budget, independent of the current offset.
    pub maneuver_moment: f64,
    /// True if both axes are controllable AND the craft is not top-heavy.
    pub controllable: bool,
}

/// Compute the static-stability envelope of a multirotor.
///
/// Model : at hover each prop carries `h = m·g / n` newtons. To correct a
/// gravity moment from a lateral COM offset, the rotors apply an
/// antisymmetric differential `δ = min(T_max − h, h)` per prop (can't exceed
/// max thrust, can't go below zero), preserving total lift. The maximum
/// corrective moment about an axis is `δ · Σ |arm_perpendicular_i|`. The
/// authority is that capacity divided by the gravity moment `m·g·offset`.
pub fn cg_envelope(com: Vec3, mass: f64, props: &[PropMount]) -> StabilityReport {
    if props.is_empty() || mass <= 0.0 {
        return StabilityReport {
            offset_xy: 0.0,
            pendulum_margin: 0.0,
            roll_authority: 0.0,
            pitch_authority: 0.0,
            maneuver_moment: 0.0,
            controllable: false,
        };
    }
    let n = props.len() as f64;

    // Thrust centroid (geometric centre of the rotor application points).
    let mut tc = [0.0f64; 3];
    for p in props {
        for i in 0..3 { tc[i] += p.pos[i]; }
    }
    for i in 0..3 { tc[i] /= n; }

    let off_x = com[0] - tc[0];
    let off_y = com[1] - tc[1];
    let offset_xy = (off_x * off_x + off_y * off_y).sqrt();
    // Rotor plane height = mean prop z. COM below it ⇒ pendulum-stable.
    let pendulum_margin = tc[2] - com[2];

    // Per-prop hover thrust and antisymmetric differential capacity.
    let h = mass * G / n;
    // Corrective-moment capacity about each axis : δ_i can differ per prop
    // because T_max can differ ; sum the per-prop contribution.
    // Roll axis = x ; the moment arm is the y-distance of the prop.
    // Pitch axis = y ; the moment arm is the x-distance.
    let mut m_roll_cap = 0.0;
    let mut m_pitch_cap = 0.0;
    for p in props {
        let delta = (p.max_thrust_n - h).min(h).max(0.0);
        m_roll_cap += delta * (p.pos[1] - tc[1]).abs();
        m_pitch_cap += delta * (p.pos[0] - tc[0]).abs();
    }

    // Gravity moments from the lateral offset.
    let m_grav_roll = mass * G * off_y.abs();
    let m_grav_pitch = mass * G * off_x.abs();

    let roll_authority = if m_grav_roll < 1e-9 { f64::INFINITY } else { m_roll_cap / m_grav_roll };
    let pitch_authority = if m_grav_pitch < 1e-9 { f64::INFINITY } else { m_pitch_cap / m_grav_pitch };
    let maneuver_moment = m_roll_cap.min(m_pitch_cap);

    // Controllable : both axes can hold the offset with margin, and the COM
    // is not above the rotor plane (top-heavy → actively unstable).
    let controllable = roll_authority >= 1.0 && pitch_authority >= 1.0 && pendulum_margin >= 0.0;

    StabilityReport {
        offset_xy,
        pendulum_margin,
        roll_authority,
        pitch_authority,
        maneuver_moment,
        controllable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard symmetric quad on a 0.1 m ring, COM at the centre : laterally
    /// balanced (infinite authority), pendulum-neutral, controllable.
    fn quad(max_thrust_n: f64, z: f64) -> Vec<PropMount> {
        vec![
            PropMount { pos: [0.1, 0.0, z], max_thrust_n },
            PropMount { pos: [-0.1, 0.0, z], max_thrust_n },
            PropMount { pos: [0.0, 0.1, z], max_thrust_n },
            PropMount { pos: [0.0, -0.1, z], max_thrust_n },
        ]
    }

    #[test]
    fn balanced_quad_is_fully_authoritative() {
        let r = cg_envelope([0.0, 0.0, -0.02], 1.0, &quad(5.0, 0.0));
        assert!(r.offset_xy < 1e-9);
        assert!(r.roll_authority.is_infinite());
        assert!(r.pitch_authority.is_infinite());
        assert!(r.pendulum_margin > 0.0, "COM below plane → pendulum-stable");
        assert!(r.controllable);
    }

    #[test]
    fn small_offset_stays_controllable() {
        // 5 mm lateral offset, strong props.
        let r = cg_envelope([0.005, 0.0, -0.02], 1.0, &quad(8.0, 0.0));
        assert!(r.pitch_authority > 1.0, "pitch authority {} should exceed 1", r.pitch_authority);
        assert!(r.controllable);
    }

    #[test]
    fn huge_offset_loses_control() {
        // COM shifted 8 cm — beyond the ring — props can't hold it.
        let r = cg_envelope([0.08, 0.0, -0.02], 2.0, &quad(6.0, 0.0));
        assert!(r.pitch_authority < 1.0, "should be uncontrollable in pitch");
        assert!(!r.controllable);
    }

    #[test]
    fn top_heavy_is_uncontrollable() {
        // COM ABOVE the rotor plane → top-heavy, actively unstable.
        let r = cg_envelope([0.0, 0.0, 0.03], 1.0, &quad(8.0, 0.0));
        assert!(r.pendulum_margin < 0.0);
        assert!(!r.controllable, "top-heavy must be flagged uncontrollable");
    }
}
