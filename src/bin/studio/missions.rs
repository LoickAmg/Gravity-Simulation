//! Missions : de petits défis pour apprendre la mécanique orbitale en jouant.

use gravity::{Body, Vec2, G};

use crate::scenes::{star, Kind, Spawn};
use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mission {
    FirstOrbit,
    Moon,
    AroundTheStar,
}

impl Mission {
    pub const ALL: [Mission; 3] = [Mission::FirstOrbit, Mission::Moon, Mission::AroundTheStar];

    pub fn title(self) -> &'static str {
        match self {
            Mission::FirstOrbit => "Première orbite",
            Mission::Moon => "Une lune pour la planète",
            Mission::AroundTheStar => "Le tour de l'étoile",
        }
    }

    pub fn goal(self) -> &'static str {
        match self {
            Mission::FirstOrbit => {
                "Glisse depuis un point pour lancer une planète : elle doit faire un tour complet de l'étoile sans s'écraser ni s'échapper."
            }
            Mission::Moon => {
                "Place une lune autour de la planète bleue : un tour complet autour d'elle, sans la percuter ni partir vers l'étoile."
            }
            Mission::AroundTheStar => {
                "Lance la sonde depuis la base (à gauche) jusqu'à la cible verte. La ligne droite passe par l'étoile : laisse la gravité courber ta trajectoire !"
            }
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            Mission::FirstOrbit => {
                "Astuce : lance perpendiculairement à la direction de l'étoile. Trop lent, la planète tombe ; trop vite, elle s'enfuit."
            }
            Mission::Moon => {
                "Astuce : la lune doit suivre la planète, puis tourner autour d'elle. Lance-la tout près de la planète, dans le même sens que son orbite, avec un petit supplément de vitesse."
            }
            Mission::AroundTheStar => {
                "Astuce : vise au-dessus ou en dessous de l'étoile. Plus tu passes près, plus la trajectoire se courbe."
            }
        }
    }

    pub fn scene(self) -> Vec<Spawn> {
        match self {
            Mission::FirstOrbit => vec![star(Vec2::ZERO, 60.0, 0.9, "Soleil")],
            Mission::Moon => {
                let r = 13.0;
                let v = (G * 60.0 / r).sqrt();
                vec![
                    star(Vec2::ZERO, 60.0, 0.9, "Soleil"),
                    Spawn {
                        body: Body::new(Vec2::new(r, 0.0), Vec2::new(0.0, v), 4.0, Some(0.45), ""),
                        kind: Kind::Planet,
                        color: Color32::from_rgb(76, 201, 240),
                        name: "Planète bleue".into(),
                    },
                ]
            }
            Mission::AroundTheStar => vec![star(Vec2::ZERO, 60.0, 1.6, "Soleil")],
        }
    }

    /// Base de lancement imposée (la sonde part toujours de là).
    pub fn launch_pad(self) -> Option<Vec2> {
        match self {
            Mission::AroundTheStar => Some(Vec2::new(-24.0, 0.0)),
            _ => None,
        }
    }

    pub fn target(self) -> Option<(Vec2, f64)> {
        match self {
            Mission::AroundTheStar => Some((Vec2::new(24.0, 0.0), 2.6)),
            _ => None,
        }
    }

    /// Vitesse de lancement maximale (en unités par unité de temps).
    pub fn max_speed(self) -> f64 {
        match self {
            Mission::AroundTheStar => 14.0,
            _ => 30.0,
        }
    }

    pub fn probe_mass(self) -> f64 {
        match self {
            Mission::Moon => 0.02,
            _ => 0.05,
        }
    }
}

/// Suivi de la tentative en cours.
#[derive(Debug, Clone)]
pub struct Attempt {
    /// Identifiant du corps lancé.
    pub probe: u64,
    /// Angle parcouru autour du corps de référence (radians, signé).
    pub swept: f64,
    pub last_angle: Option<f64>,
    pub elapsed: f64,
}

pub enum Verdict {
    Running,
    Success(&'static str),
    Failure(&'static str),
}

impl Attempt {
    pub fn new(probe: u64) -> Self {
        Attempt {
            probe,
            swept: 0.0,
            last_angle: None,
            elapsed: 0.0,
        }
    }

    /// Met à jour la tentative. `probe` = position du corps lancé (None s'il a disparu),
    /// `center` = corps de référence (étoile ou planète).
    pub fn update(
        &mut self,
        mission: Mission,
        probe: Option<Vec2>,
        center: Option<Vec2>,
        dt: f64,
    ) -> Verdict {
        self.elapsed += dt;
        let Some(p) = probe else {
            return Verdict::Failure(match mission {
                Mission::Moon => {
                    "Collision ! La lune s'est écrasée. Réessaie un peu plus loin ou plus vite."
                }
                _ => "Écrasée ! Elle est tombée sur l'étoile. Lance-la plus vite, ou de plus loin.",
            });
        };
        if let Some((target, radius)) = mission.target() {
            if (p - target).norm() <= radius {
                return Verdict::Success(
                    "Cible atteinte ! Tu as utilisé la gravité pour contourner l'étoile.",
                );
            }
            if p.norm() > 60.0 || self.elapsed > 40.0 {
                return Verdict::Failure(
                    "Raté : la sonde s'est perdue dans l'espace. Ajuste l'angle et la force.",
                );
            }
            return Verdict::Running;
        }
        let Some(c) = center else {
            return Verdict::Failure("Le corps de référence a disparu.");
        };
        let d = p - c;
        let dist = d.norm();
        let limit = if mission == Mission::Moon { 5.0 } else { 60.0 };
        if dist > limit {
            return Verdict::Failure(match mission {
                Mission::Moon => {
                    "La lune s'est échappée : elle n'est plus liée à la planète. Moins vite !"
                }
                _ => "Partie dans l'espace ! Elle allait trop vite pour être retenue.",
            });
        }
        let angle = d.y.atan2(d.x);
        if let Some(prev) = self.last_angle {
            let mut delta = angle - prev;
            if delta > std::f64::consts::PI {
                delta -= std::f64::consts::TAU;
            } else if delta < -std::f64::consts::PI {
                delta += std::f64::consts::TAU;
            }
            self.swept += delta;
        }
        self.last_angle = Some(angle);
        if self.swept.abs() >= std::f64::consts::TAU {
            return Verdict::Success(match mission {
                Mission::Moon => "Bravo ! Ta lune a fait le tour de sa planète.",
                _ => "Bravo ! Un tour complet : ta planète est en orbite.",
            });
        }
        Verdict::Running
    }

    pub fn progress(&self) -> f32 {
        (self.swept.abs() / std::f64::consts::TAU).min(1.0) as f32
    }
}
