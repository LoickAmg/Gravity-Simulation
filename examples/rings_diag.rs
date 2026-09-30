//! Diagnostic : un disque de poussière autour d'une planète doit rester stable.
use gravity::{Body, Simulation, SimulationConfig, Vec2, G};

fn main() {
    let m = 40.0;
    let mut bodies = vec![Body::new(Vec2::ZERO, Vec2::ZERO, m, Some(1.1), "")];
    for i in 0..200 {
        let r = 2.2 + 2.6 * (i as f64 / 200.0);
        let t = i as f64 * 0.7;
        let v = (G * m / r).sqrt();
        bodies.push(Body::new(Vec2::new(t.cos(), t.sin()) * r, Vec2::new(-t.sin(), t.cos()) * v, 0.0005, Some(0.0), ""));
    }
    let config = SimulationConfig { dt: 0.01, softening: 0.08, ..Default::default() };
    let mut sim = Simulation::new(config, bodies);
    sim.max_history = 0;
    for step in 0..2000 {
        sim.step();
        if step % 2 == 0 {
            sim.merge_collisions();
        }
        if step % 400 == 0 {
            let rmin = sim.bodies[1..].iter().map(|b| b.pos.norm()).fold(f64::MAX, f64::min);
            println!("pas {step}: {} corps, rayon min {rmin:.2}", sim.bodies.len());
        }
    }
}
