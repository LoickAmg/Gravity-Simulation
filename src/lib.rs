//! Moteur gravitationnel N-corps, indépendant de tout rendu.
//!
//! Ce crate expose :
//! - les types physiques (`Body`, `Vec2`) ;
//! - les intégrateurs (`Euler`, `Verlet`, `Leapfrog`) via le trait `Integrator` ;
//! - le moteur `Simulation` qui fait avancer l'état, mesure l'énergie/le momentum
//!   et garde l'historique des trajectoires ;
//! - des présets (`solar_system`, `binary`, …).
//!
//! Aucune dépendance externe : le rendu (CLI, Pixels, Bevy) est un plugin
//! séparé qui consomme ce moteur.

pub mod integrator;
pub mod math;
pub mod preset;
pub mod simulation;

pub use integrator::{Integrator, IntegratorKind, Leapfrog, VelocityVerlet};
pub use math::{Vec2, G};
pub use preset::{build_preset, presets, Preset};
pub use simulation::{Body, Collision, Simulation, SimulationConfig};
