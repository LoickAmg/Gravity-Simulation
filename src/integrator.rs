//! Intégrateurs temporels pour la dynamique N-corps.
//!
//! Trois schémas numériques sont proposés, tous implémentant le trait
//! [`Integrator`]. Le rendu peut permuter de modèle à l'exécution.

use crate::math::Vec2;
use crate::simulation::Body;

/// Un pas de temps pour faire avancer une liste de corps sous l'effet de la
/// gravité mutuelle. Le trait est générique sur le type d'état pour rester
/// indépendant du stockage (vecteur ou table).
pub trait Integrator {
    /// Avance d'un pas `dt` tous les corps, in place.
    ///
    /// `a` est la fonction d'accélération d'un corps donné (tirée de tous les
    /// autres), retournant le vecteur d'accélération.
    fn step(&mut self, bodies: &mut [Body], dt: f64, accel: &dyn Fn(&[Body], usize) -> Vec2);
}

/// Identifiant du modèle numéral, pour l'afficher dans l'UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegratorKind {
    Euler,
    Verlet,
    Leapfrog,
}

impl IntegratorKind {
    pub const ALL: [IntegratorKind; 3] = [
        IntegratorKind::Euler,
        IntegratorKind::Verlet,
        IntegratorKind::Leapfrog,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IntegratorKind::Euler => "Euler (explicite)",
            IntegratorKind::Verlet => "Velocity Verlet",
            IntegratorKind::Leapfrog => "Leapfrog / Verlet kick-drift",
        }
    }
}

/// Euler explicite : instable sur de longues périodes, utile pour comparer.
pub struct Euler;
impl Integrator for Euler {
    fn step(&mut self, bodies: &mut [Body], dt: f64, accel: &dyn Fn(&[Body], usize) -> Vec2) {
        // Calcule toutes les accélérations à l'instant t (avant mutation).
        let accelerations: Vec<Vec2> = (0..bodies.len()).map(|i| accel(bodies, i)).collect();
        for (i, body) in bodies.iter_mut().enumerate() {
            let a = accelerations[i];
            body.pos += body.vel * dt + a * (0.5 * dt * dt);
            body.vel += a * dt;
        }
    }
}

/// Velocity Verlet : scheme symplectique stable pour N-corps.
pub struct VelocityVerlet;
impl Integrator for VelocityVerlet {
    fn step(&mut self, bodies: &mut [Body], dt: f64, accel: &dyn Fn(&[Body], usize) -> Vec2) {
        let half_dt = 0.5 * dt;
        // 1) demi-pas de vitesse à partir des accélérations de t.
        let a0: Vec<Vec2> = (0..bodies.len()).map(|i| accel(bodies, i)).collect();
        for (body, a) in bodies.iter_mut().zip(&a0) {
            body.vel += *a * half_dt;
        }
        // 2) pas de position avec la nouvelle vitesse.
        for body in bodies.iter_mut() {
            body.pos += body.vel * dt;
        }
        // 3) demi-pas de vitesse à partir des accélérations de t+dt.
        let a1: Vec<Vec2> = (0..bodies.len()).map(|i| accel(bodies, i)).collect();
        for (body, a) in bodies.iter_mut().zip(&a1) {
            body.vel += *a * half_dt;
        }
    }
}

/// Leapfrog kick-drift-kick (équivalent au Velocity Verlet).
pub struct Leapfrog;
impl Integrator for Leapfrog {
    fn step(&mut self, bodies: &mut [Body], dt: f64, accel: &dyn Fn(&[Body], usize) -> Vec2) {
        VelocityVerlet.step(bodies, dt, accel);
    }
}

/// Fabrique l'intégrateur à partir de son identifiant.
pub fn make_integrator(kind: IntegratorKind) -> Box<dyn Integrator> {
    match kind {
        IntegratorKind::Euler => Box::new(Euler),
        IntegratorKind::Verlet => Box::new(VelocityVerlet),
        IntegratorKind::Leapfrog => Box::new(Leapfrog),
    }
}
