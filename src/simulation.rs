//! Corps célestes, moteur N-corps, énergie et momentum.

use crate::integrator::make_integrator;
use crate::math::{Vec2, G};

/// Un corps ponctuel : position, vitesse, masse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub pos: Vec2,
    pub vel: Vec2,
    pub mass: f64,
    /// Rayon de collision optionnel (pour le merging). `None` = ponctuel.
    pub radius: Option<f64>,
    pub name: &'static str,
}

impl Body {
    pub fn new(pos: Vec2, vel: Vec2, mass: f64, radius: Option<f64>, name: &'static str) -> Self {
        Body {
            pos,
            vel,
            mass,
            radius,
            name,
        }
    }
}

/// Une collision détectée entre deux corps (fusion).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Collision {
    pub a: usize,
    pub b: usize,
}

/// Configuration du moteur.
#[derive(Debug, Clone, Copy)]
pub struct SimulationConfig {
    /// Pas de temps physique (unités adimensionnées).
    pub dt: f64,
    /// Graine du champ gravitationnel (offset anti-dégénérescence).
    pub softening: f64,
    /// Type d'intégrateur.
    pub integrator: crate::integrator::IntegratorKind,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        SimulationConfig {
            dt: 0.01,
            softening: 0.05,
            integrator: crate::integrator::IntegratorKind::Verlet,
        }
    }
}

/// État complet d'une simulation.
#[derive(Debug, Clone)]
pub struct Simulation {
    pub config: SimulationConfig,
    pub bodies: Vec<Body>,
    /// Piste les positions successives pour tracer les trajectoires.
    pub history: Vec<Vec<Vec2>>,
    pub step_count: u64,
}

/// Accélération gravitationnelle subie par le corps `i` de la liste, avec un
/// offset de lissage `softening` pour éviter les dégénérescences à courte
/// distance.
fn acceleration(bodies: &[Body], i: usize, softening: f64) -> Vec2 {
    let mut a = Vec2::ZERO;
    for (j, other) in bodies.iter().enumerate() {
        if i == j {
            continue;
        }
        let r = other.pos - bodies[i].pos;
        let r2 = r.norm_squared() + softening * softening;
        let inv_r3 = 1.0 / (r2 * r2.sqrt());
        a += r * (G * other.mass * inv_r3);
    }
    a
}

impl Simulation {
    pub fn new(config: SimulationConfig, bodies: Vec<Body>) -> Self {
        // `dt` sert de multiplicateur dans les trois intégrateurs (jamais de
        // diviseur), donc `dt=0`/négatif/NaN ne provoque pas de crash
        // immédiat — mais produit silencieusement une simulation figée
        // (dt=0), qui remonte le temps (dt<0), ou qui diverge en NaN sans
        // jamais lever d'erreur (dt=NaN, ex. `f64::parse` accepte la chaîne
        // "nan"). On préfère refuser franchement à la construction plutôt
        // que de laisser tourner un état physiquement dénué de sens.
        assert!(
            config.dt.is_finite() && config.dt > 0.0,
            "SimulationConfig::dt doit être fini et strictement positif (reçu : {})",
            config.dt
        );
        let history = vec![bodies.iter().map(|b| b.pos).collect()];
        Simulation {
            config,
            bodies,
            history,
            step_count: 0,
        }
    }

    /// Fait avancer la simulation d'un pas de temps `dt`.
    pub fn step(&mut self) {
        let mut integrator = make_integrator(self.config.integrator);
        let softening = self.config.softening;
        let accel = |bodies: &[Body], i: usize| acceleration(bodies, i, softening);
        integrator.step(&mut self.bodies, self.config.dt, &accel);

        self.history
            .push(self.bodies.iter().map(|b| b.pos).collect());
        // Garde un historique borné (évite la fuite mémoire sur les runs longs).
        const MAX_HISTORY: usize = 1024;
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.step_count += 1;
    }

    /// Détecte et fusionne les corps en contact (si des rayons sont définis).
    ///
    /// Retourne les collisions traitées. La fusion conserve le momentum
    /// linéaire total.
    pub fn merge_collisions(&mut self) -> Vec<Collision> {
        let mut collisions = Vec::new();
        let mut i = 0;
        loop {
            if i >= self.bodies.len() {
                break;
            }
            let mut merged = false;
            let mut j = 0;
            while j < self.bodies.len() {
                if i == j {
                    j += 1;
                    continue;
                }
                let a = self.bodies[i];
                let b = self.bodies[j];
                let ri = a.radius.unwrap_or(f64::INFINITY);
                let rj = b.radius.unwrap_or(f64::INFINITY);
                let touch = (a.pos - b.pos).norm() <= ri + rj;
                if touch {
                    collisions.push(Collision { a: i, b: j });
                    // Fusionne b dans a (conserve le momentum linéaire).
                    let total_mass = a.mass + b.mass;
                    let new_pos = (a.pos * a.mass + b.pos * b.mass) / total_mass;
                    let new_vel = (a.vel * a.mass + b.vel * b.mass) / total_mass;
                    let new_radius = a
                        .radius
                        .zip(b.radius)
                        .map(|(ra, rb)| (ra.powi(3) + rb.powi(3)).cbrt());
                    self.bodies[i] = Body {
                        pos: new_pos,
                        vel: new_vel,
                        mass: total_mass,
                        radius: new_radius,
                        name: a.name,
                    };
                    self.bodies.remove(j);
                    merged = true;
                    break;
                }
                j += 1;
            }
            if !merged {
                i += 1;
            }
        }
        collisions
    }

    /// Énergie cinétique totale.
    pub fn kinetic_energy(&self) -> f64 {
        self.bodies
            .iter()
            .map(|b| 0.5 * b.mass * b.vel.norm_squared())
            .sum()
    }

    /// Énergie potentielle gravitationnelle totale.
    pub fn potential_energy(&self) -> f64 {
        let mut pe = 0.0;
        for i in 0..self.bodies.len() {
            for j in (i + 1)..self.bodies.len() {
                let r = (self.bodies[i].pos - self.bodies[j].pos).norm();
                pe -= G * self.bodies[i].mass * self.bodies[j].mass / r;
            }
        }
        pe
    }

    /// Énergie mécanique totale (doit se conserver pour un intégrateur
    /// symplectique sur de courtes périodes).
    pub fn total_energy(&self) -> f64 {
        self.kinetic_energy() + self.potential_energy()
    }

    /// Momentum linéaire total (doit être conservé).
    pub fn total_momentum(&self) -> Vec2 {
        let mut p = Vec2::ZERO;
        for b in &self.bodies {
            p += b.vel * b.mass;
        }
        p
    }
}
