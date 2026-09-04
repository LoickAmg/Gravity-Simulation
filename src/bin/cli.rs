//! Frontend CLI : simule N pas de temps et affiche l'état, l'énergie et le
//! momentum à intervalles réguliers. Sert aussi de vérif de stabilité
//! (conservation de l'énergie) utilisable sans interface graphique.

use gravity::integrator::IntegratorKind;
use gravity::preset::{build_preset, Preset};
use gravity::simulation::{Simulation, SimulationConfig};

fn usage() -> ! {
    eprintln!(
        "gravity-cli [PRESET] [PAS] [DT] [INTEGRATEUR]\n\
         \n\
         PRESET      : solar | binary | eight | cluster  (défaut: solar)\n\
         PAS         : nombre de pas total               (défaut: 2000)\n\
         DT          : pas de temps                      (défaut: 0.01)\n\
         INTEGRATEUR : euler | verlet | leapfrog         (défaut: verlet)\n\
         \n\
         Exemple : gravity-cli binary 5000 0.01 leapfrog"
    );
    std::process::exit(2);
}

fn parse_preset(s: &str) -> Option<Preset> {
    match s.to_ascii_lowercase().as_str() {
        "solar" => Some(Preset::SolarSystem),
        "binary" => Some(Preset::Binary),
        "eight" | "figureeight" => Some(Preset::FigureEight),
        "cluster" | "random" => Some(Preset::RandomCluster),
        _ => None,
    }
}

fn parse_integrator(s: &str) -> Option<IntegratorKind> {
    match s.to_ascii_lowercase().as_str() {
        "euler" => Some(IntegratorKind::Euler),
        "verlet" => Some(IntegratorKind::Verlet),
        "leapfrog" => Some(IntegratorKind::Leapfrog),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let preset = match args.first().map(String::as_str) {
        Some(s) if parse_preset(s).is_some() => parse_preset(s).unwrap(),
        Some(_) => usage(),
        None => Preset::SolarSystem,
    };
    let steps: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let dt: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.01);
    let integrator = match args.get(3).map(String::as_str) {
        Some(s) if parse_integrator(s).is_some() => parse_integrator(s).unwrap(),
        Some(_) => usage(),
        None => IntegratorKind::Verlet,
    };

    println!(
        "Gravity CLI — preset '{}', pas {}, dt {}, intégrateur {}\n",
        preset.label(),
        steps,
        dt,
        integrator.label()
    );

    let config = SimulationConfig {
        dt,
        integrator,
        ..Default::default()
    };
    let mut sim = Simulation::new(config, build_preset(preset));

    let energy0 = sim.total_energy();
    let momentum0 = sim.total_momentum();
    println!(
        "état initial — {} corps, énergie {:.4}, momentum ({:.4}, {:.4})",
        sim.bodies.len(),
        energy0,
        momentum0.x,
        momentum0.y
    );
    println!("pas            position(repère)  ...");

    let report_every = (steps / 40).max(1);
    for step in 1..=steps {
        sim.step();
        sim.merge_collisions();
        if step % report_every == 0 || step == steps {
            let e = sim.total_energy();
            let p = sim.total_momentum();
            let de = (e - energy0).abs();
            let bodies = sim.bodies.len();
            println!(
                "{step:>6}   énergie {e:.6}  Δ {de:.2e}  momentum ({:.4},{:.4})  corps {bodies}",
                p.x, p.y
            );
        }
    }

    println!("\nposition finale des corps :");
    for b in &sim.bodies {
        println!(
            "  {:<12} pos ({:.3}, {:.3})  v ({:.3}, {:.3})  m {:.3}",
            b.name, b.pos.x, b.pos.y, b.vel.x, b.vel.y, b.mass
        );
    }
}
