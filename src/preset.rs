//! Présets de simulation rechargeables depuis l'interface.
//!
//! Paramètres choisis pour des orbites **lentes** (période ≈ 8–25 unités de
//! temps) : à `dt = 0.01`, chaque orbite couvre plusieurs centaines de pas,
//! ce qui garantit la stabilité du Velocity Verlet ET un mouvement visible
//! et lisible à l'écran.

use crate::math::{Vec2, G};
use crate::simulation::Body;

/// Une simulation prête à l'emploi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    SolarSystem,
    Binary,
    FigureEight,
    RandomCluster,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::SolarSystem,
        Preset::Binary,
        Preset::FigureEight,
        Preset::RandomCluster,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Preset::SolarSystem => "Système solaire",
            Preset::Binary => "Étoile binaire + planète",
            Preset::FigureEight => "8 de chiffre (3 corps)",
            Preset::RandomCluster => "Amas aléatoire",
        }
    }
}

pub fn presets() -> Vec<(Preset, Vec<Body>)> {
    Preset::ALL.iter().map(|p| (*p, build_preset(*p))).collect()
}

/// Construit le jeu de corps pour un preset.
pub fn build_preset(preset: Preset) -> Vec<Body> {
    match preset {
        Preset::SolarSystem => solar_system(),
        Preset::Binary => binary(),
        Preset::FigureEight => figure_eight(),
        Preset::RandomCluster => random_cluster(),
    }
}

/// Étoile centrale massive + deux planètes en orbite circulaire.
///
/// Vitesse circulaire `v = sqrt(G·M/r)` ; masses planétaires minuscules
/// (ratio ≈ 400–1200:1) pour des orbites quasi-kepleriennes stables, et des
/// orbites lentes (périodes ~6 et ~15 u.t.).
fn solar_system() -> Vec<Body> {
    let m_star = 60.0;
    let star = Body::new(Vec2::ZERO, Vec2::ZERO, m_star, Some(0.7), "étoile");

    fn planet(r: f64, mass: f64, radius: f64, name: &'static str, m_star: f64) -> Body {
        let v = (G * m_star / r).sqrt();
        Body::new(
            Vec2::new(r, 0.0),
            Vec2::new(0.0, v),
            mass,
            Some(radius),
            name,
        )
    }

    vec![
        star,
        planet(8.0, 0.05, 0.3, "planète 1", m_star),
        planet(15.0, 0.1, 0.38, "planète 2", m_star),
    ]
}

/// Deux étoiles en orbite circulaire l'une autour de l'autre, plus une planète
/// éloignée autour du barycentre.
fn binary() -> Vec<Body> {
    let m1 = 6.0;
    let m2 = 4.0;
    let m3 = 0.05;
    let sep = 3.0;
    let total = m1 + m2;

    let r1 = sep * m2 / total;
    let r2 = sep * m1 / total;
    let w = (G * total / sep.powi(3)).sqrt();

    let star1 = Body::new(
        Vec2::new(-r1, 0.0),
        Vec2::new(0.0, -w * r1),
        m1,
        Some(0.6),
        "étoile A",
    );
    let star2 = Body::new(
        Vec2::new(r2, 0.0),
        Vec2::new(0.0, w * r2),
        m2,
        Some(0.5),
        "étoile B",
    );

    // Planète loin, orbite quasi circulaire autour du barycentre.
    let rp = 12.0;
    let vp = (G * total / rp).sqrt();
    let planet = Body::new(
        Vec2::new(rp, 0.0),
        Vec2::new(0.0, vp),
        m3,
        Some(0.22),
        "planète",
    );

    vec![star1, star2, planet]
}

/// Solution « 8 de chiffre » des trois corps égaux (Chenciner–Montgomery).
///
/// Conditions initiales classiques valides pour G = 1, les vitesses étant
/// ré-échelonnées par √G pour rester une solution exacte à G = 10.
fn figure_eight() -> Vec<Body> {
    let m = 1.0;
    let s = 0.97000436;
    let k = 0.24308753;
    let svel = 0.4662036850;
    let kvel = 0.4323657300;
    let sq = G.sqrt(); // ≈ √10
    let p1 = Body::new(
        Vec2::new(-s, k),
        Vec2::new(svel * sq, kvel * sq),
        m,
        Some(0.3),
        "b1",
    );
    let p2 = Body::new(
        Vec2::new(s, -k),
        Vec2::new(svel * sq, -kvel * sq),
        m,
        Some(0.3),
        "b2",
    );
    let p3 = Body::new(
        Vec2::ZERO,
        Vec2::new(-2.0 * svel * sq, 0.0),
        m,
        Some(0.3),
        "b3",
    );
    vec![p1, p2, p3]
}

/// Amas de corps de masses/positions pseudo-aléatoires (déterministe).
fn random_cluster() -> Vec<Body> {
    let mut seed: u64 = 0x1234_5678_9abc_def0;
    let mut rng = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 32) as f64 / u32::MAX as f64
    };

    let mut bodies = Vec::new();
    let n = 12;
    for _ in 0..n {
        let angle = rng() * std::f64::consts::TAU;
        let rad = 3.0 + 8.0 * rng();
        let pos = Vec2::new(angle.cos(), angle.sin()) * rad;
        // Rotation douce autour du centre.
        let v = 0.3 + 1.2 * rng();
        let vel = Vec2::new(-pos.y, pos.x) * (v / rad.max(1.0));
        let mass = 0.2 + 1.5 * rng();
        bodies.push(Body::new(pos, vel, mass, Some(0.18 + 0.1 * rng()), ""));
    }
    for (idx, b) in bodies.iter_mut().enumerate() {
        b.name = Box::leak(format!("corps {idx}").into_boxed_str());
    }
    bodies
}
