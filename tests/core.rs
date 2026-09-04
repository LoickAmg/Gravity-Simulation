//! Tests du moteur N-corps : conservation, stabilité orbitale, collisions.

use gravity::integrator::IntegratorKind;
use gravity::math::Vec2;
use gravity::preset::{build_preset, Preset};
use gravity::simulation::{Simulation, SimulationConfig};

fn run(preset: Preset, _steps: u64, dt: f64, integrator: IntegratorKind) -> Simulation {
    let config = SimulationConfig {
        dt,
        integrator,
        ..Default::default()
    };
    Simulation::new(config, build_preset(preset))
}

/// Le momentum linéaire total doit être exactement conservé par tous les
/// intégrateurs (symétrie de la loi d'interaction).
#[test]
fn momentum_is_conserved_all_integrators() {
    for &kind in &IntegratorKind::ALL {
        let mut sim = run(Preset::SolarSystem, 0, 0.01, kind);
        let p0 = sim.total_momentum();
        for _ in 0..2000 {
            sim.step();
        }
        let p1 = sim.total_momentum();
        assert!(
            (p1 - p0).norm() < 1e-6,
            "{kind:?} : momentum dérivé de {p0:?} à {p1:?}"
        );
    }
}

/// Pour un intégrateur symplectique (Verlet/Leapfrog), l'énergie doit être
/// quasi-conservée sur une durée raisonnable (≤ 1 % de dérive).
#[test]
fn verlet_conserves_energy() {
    let mut sim = run(Preset::SolarSystem, 0, 0.01, IntegratorKind::Verlet);
    let e0 = sim.total_energy();
    for _ in 0..2000 {
        sim.step();
    }
    let e1 = sim.total_energy();
    let drift = ((e1 - e0).abs() / e0.abs()).abs();
    assert!(drift < 0.01, "dérive énergie trop grande : {drift:.3}");
}

/// Les planètes d'un système circulaire doivent rester sur une orbite bornée
/// (ne pas s'échapper) sur plusieurs périodes.
#[test]
fn planets_stay_bound() {
    let mut sim = run(Preset::SolarSystem, 0, 0.01, IntegratorKind::Verlet);
    // Période orbitale ≈ 2π·r/v ; pour une planète à r=9 (v=sqrt(10*100/9)≈10.5)
    // T ≈ 5.4 u.t. → 2000 pas × 0.01 = 20 u.t. ≈ 3,7 périodes.
    let r0 = sim.bodies[1].pos.norm();
    for _ in 0..4000 {
        sim.step();
    }
    for b in &sim.bodies[1..] {
        let r = b.pos.norm();
        assert!(
            r < r0 * 2.5,
            "la planète '{}' s'est échappée : r={r} (r0={r0})",
            b.name
        );
    }
}

/// La vitesse d'orbite circulaire doit maintenir un rayon quasi constant pour
/// un corps isolé autour d'une masse centrale (test 2-corps).
#[test]
fn circular_orbit_radius_stable() {
    // Régime lent : M=10, r=10 → v=sqrt(GM/r)=sqrt(100)=10 ; période T≈6,3 u.t.
    // avec dt=0.01 → ~630 pas/orbite, très stable.
    let m_star = 10.0;
    let r = 10.0;
    let v = (gravity::math::G * m_star / r).sqrt();
    use gravity::simulation::Body;
    let bodies = vec![
        Body::new(Vec2::ZERO, Vec2::ZERO, m_star, None, "centrale"),
        Body::new(
            Vec2::new(r, 0.0),
            Vec2::new(0.0, v),
            0.05,
            None,
            "compagnon",
        ),
    ];
    let mut sim = Simulation::new(SimulationConfig::default(), bodies);
    let mut max_dev: f64 = 0.0;
    for _ in 0..3000 {
        sim.step();
        let rr = sim.bodies[1].pos.norm();
        max_dev = max_dev.max((rr - r).abs());
    }
    assert!(
        max_dev < r * 0.08,
        "rayon a dérivé de {max_dev:.4} (max tol 8 % de r)"
    );
}

/// Les collisions fusionnent les corps et conservent le momentum.
#[test]
fn collision_merges_and_conserves_momentum() {
    use gravity::simulation::Body;
    let bodies = vec![
        Body::new(Vec2::new(0.0, 0.0), Vec2::ZERO, 1.0, Some(0.5), "a"),
        Body::new(Vec2::new(0.9, 0.0), Vec2::ZERO, 1.0, Some(0.5), "b"),
    ];
    let mut sim = Simulation::new(SimulationConfig::default(), bodies);
    let p0 = sim.total_momentum();
    let collisions = sim.merge_collisions();
    assert_eq!(collisions.len(), 1);
    assert_eq!(sim.bodies.len(), 1);
    assert!((sim.total_momentum() - p0).norm() < 1e-9);
    assert!((sim.bodies[0].mass - 2.0).abs() < 1e-9);
}

/// Euler, comme attendu, n'est pas conservatif : la dérive est bien plus
/// grande que celle de Verlet (on teste l'écart relatif de comportement).
#[test]
fn euler_drifts_more_than_verlet() {
    let e0 = {
        let mut s = run(Preset::SolarSystem, 0, 0.01, IntegratorKind::Euler);
        let e = s.total_energy();
        for _ in 0..2000 {
            s.step();
        }
        (s.total_energy() - e).abs() / e.abs()
    };
    let v0 = {
        let mut s = run(Preset::SolarSystem, 0, 0.01, IntegratorKind::Verlet);
        let e = s.total_energy();
        for _ in 0..2000 {
            s.step();
        }
        (s.total_energy() - e).abs() / e.abs()
    };
    assert!(
        e0 > v0,
        "Euler devrait dériver plus que Verlet (Euler {e0:.3}, Verlet {v0:.3})"
    );
}

/// La figure en 8 doit rester périodique et bornée sur plusieurs cycles.
#[test]
fn figure_eight_stays_bounded() {
    let mut sim = run(Preset::FigureEight, 0, 0.001, IntegratorKind::Verlet);
    let max_radius = sim
        .bodies
        .iter()
        .map(|b| b.pos.norm())
        .fold(0.0f64, f64::max);
    for _ in 0..8000 {
        sim.step();
    }
    for b in &sim.bodies {
        let r = b.pos.norm();
        assert!(
            r < max_radius * 3.0,
            "corps '{}' s'est échappé de la figure-8 : r={r} (max initial {max_radius})",
            b.name
        );
    }
}
