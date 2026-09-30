//! Scènes de Gravity Studio : les présets du moteur, plus des galaxies, des anneaux
//! et les décors des missions.

use eframe::egui::Color32;
use gravity::{build_preset, Body, Preset, Vec2, G};

/// Nature d'un corps : décide de son apparence et de son comportement à l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Star,
    Planet,
    BlackHole,
    /// Poussière d'étoiles (galaxies, anneaux) : dessinée sans halo ni traînée. Son rayon de
    /// collision est nul : les grains ne fusionnent qu'avec les astres, jamais entre eux.
    Dust,
    /// Sonde lancée pendant une mission.
    Probe,
}

pub struct Spawn {
    pub body: Body,
    pub kind: Kind,
    pub color: Color32,
    pub name: String,
}

pub const PLANET_COLORS: [Color32; 7] = [
    Color32::from_rgb(76, 201, 240),
    Color32::from_rgb(247, 37, 133),
    Color32::from_rgb(114, 239, 150),
    Color32::from_rgb(255, 170, 60),
    Color32::from_rgb(157, 115, 255),
    Color32::from_rgb(255, 214, 102),
    Color32::from_rgb(90, 140, 255),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Solar,
    Binary,
    FigureEight,
    Galaxy,
    GalaxyCollision,
    Rings,
    Cluster,
    Sandbox,
}

impl Scene {
    pub const ALL: [Scene; 8] = [
        Scene::Solar,
        Scene::Binary,
        Scene::FigureEight,
        Scene::Galaxy,
        Scene::GalaxyCollision,
        Scene::Rings,
        Scene::Cluster,
        Scene::Sandbox,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scene::Solar => "Système solaire",
            Scene::Binary => "Étoile double",
            Scene::FigureEight => "Ballet à trois",
            Scene::Galaxy => "Galaxie spirale",
            Scene::GalaxyCollision => "Collision de galaxies",
            Scene::Rings => "Planète à anneaux",
            Scene::Cluster => "Amas chaotique",
            Scene::Sandbox => "Bac à sable",
        }
    }

    /// Nom court pour les boutons.
    pub fn short_label(self) -> &'static str {
        match self {
            Scene::Galaxy => "Galaxie",
            Scene::GalaxyCollision => "Galaxies en choc",
            Scene::Rings => "Anneaux",
            Scene::Cluster => "Amas",
            other => other.label(),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Scene::Solar => "☀",
            Scene::Binary => "⚛",
            Scene::FigureEight => "∞",
            Scene::Galaxy => "🌀",
            Scene::GalaxyCollision => "💥",
            Scene::Rings => "◎",
            Scene::Cluster => "✨",
            Scene::Sandbox => "✏",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Scene::Solar => "Une étoile et deux planètes sur des orbites presque circulaires.",
            Scene::Binary => "Deux étoiles dansent autour de leur centre de masse ; une planète les entoure.",
            Scene::FigureEight => "Trois corps égaux qui se poursuivent sur un « 8 » parfait (Chenciner–Montgomery).",
            Scene::Galaxy => "Un trou noir central et 700 étoiles en rotation.",
            Scene::GalaxyCollision => "Deux galaxies se frôlent : bras de marée et étoiles éjectées.",
            Scene::Rings => "Une planète géante, ses anneaux de poussière et une lune qui les perturbe.",
            Scene::Cluster => "Douze corps au hasard : collisions, fusions et éjections.",
            Scene::Sandbox => "Une seule étoile. À toi de construire ton système !",
        }
    }

    /// Distance visible conseillée (demi-hauteur de l'écran, en unités du monde).
    pub fn view_radius(self) -> f32 {
        match self {
            Scene::Solar => 18.0,
            Scene::Binary => 15.0,
            Scene::FigureEight => 2.2,
            Scene::Galaxy => 26.0,
            Scene::GalaxyCollision => 48.0,
            Scene::Rings => 12.0,
            Scene::Cluster => 14.0,
            Scene::Sandbox => 20.0,
        }
    }

    /// Adoucissement de la gravité adapté à la scène (plus fort pour les nuages de poussière).
    pub fn softening(self) -> f64 {
        match self {
            Scene::Galaxy | Scene::GalaxyCollision => 0.35,
            Scene::Rings => 0.08,
            _ => 0.05,
        }
    }

    pub fn build(self) -> Vec<Spawn> {
        match self {
            Scene::Solar => from_preset(Preset::SolarSystem),
            Scene::Binary => from_preset(Preset::Binary),
            Scene::FigureEight => from_preset(Preset::FigureEight),
            Scene::Cluster => from_preset(Preset::RandomCluster),
            Scene::Galaxy => galaxy(Vec2::ZERO, Vec2::ZERO, 700, 22.0, 1.0, Color32::from_rgb(150, 210, 255), 1),
            Scene::GalaxyCollision => {
                let mut a = galaxy(Vec2::new(-26.0, -9.0), Vec2::new(2.6, 0.0), 350, 13.0, 1.0, Color32::from_rgb(140, 205, 255), 7);
                let b = galaxy(Vec2::new(26.0, 9.0), Vec2::new(-2.6, 0.0), 350, 13.0, -1.0, Color32::from_rgb(255, 150, 210), 11);
                a.extend(b);
                a
            }
            Scene::Rings => rings(),
            Scene::Sandbox => vec![star(Vec2::ZERO, 60.0, 0.7, "Soleil")],
        }
    }
}

pub fn star(pos: Vec2, mass: f64, radius: f64, name: &str) -> Spawn {
    Spawn {
        body: Body::new(pos, Vec2::ZERO, mass, Some(radius), ""),
        kind: Kind::Star,
        color: Color32::from_rgb(255, 214, 130),
        name: name.to_string(),
    }
}

fn from_preset(preset: Preset) -> Vec<Spawn> {
    let bodies = build_preset(preset);
    let max_mass = bodies.iter().map(|b| b.mass).fold(0.0, f64::max);
    bodies
        .into_iter()
        .enumerate()
        .map(|(i, body)| {
            let is_star = body.mass >= max_mass * 0.3 && body.mass >= 1.0 && preset != Preset::RandomCluster;
            let name = if body.name.is_empty() { format!("corps {}", i + 1) } else { capitalize(body.name) };
            if is_star {
                let color = if i % 2 == 0 { Color32::from_rgb(255, 214, 130) } else { Color32::from_rgb(255, 150, 110) };
                Spawn { body, kind: Kind::Star, color, name }
            } else {
                Spawn { body, kind: Kind::Planet, color: PLANET_COLORS[i % PLANET_COLORS.len()], name }
            }
        })
        .collect()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Petit générateur pseudo-aléatoire déterministe (xorshift).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Galaxie : un trou noir central et un disque d'étoiles en orbite quasi circulaire.
/// `spin` = +1 (sens trigonométrique) ou -1.
fn galaxy(center: Vec2, drift: Vec2, n: usize, radius: f64, spin: f64, tint: Color32, seed: u64) -> Vec<Spawn> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
    let m_core = 140.0;
    let m_star = 0.004;
    let mut out = vec![Spawn {
        body: Body::new(center, drift, m_core, Some(0.55), ""),
        kind: Kind::BlackHole,
        color: Color32::from_rgb(255, 140, 60),
        name: "Trou noir".into(),
    }];
    for i in 0..n {
        // Distribution plus dense au centre, deux bras spiraux légers.
        let u = rng.next();
        let r = 1.6 + radius * u.powf(0.8);
        let arm = if i % 2 == 0 { 0.0 } else { std::f64::consts::PI };
        let theta = arm + r * 0.28 * spin + (rng.next() - 0.5) * 0.7;
        let pos = center + Vec2::new(theta.cos(), theta.sin()) * r;
        let enclosed = m_core + m_star * n as f64 * (r / (radius + 1.6)).min(1.0);
        let v = (G * enclosed / r).sqrt() * (0.97 + 0.06 * rng.next());
        let tangent = Vec2::new(-theta.sin(), theta.cos()) * (v * spin);
        let warm = rng.next();
        let color = if warm < 0.12 {
            Color32::from_rgb(255, 225, 170)
        } else if warm < 0.2 {
            Color32::from_rgb(255, 255, 255)
        } else {
            tint
        };
        out.push(Spawn { body: Body::new(pos, drift + tangent, m_star, Some(0.0), ""), kind: Kind::Dust, color, name: "étoile".into() });
    }
    out
}

fn rings() -> Vec<Spawn> {
    let mut rng = Rng(0xC0FF_EE12_3456_789A);
    let m_planet = 40.0;
    let mut out = vec![Spawn {
        body: Body::new(Vec2::ZERO, Vec2::ZERO, m_planet, Some(1.1), ""),
        kind: Kind::Planet,
        color: Color32::from_rgb(235, 190, 120),
        name: "Géante".into(),
    }];
    for _ in 0..420 {
        let r = 2.2 + 2.6 * rng.next();
        let theta = rng.next() * std::f64::consts::TAU;
        let pos = Vec2::new(theta.cos(), theta.sin()) * r;
        let v = (G * m_planet / r).sqrt();
        let vel = Vec2::new(-theta.sin(), theta.cos()) * v;
        let shade = 170 + (rng.next() * 80.0) as u8;
        out.push(Spawn {
            body: Body::new(pos, vel, 0.0005, Some(0.0), ""),
            kind: Kind::Dust,
            color: Color32::from_rgb(shade, shade - 20, shade - 50),
            name: "poussière".into(),
        });
    }
    let rm = 7.5;
    let vm = (G * m_planet / rm).sqrt();
    out.push(Spawn {
        body: Body::new(Vec2::new(rm, 0.0), Vec2::new(0.0, vm), 0.4, Some(0.25), ""),
        kind: Kind::Planet,
        color: Color32::from_rgb(200, 205, 220),
        name: "Lune".into(),
    });
    out
}

/// Vitesse d'orbite circulaire autour de `center` (masse `mass`) pour un corps placé en `pos`.
pub fn circular_velocity(pos: Vec2, center: Vec2, center_vel: Vec2, mass: f64) -> Vec2 {
    let d = pos - center;
    let r = d.norm().max(0.1);
    let v = (G * mass / r).sqrt();
    center_vel + Vec2::new(-d.y, d.x) * (v / r)
}
