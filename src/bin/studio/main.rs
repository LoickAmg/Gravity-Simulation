//! Gravity Studio : bac à sable gravitationnel interactif (egui).
//!
//! Lance des planètes à la fronde, crée des étoiles et des trous noirs, regarde des
//! galaxies se percuter, et relève des missions de mécanique orbitale. Le calcul
//! physique vient entièrement du moteur `gravity` de ce dépôt.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod missions;
mod scenes;

use std::collections::VecDeque;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Margin, Pos2, Rect, RichText, Sense, Shape,
    Stroke, Vec2 as EVec2,
};
use gravity::{Body, IntegratorKind, Simulation, SimulationConfig, Vec2, G};
use missions::{Attempt, Mission, Verdict};
use scenes::{circular_velocity, Kind, Scene, Spawn, PLANET_COLORS};

// ---------------------------------------------------------------------------- palette

const BG: Color32 = Color32::from_rgb(6, 7, 18);
const PANEL: Color32 = Color32::from_rgb(14, 16, 34);
const PANEL_2: Color32 = Color32::from_rgb(22, 25, 50);
const LINE: Color32 = Color32::from_rgb(40, 44, 84);
const TEXT: Color32 = Color32::from_rgb(228, 232, 255);
const DIM: Color32 = Color32::from_rgb(140, 148, 190);
const CYAN: Color32 = Color32::from_rgb(76, 201, 240);
const VIOLET: Color32 = Color32::from_rgb(157, 115, 255);
const GOLD: Color32 = Color32::from_rgb(255, 209, 102);
const GREEN: Color32 = Color32::from_rgb(114, 239, 150);
const RED: Color32 = Color32::from_rgb(255, 99, 120);

const DT: f64 = 0.01;
const STEPS_PER_FRAME: f64 = 4.0;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Gravity Studio")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([980.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Gravity Studio",
        options,
        Box::new(|cc| Ok(Box::new(Studio::new(&cc.egui_ctx)))),
    )
}

// ---------------------------------------------------------------------------- état

struct Meta {
    id: u64,
    kind: Kind,
    color: Color32,
    name: String,
    trail: VecDeque<Vec2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Launch,
    Star,
    BlackHole,
    Erase,
    Select,
}

impl Tool {
    const ALL: [Tool; 5] = [
        Tool::Launch,
        Tool::Star,
        Tool::BlackHole,
        Tool::Erase,
        Tool::Select,
    ];
    fn label(self) -> &'static str {
        match self {
            Tool::Launch => "🚀  Lancer une planète",
            Tool::Star => "☀  Poser une étoile",
            Tool::BlackHole => "⚫  Poser un trou noir",
            Tool::Erase => "🗑  Effacer",
            Tool::Select => "👆  Sélectionner / suivre",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Tool::Launch => "Glisse depuis un point : la planète part dans la direction opposée, comme une fronde. Un simple clic la met en orbite (si l'option est cochée).",
            Tool::Star => "Clique pour créer une étoile immobile.",
            Tool::BlackHole => "Clique pour créer un trou noir très massif.",
            Tool::Erase => "Clique sur un corps pour le retirer.",
            Tool::Select => "Clique sur un corps pour voir ses infos, double-clique pour que la caméra le suive.",
        }
    }
}

struct Camera {
    center: Vec2,
    /// Pixels par unité du monde.
    zoom: f32,
    follow: Option<u64>,
}

struct Confetti {
    pos: Pos2,
    vel: EVec2,
    color: Color32,
    life: f32,
}

struct Studio {
    sim: Simulation,
    meta: Vec<Meta>,
    next_id: u64,
    scene: Scene,
    mission: Option<Mission>,
    attempt: Option<Attempt>,
    attempts: u32,
    completed: [bool; 3],
    status: Option<(String, Color32, f64)>,

    paused: bool,
    speed: f64,
    time: f64,
    step_accum: f64,
    energy0: f64,
    rebaseline: bool,

    tool: Tool,
    spawn_mass: f64,
    auto_orbit: bool,
    merge: bool,
    trails: bool,
    trail_len: usize,
    grid: bool,
    vectors: bool,
    glow: bool,

    cam: Camera,
    view_radius: f32,
    drag: Option<Vec2>,
    selected: Option<u64>,
    stars_bg: Vec<(f32, f32, f32, f32)>,
    confetti: Vec<Confetti>,
    frame_time: f64,
}

impl Studio {
    fn new(ctx: &egui::Context) -> Self {
        style(ctx);
        let mut seed = 0x1234_5678u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 10_000) as f32 / 10_000.0
        };
        let stars_bg = (0..420)
            .map(|_| (rnd(), rnd(), 0.4 + rnd() * 1.4, 0.15 + rnd() * 0.7))
            .collect();
        let mut studio = Studio {
            sim: Simulation::new(SimulationConfig::default(), Vec::new()),
            meta: Vec::new(),
            next_id: 1,
            scene: Scene::Solar,
            mission: None,
            attempt: None,
            attempts: 0,
            completed: load_progress(),
            status: None,
            paused: false,
            speed: 1.0,
            time: 0.0,
            step_accum: 0.0,
            energy0: 0.0,
            rebaseline: false,
            tool: Tool::Launch,
            spawn_mass: 0.1,
            auto_orbit: true,
            merge: true,
            trails: true,
            trail_len: 220,
            grid: true,
            vectors: false,
            glow: true,
            cam: Camera {
                center: Vec2::ZERO,
                zoom: 20.0,
                follow: None,
            },
            view_radius: 18.0,
            drag: None,
            selected: None,
            stars_bg,
            confetti: Vec::new(),
            frame_time: 0.0,
        };
        studio.load_scene(Scene::Solar);
        // Options de lancement : --scene <n> (1 à 8) ou --mission <n> (1 à 3).
        let args: Vec<String> = std::env::args().collect();
        for pair in args.windows(2) {
            let n: usize = pair[1].parse().unwrap_or(0);
            match pair[0].as_str() {
                "--scene" if (1..=Scene::ALL.len()).contains(&n) => {
                    studio.load_scene(Scene::ALL[n - 1])
                }
                "--mission" if (1..=Mission::ALL.len()).contains(&n) => {
                    studio.load_mission(Mission::ALL[n - 1])
                }
                _ => {}
            }
        }
        studio
    }

    fn reset_world(&mut self, spawns: Vec<Spawn>, softening: f64, view_radius: f32) {
        let integrator = self.sim.config.integrator;
        // Les grains de poussière (masse < 0,005) subissent la gravité sans l'exercer.
        let config = SimulationConfig {
            dt: DT,
            softening,
            integrator,
            test_particle_mass: 0.005,
        };
        self.sim = Simulation::new(config, Vec::new());
        self.sim.max_history = 0;
        self.meta.clear();
        for s in spawns {
            self.add(s);
        }
        self.time = 0.0;
        self.energy0 = self.sim.total_energy();
        self.cam = Camera {
            center: Vec2::ZERO,
            zoom: self.cam.zoom,
            follow: None,
        };
        self.view_radius = view_radius;
        self.cam.zoom = 0.0; // recalculé au premier affichage
        self.selected = None;
        self.drag = None;
        self.attempt = None;
    }

    fn load_scene(&mut self, scene: Scene) {
        self.scene = scene;
        self.mission = None;
        self.status = None;
        self.reset_world(scene.build(), scene.softening(), scene.view_radius());
        self.trails =
            !matches!(scene, Scene::Galaxy | Scene::GalaxyCollision | Scene::Rings) || self.trails;
    }

    fn load_mission(&mut self, mission: Mission) {
        self.mission = Some(mission);
        self.attempts = 0;
        self.tool = Tool::Launch;
        self.auto_orbit = false;
        self.paused = false;
        self.reset_world(
            mission.scene(),
            0.05,
            if mission == Mission::AroundTheStar {
                28.0
            } else {
                20.0
            },
        );
        self.status = Some((mission.goal().to_string(), CYAN, f64::INFINITY));
    }

    fn add(&mut self, s: Spawn) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.sim.bodies.push(s.body);
        self.rebaseline = true;
        self.meta.push(Meta {
            id,
            kind: s.kind,
            color: s.color,
            name: s.name,
            trail: VecDeque::new(),
        });
        id
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.meta.iter().position(|m| m.id == id)
    }

    fn remove(&mut self, index: usize) {
        self.sim.bodies.remove(index);
        self.rebaseline = true;
        let id = self.meta.remove(index).id;
        if self.selected == Some(id) {
            self.selected = None;
        }
        if self.cam.follow == Some(id) {
            self.cam.follow = None;
        }
    }

    /// Corps dont l'attraction domine en `pos` (pour l'orbite automatique).
    fn dominant(&self, pos: Vec2) -> Option<usize> {
        self.sim
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.mass > 0.5)
            .max_by(|(_, a), (_, b)| {
                let fa = a.mass / (a.pos - pos).norm_squared().max(0.01);
                let fb = b.mass / (b.pos - pos).norm_squared().max(0.01);
                fa.total_cmp(&fb)
            })
            .map(|(i, _)| i)
    }

    // ------------------------------------------------------------------ physique

    fn advance(&mut self, frame_dt: f64) {
        if self.paused {
            return;
        }
        self.step_accum += STEPS_PER_FRAME * self.speed * (frame_dt * 60.0).clamp(0.25, 2.0);
        let steps = self.step_accum.floor() as usize;
        self.step_accum -= steps as f64;
        for n in 0..steps.min(64) {
            self.sim.step();
            self.time += DT;
            if self.merge && n % 2 == 0 {
                self.apply_merges();
            }
            if n % 2 == 0 && self.trails {
                for (m, b) in self.meta.iter_mut().zip(&self.sim.bodies) {
                    if m.kind == Kind::Dust {
                        continue;
                    }
                    m.trail.push_back(b.pos);
                    while m.trail.len() > self.trail_len {
                        m.trail.pop_front();
                    }
                }
            }
            self.check_mission(DT);
        }
        // Les corps très lointains sont retirés pour garder la simulation fluide.
        let far = (self.view_radius as f64) * 12.0;
        let mut i = 0;
        while i < self.sim.bodies.len() {
            if self.sim.bodies[i].pos.norm() > far {
                self.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Rejoue les fusions du moteur sur les métadonnées (mêmes indices, même ordre).
    fn apply_merges(&mut self) {
        let before: Vec<(f64, usize)> = self.sim.bodies.iter().map(|b| b.mass).zip(0..).collect();
        let collisions = self.sim.merge_collisions();
        if collisions.is_empty() {
            return;
        }
        // Une fusion est inélastique : l'énergie perdue n'est pas une erreur de calcul,
        // on repart donc de la nouvelle valeur pour mesurer la dérive numérique.
        self.rebaseline = true;
        let mut masses: Vec<f64> = before.iter().map(|(m, _)| *m).collect();
        for c in collisions {
            let (keep, gone) = (c.a, c.b);
            // Le survivant prend l'apparence du plus massif des deux.
            if masses[gone] > masses[keep] {
                let donor = &self.meta[gone];
                let (kind, color, name) = (donor.kind, donor.color, donor.name.clone());
                let m = &mut self.meta[keep];
                m.kind = kind;
                m.color = color;
                m.name = name;
            }
            masses[keep] += masses[gone];
            masses.remove(gone);
            let id = self.meta.remove(gone).id;
            if self.selected == Some(id) {
                self.selected = None;
            }
            if self.cam.follow == Some(id) {
                self.cam.follow = Some(self.meta[if gone < keep { keep - 1 } else { keep }].id);
            }
        }
    }

    fn check_mission(&mut self, dt: f64) {
        let (Some(mission), Some(attempt)) = (self.mission, self.attempt.as_mut()) else {
            return;
        };
        let probe = self
            .meta
            .iter()
            .position(|m| m.id == attempt.probe)
            .map(|i| self.sim.bodies[i].pos);
        let center = match mission {
            Mission::Moon => self
                .meta
                .iter()
                .position(|m| m.name == "Planète bleue")
                .map(|i| self.sim.bodies[i].pos),
            _ => self
                .sim
                .bodies
                .iter()
                .zip(&self.meta)
                .find(|(_, m)| m.kind == Kind::Star)
                .map(|(b, _)| b.pos),
        };
        match attempt.update(mission, probe, center, dt) {
            Verdict::Running => {}
            Verdict::Success(msg) => {
                self.attempt = None;
                let idx = Mission::ALL.iter().position(|m| *m == mission).unwrap_or(0);
                self.completed[idx] = true;
                save_progress(&self.completed);
                self.status = Some((msg.to_string(), GREEN, self.time + 8.0));
                self.celebrate();
            }
            Verdict::Failure(msg) => {
                self.attempt = None;
                self.status = Some((format!("{msg}\n{}", mission.tip()), RED, f64::INFINITY));
            }
        }
    }

    fn celebrate(&mut self) {
        let colors = [CYAN, VIOLET, GOLD, GREEN, Color32::from_rgb(247, 37, 133)];
        for i in 0..160 {
            let a = i as f32 * 0.618 * std::f32::consts::TAU;
            let s = 3.0 + (i % 7) as f32;
            self.confetti.push(Confetti {
                pos: Pos2::new(0.5, 0.35),
                vel: EVec2::new(a.cos() * s, a.sin() * s - 4.0),
                color: colors[i % colors.len()],
                life: 1.0,
            });
        }
    }

    // ------------------------------------------------------------------ création

    fn launch(&mut self, origin: Vec2, velocity: Vec2) {
        let (kind, mass, radius, name, color) = if let Some(m) = self.mission {
            (Kind::Probe, m.probe_mass(), 0.22, "Sonde".to_string(), GOLD)
        } else {
            let n = self.meta.iter().filter(|m| m.kind == Kind::Planet).count();
            let radius = 0.18 + 0.12 * self.spawn_mass.cbrt();
            (
                Kind::Planet,
                self.spawn_mass,
                radius,
                format!("Planète {}", n + 1),
                PLANET_COLORS[n % PLANET_COLORS.len()],
            )
        };
        let id = self.add(Spawn {
            body: Body::new(origin, velocity, mass, Some(radius), ""),
            kind,
            color,
            name,
        });
        if self.mission.is_some() {
            // Une seule sonde à la fois : la précédente est retirée.
            if let Some(prev) = self.attempt.as_ref().and_then(|a| self.index_of(a.probe)) {
                self.remove(prev);
            }
            self.attempts += 1;
            self.attempt = Some(Attempt::new(id));
            self.status = Some((
                format!("Tentative n°{} en cours…", self.attempts),
                CYAN,
                f64::INFINITY,
            ));
        }
    }

    /// Vitesse donnée par la fronde : direction opposée au glissé.
    fn sling_velocity(&self, from: Vec2, to: Vec2) -> Vec2 {
        let mut v = (from - to) * 1.6;
        let max = self.mission.map(|m| m.max_speed()).unwrap_or(40.0);
        let n = v.norm();
        if n > max {
            v = v * (max / n);
        }
        v
    }

    /// Trajectoire prévue d'un corps lancé (avec les corps massifs qui bougent aussi).
    fn predict(&self, pos: Vec2, vel: Vec2) -> Vec<Vec2> {
        let mut bodies: Vec<Body> = self
            .sim
            .bodies
            .iter()
            .filter(|b| b.mass > 0.3)
            .take(24)
            .copied()
            .collect();
        bodies.push(Body::new(pos, vel, 0.001, Some(0.1), ""));
        let config = SimulationConfig {
            dt: DT,
            softening: self.sim.config.softening,
            integrator: IntegratorKind::Verlet,
            test_particle_mass: 0.0,
        };
        let mut sim = Simulation::new(config, bodies);
        sim.max_history = 0;
        let mut out = Vec::with_capacity(160);
        for i in 0..900 {
            sim.step();
            let p = sim.bodies.last().unwrap().pos;
            if i % 6 == 0 {
                out.push(p);
            }
            let hit = sim.bodies[..sim.bodies.len() - 1]
                .iter()
                .any(|b| (b.pos - p).norm() < b.radius.unwrap_or(0.3));
            if hit {
                out.push(p);
                break;
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------- interface

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
        self.frame_time = self.frame_time * 0.9 + dt * 0.1;
        self.shortcuts(&ctx);
        self.advance(dt);

        egui::Panel::left("tools")
            .exact_size(300.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::same(18)),
            )
            .show(ui, |ui| self.left_panel(ui));
        egui::Panel::right("sim")
            .exact_size(280.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::same(18)),
            )
            .show(ui, |ui| self.right_panel(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas(ui));
        ctx.request_repaint();
    }
}

impl Studio {
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(Key::Space) {
                self.paused = !self.paused;
            }
            if i.key_pressed(Key::H) {
                self.cam.center = Vec2::ZERO;
                self.cam.follow = None;
            }
            if i.key_pressed(Key::T) {
                self.trails = !self.trails;
            }
            if i.key_pressed(Key::G) {
                self.grid = !self.grid;
            }
        });
        if ctx.input(|i| i.key_pressed(Key::N)) && self.paused {
            self.sim.step();
            self.time += DT;
        }
        if ctx.input(|i| i.key_pressed(Key::R)) {
            match self.mission {
                Some(m) => self.load_mission(m),
                None => self.load_scene(self.scene),
            }
        }
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("GRAVITY").size(30.0).strong().color(TEXT));
        ui.label(RichText::new("STUDIO").size(30.0).strong().color(CYAN));
        ui.label(RichText::new("Bac à sable de la gravité").color(DIM));
        ui.add_space(14.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let full = ui.available_width() - 6.0;
                section(ui, "SCÈNES");
                egui::Grid::new("scenes")
                    .num_columns(2)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        for (i, scene) in Scene::ALL.iter().enumerate() {
                            let on = self.mission.is_none() && self.scene == *scene;
                            let text =
                                RichText::new(format!("{}  {}", scene.icon(), scene.short_label()))
                                    .size(13.0);
                            let button = egui::Button::new(text)
                                .min_size(EVec2::new((full - 6.0) / 2.0, 34.0))
                                .selected(on);
                            if ui.add(button).on_hover_text(scene.description()).clicked() {
                                self.load_scene(*scene);
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });

                ui.add_space(12.0);
                section(ui, "OUTILS");
                for tool in Tool::ALL {
                    let on = self.tool == tool;
                    let button = egui::Button::new(RichText::new(tool.label()).size(14.0))
                        .min_size(EVec2::new(full, 32.0))
                        .selected(on);
                    if ui.add(button).clicked() {
                        self.tool = tool;
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new(self.tool.hint()).size(12.5).color(DIM));
                if self.tool == Tool::Launch && self.mission.is_none() {
                    ui.add_space(6.0);
                    ui.add(
                        egui::Slider::new(&mut self.spawn_mass, 0.01..=30.0)
                            .logarithmic(true)
                            .text("masse"),
                    );
                    ui.checkbox(&mut self.auto_orbit, "Clic simple : orbite auto");
                }

                ui.add_space(12.0);
                section(ui, "MISSIONS");
                for (i, mission) in Mission::ALL.iter().enumerate() {
                    let done = self.completed[i];
                    let on = self.mission == Some(*mission);
                    let label = format!("{}  {}", if done { "✅" } else { "🎯" }, mission.title());
                    let button = egui::Button::new(RichText::new(label).size(14.0))
                        .min_size(EVec2::new(full, 34.0))
                        .selected(on);
                    if ui.add(button).on_hover_text(mission.goal()).clicked() {
                        self.load_mission(*mission);
                    }
                }
                let done = self.completed.iter().filter(|d| **d).count();
                ui.label(
                    RichText::new(format!("{done} / 3 missions réussies"))
                        .size(12.5)
                        .color(DIM),
                );
            });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        section(ui, "SIMULATION");
        ui.horizontal(|ui| {
            let label = if self.paused {
                "▶  Reprendre"
            } else {
                "⏸  Pause"
            };
            if ui
                .add(
                    egui::Button::new(RichText::new(label).size(14.0))
                        .min_size(EVec2::new(120.0, 32.0)),
                )
                .clicked()
            {
                self.paused = !self.paused;
            }
            if ui
                .add_enabled(
                    self.paused,
                    egui::Button::new("⏭  Pas").min_size(EVec2::new(60.0, 32.0)),
                )
                .clicked()
            {
                self.sim.step();
                self.time += DT;
            }
            if ui
                .add(egui::Button::new("⟲").min_size(EVec2::new(36.0, 32.0)))
                .on_hover_text("Recommencer (R)")
                .clicked()
            {
                match self.mission {
                    Some(m) => self.load_mission(m),
                    None => self.load_scene(self.scene),
                }
            }
        });
        ui.add(
            egui::Slider::new(&mut self.speed, 0.1..=8.0)
                .logarithmic(true)
                .text("vitesse")
                .suffix("×"),
        );

        ui.add_space(6.0);
        ui.label(RichText::new("Méthode de calcul").color(DIM).size(12.5));
        let mut kind = self.sim.config.integrator;
        egui::ComboBox::from_id_salt("integrator")
            .selected_text(kind.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for k in IntegratorKind::ALL {
                    ui.selectable_value(&mut kind, k, k.label());
                }
            });
        if kind != self.sim.config.integrator {
            self.sim.config.integrator = kind;
            self.energy0 = self.sim.total_energy();
        }
        ui.label(
            RichText::new(match kind {
                IntegratorKind::Euler => {
                    "Simple mais imprécis : l'énergie dérive, les orbites s'écartent peu à peu."
                }
                _ => "Symplectique : l'énergie reste stable, les orbites se referment.",
            })
            .size(12.0)
            .color(DIM),
        );
        ui.checkbox(&mut self.merge, "Fusion lors des collisions");

        ui.add_space(12.0);
        section(ui, "AFFICHAGE");
        ui.checkbox(&mut self.trails, "Traînées (T)");
        if self.trails {
            ui.add(egui::Slider::new(&mut self.trail_len, 20..=800).text("longueur"));
        }
        ui.checkbox(&mut self.grid, "Tissu de l'espace-temps (G)");
        ui.checkbox(&mut self.vectors, "Vecteurs vitesse");
        ui.checkbox(&mut self.glow, "Halos lumineux");

        ui.add_space(12.0);
        section(ui, "MESURES");
        let e = self.sim.total_energy();
        if std::mem::take(&mut self.rebaseline) {
            self.energy0 = e;
        }
        let drift = if self.energy0.abs() > 1e-9 {
            (e - self.energy0) / self.energy0.abs() * 100.0
        } else {
            0.0
        };
        stat(ui, "Corps", format!("{}", self.sim.bodies.len()));
        stat(ui, "Temps", format!("{:.1}", self.time));
        stat(ui, "Énergie totale", format!("{e:.1}"));
        let drift_color = if drift.abs() < 1.0 {
            GREEN
        } else if drift.abs() < 10.0 {
            GOLD
        } else {
            RED
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Dérive d'énergie").color(DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{drift:+.2} %"))
                        .color(drift_color)
                        .strong(),
                );
            });
        });
        let p = self.sim.total_momentum();
        stat(ui, "Quantité de mouvement", format!("{:.2}", p.norm()));
        stat(
            ui,
            "Images / s",
            format!("{:.0}", 1.0 / self.frame_time.max(1e-3)),
        );

        if let Some(i) = self.selected.and_then(|id| self.index_of(id)) {
            ui.add_space(12.0);
            section(ui, "CORPS SÉLECTIONNÉ");
            let b = self.sim.bodies[i];
            let (name, color, id) = (
                self.meta[i].name.clone(),
                self.meta[i].color,
                self.meta[i].id,
            );
            egui::Frame::new()
                .fill(PANEL_2)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(EVec2::splat(14.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 6.0, color);
                        ui.label(RichText::new(&name).strong().color(TEXT));
                    });
                    stat(ui, "Masse", format!("{:.3}", b.mass));
                    stat(ui, "Vitesse", format!("{:.2}", b.vel.norm()));
                    if let Some(d) = self.dominant(b.pos).filter(|d| *d != i) {
                        let r = (b.pos - self.sim.bodies[d].pos).norm();
                        let v_esc = (2.0 * G * self.sim.bodies[d].mass / r).sqrt();
                        stat(ui, "Distance à l'astre", format!("{r:.2}"));
                        let bound = (b.vel - self.sim.bodies[d].vel).norm() < v_esc;
                        stat(ui, "Vitesse de libération", format!("{v_esc:.2}"));
                        ui.label(
                            RichText::new(if bound {
                                "Lié : en orbite"
                            } else {
                                "Libre : il s'échappe"
                            })
                            .color(if bound { GREEN } else { GOLD }),
                        );
                    }
                    let following = self.cam.follow == Some(id);
                    if ui
                        .button(if following {
                            "Ne plus suivre"
                        } else {
                            "Suivre avec la caméra"
                        })
                        .clicked()
                    {
                        self.cam.follow = if following { None } else { Some(id) };
                    }
                });
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.label(RichText::new("Espace : pause · R : recommencer · H : recentrer · molette : zoom · clic droit : déplacer").size(11.5).color(DIM));
        });
    }

    // ------------------------------------------------------------------ canevas

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        if self.cam.zoom <= 0.0 {
            self.cam.zoom = rect.width().min(rect.height()) * 0.5 / self.view_radius;
        }
        if let Some(i) = self.cam.follow.and_then(|id| self.index_of(id)) {
            self.cam.center = self.sim.bodies[i].pos;
        }
        let center = rect.center();
        let zoom = self.cam.zoom;
        let cam = self.cam.center;
        let to_screen = |p: Vec2| {
            Pos2::new(
                center.x + ((p.x - cam.x) as f32) * zoom,
                center.y - ((p.y - cam.y) as f32) * zoom,
            )
        };
        let to_world = |s: Pos2| {
            Vec2::new(
                cam.x + ((s.x - center.x) / zoom) as f64,
                cam.y - ((s.y - center.y) / zoom) as f64,
            )
        };

        // --- entrées : zoom, déplacement, outils
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(mouse) = response.hover_pos() {
                    let before = to_world(mouse);
                    self.cam.zoom = (self.cam.zoom * (scroll * 0.0018).exp()).clamp(0.5, 600.0);
                    let z = self.cam.zoom;
                    let after = Vec2::new(
                        self.cam.center.x + ((mouse.x - center.x) / z) as f64,
                        self.cam.center.y - ((mouse.y - center.y) / z) as f64,
                    );
                    if self.cam.follow.is_none() {
                        self.cam.center += before - after;
                    }
                }
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let d = response.drag_delta();
            self.cam.follow = None;
            self.cam.center += Vec2::new((-d.x / zoom) as f64, (d.y / zoom) as f64);
        }
        let pad = self.mission.and_then(|m| m.launch_pad());
        if self.tool == Tool::Launch {
            if response.drag_started_by(egui::PointerButton::Primary) {
                self.drag = Some(pad.unwrap_or_else(|| {
                    response
                        .interact_pointer_pos()
                        .map(to_world)
                        .unwrap_or(Vec2::ZERO)
                }));
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                if let (Some(from), Some(to)) = (
                    self.drag.take(),
                    response.interact_pointer_pos().map(to_world),
                ) {
                    let v = self.sling_velocity(from, to);
                    self.launch(from, v);
                }
            }
        }
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos().map(to_world) {
                self.click(p, zoom);
            }
        }
        if response.double_clicked() {
            if let Some(p) = response.interact_pointer_pos().map(to_world) {
                if let Some(i) = self.pick(p, zoom) {
                    self.cam.follow = Some(self.meta[i].id);
                    self.selected = Some(self.meta[i].id);
                }
            }
        }

        // --- fond
        painter.rect_filled(rect, CornerRadius::ZERO, BG);
        let t = self.time as f32;
        for (x, y, size, bright) in &self.stars_bg {
            let px =
                rect.left() + (x * rect.width() - (cam.x as f32) * 0.6).rem_euclid(rect.width());
            let py =
                rect.top() + (y * rect.height() + (cam.y as f32) * 0.6).rem_euclid(rect.height());
            let twinkle = 0.75 + 0.25 * ((t * 1.3 + x * 40.0).sin());
            let a = (bright * twinkle * 255.0) as u8;
            painter.circle_filled(
                Pos2::new(px, py),
                *size * 0.6,
                Color32::from_rgba_unmultiplied(200, 210, 255, a),
            );
        }

        if self.grid {
            self.draw_grid(&painter, rect, &to_screen, &to_world);
        }

        // --- cible et base de mission
        if let Some(m) = self.mission {
            if let Some((target, r)) = m.target() {
                let c = to_screen(target);
                let pulse = 1.0 + 0.08 * (t * 3.0).sin();
                painter.circle_filled(
                    c,
                    r as f32 * zoom * pulse,
                    Color32::from_rgba_unmultiplied(114, 239, 150, 40),
                );
                painter.circle_stroke(c, r as f32 * zoom * pulse, Stroke::new(2.0, GREEN));
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    "CIBLE",
                    FontId::proportional(13.0),
                    GREEN,
                );
            }
            if let Some(p) = pad {
                let c = to_screen(p);
                painter.circle_stroke(c, 14.0, Stroke::new(2.0, GOLD));
                painter.text(
                    c + EVec2::new(0.0, 26.0),
                    Align2::CENTER_CENTER,
                    "BASE",
                    FontId::proportional(12.0),
                    GOLD,
                );
            }
        }

        // --- traînées
        if self.trails {
            for m in &self.meta {
                if m.trail.len() < 2 || m.kind == Kind::Dust {
                    continue;
                }
                let n = m.trail.len();
                let pts: Vec<Pos2> = m.trail.iter().map(|p| to_screen(*p)).collect();
                for (k, seg) in pts.windows(2).enumerate() {
                    let a = (k as f32 / n as f32).powf(1.5);
                    let c = with_alpha(m.color, (a * 200.0) as u8);
                    painter.line_segment([seg[0], seg[1]], Stroke::new(1.0 + a * 1.6, c));
                }
            }
        }

        // --- corps
        for (b, m) in self.sim.bodies.iter().zip(&self.meta) {
            let p = to_screen(b.pos);
            if !rect.expand(40.0).contains(p) {
                continue;
            }
            let r = (b.radius.unwrap_or(0.2) as f32 * zoom).max(match m.kind {
                Kind::Dust => 1.1,
                Kind::Probe => 3.5,
                _ => 2.5,
            });
            match m.kind {
                Kind::Dust => {
                    painter.circle_filled(p, r, with_alpha(m.color, 210));
                }
                Kind::BlackHole => {
                    if self.glow {
                        for (k, a) in [(4.0, 18u8), (2.6, 40), (1.8, 90)] {
                            painter.circle_filled(p, r * k, with_alpha(m.color, a));
                        }
                    }
                    painter.circle_stroke(
                        p,
                        r * 1.35,
                        Stroke::new(r * 0.35, Color32::from_rgb(255, 170, 80)),
                    );
                    painter.circle_filled(p, r, Color32::BLACK);
                }
                Kind::Star => {
                    if self.glow {
                        for (k, a) in [(5.0, 10u8), (3.2, 22), (2.0, 55), (1.4, 110)] {
                            painter.circle_filled(
                                p,
                                r * k,
                                with_alpha(Color32::from_rgb(255, 170, 70), a),
                            );
                        }
                    }
                    painter.circle_filled(p, r, m.color);
                    painter.circle_filled(p, r * 0.6, Color32::from_rgb(255, 250, 225));
                }
                Kind::Planet | Kind::Probe => {
                    if self.glow {
                        painter.circle_filled(p, r * 2.4, with_alpha(m.color, 30));
                        painter.circle_filled(p, r * 1.5, with_alpha(m.color, 60));
                    }
                    painter.circle_filled(p, r, m.color);
                    // Petit reflet pour donner du volume
                    painter.circle_filled(
                        p + EVec2::new(-r * 0.3, -r * 0.3),
                        r * 0.35,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                    );
                }
            }
            if self.vectors && m.kind != Kind::Dust {
                let tip = p + EVec2::new(b.vel.x as f32, -(b.vel.y as f32)) * zoom * 0.25;
                painter.arrow(p, tip - p, Stroke::new(1.5, GREEN));
            }
            if self.selected == Some(m.id) {
                painter.circle_stroke(p, r + 7.0 + 2.0 * (t * 4.0).sin(), Stroke::new(1.5, CYAN));
                painter.text(
                    p + EVec2::new(0.0, -r - 18.0),
                    Align2::CENTER_CENTER,
                    &m.name,
                    FontId::proportional(13.0),
                    TEXT,
                );
            }
        }

        // --- fronde : trajectoire prévue
        if let (Some(from), Some(mouse)) = (self.drag, response.interact_pointer_pos()) {
            let to = to_world(mouse);
            let v = self.sling_velocity(from, to);
            let a = to_screen(from);
            painter.line_segment([a, mouse], Stroke::new(1.5, with_alpha(TEXT, 90)));
            painter.circle_filled(a, 5.0, GOLD);
            let path: Vec<Pos2> = self.predict(from, v).into_iter().map(to_screen).collect();
            for (k, seg) in path.windows(2).enumerate() {
                if k % 2 == 0 {
                    let fade = 1.0 - k as f32 / path.len() as f32;
                    painter.line_segment(
                        [seg[0], seg[1]],
                        Stroke::new(2.0, with_alpha(GOLD, (fade * 230.0) as u8)),
                    );
                }
            }
            painter.text(
                mouse + EVec2::new(16.0, 16.0),
                Align2::LEFT_TOP,
                format!("vitesse {:.1}", v.norm()),
                FontId::proportional(13.0),
                GOLD,
            );
        }

        // --- bandeau de mission / message
        let mut banner: Option<(String, Color32)> = None;
        if let Some((msg, color, until)) = &self.status {
            if self.time < *until {
                banner = Some((msg.clone(), *color));
            }
        }
        if let Some(m) = self.mission {
            let title = format!("MISSION · {}", m.title().to_uppercase());
            let body = banner.unwrap_or((m.goal().to_string(), CYAN));
            let width = (rect.width() - 80.0).min(640.0);
            let top = Rect::from_min_size(
                Pos2::new(rect.center().x - width / 2.0, rect.top() + 16.0),
                EVec2::new(width, 86.0),
            );
            painter.rect_filled(
                top,
                CornerRadius::same(14),
                Color32::from_rgba_unmultiplied(14, 16, 34, 230),
            );
            painter.rect_stroke(
                top,
                CornerRadius::same(14),
                Stroke::new(1.0, body.1),
                egui::StrokeKind::Inside,
            );
            painter.text(
                top.left_top() + EVec2::new(16.0, 12.0),
                Align2::LEFT_TOP,
                title,
                FontId::proportional(13.0),
                body.1,
            );
            let galley = painter.layout(body.0, FontId::proportional(14.0), TEXT, width - 32.0);
            painter.galley(top.left_top() + EVec2::new(16.0, 32.0), galley, TEXT);
            if let Some(a) = &self.attempt {
                let bar = Rect::from_min_size(
                    top.left_bottom() + EVec2::new(16.0, -10.0),
                    EVec2::new((width - 32.0) * a.progress(), 4.0),
                );
                painter.rect_filled(bar, CornerRadius::same(2), GREEN);
            }
        } else if let Some((msg, color)) = banner {
            painter.text(
                rect.center_top() + EVec2::new(0.0, 24.0),
                Align2::CENTER_TOP,
                msg,
                FontId::proportional(15.0),
                color,
            );
        } else {
            painter.text(
                rect.center_top() + EVec2::new(0.0, 22.0),
                Align2::CENTER_TOP,
                self.scene.description(),
                FontId::proportional(14.0),
                DIM,
            );
        }
        if self.paused {
            painter.text(
                rect.center_bottom() + EVec2::new(0.0, -26.0),
                Align2::CENTER_BOTTOM,
                "⏸  EN PAUSE",
                FontId::proportional(18.0),
                GOLD,
            );
        }

        // --- confettis
        let dt = ui.input(|i| i.unstable_dt).min(0.05);
        self.confetti.retain_mut(|c| {
            c.vel.y += 9.0 * dt;
            c.pos += c.vel * dt * 0.05;
            c.life -= dt * 0.45;
            c.life > 0.0
        });
        for c in &self.confetti {
            let p = Pos2::new(
                rect.left() + c.pos.x * rect.width(),
                rect.top() + c.pos.y * rect.height(),
            );
            painter.rect_filled(
                Rect::from_center_size(p, EVec2::new(6.0, 3.0)),
                CornerRadius::same(1),
                with_alpha(c.color, (c.life * 255.0) as u8),
            );
        }
    }

    fn pick(&self, p: Vec2, zoom: f32) -> Option<usize> {
        let tolerance = 10.0 / zoom as f64;
        self.sim
            .bodies
            .iter()
            .enumerate()
            .filter(|(i, _)| self.meta[*i].kind != Kind::Dust)
            .map(|(i, b)| (i, (b.pos - p).norm() - b.radius.unwrap_or(0.2)))
            .filter(|(_, d)| *d < tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn click(&mut self, p: Vec2, zoom: f32) {
        match self.tool {
            Tool::Launch => {
                if self.mission.is_some() || !self.auto_orbit {
                    return;
                }
                if let Some(d) = self.dominant(p) {
                    let c = self.sim.bodies[d];
                    let v = circular_velocity(p, c.pos, c.vel, c.mass);
                    self.launch(p, v);
                }
            }
            Tool::Star => {
                self.add(scenes::star(p, 40.0, 0.6, "Étoile"));
            }
            Tool::BlackHole => {
                self.add(Spawn {
                    body: Body::new(p, Vec2::ZERO, 250.0, Some(0.5), ""),
                    kind: Kind::BlackHole,
                    color: Color32::from_rgb(255, 120, 60),
                    name: "Trou noir".into(),
                });
            }
            Tool::Erase => {
                if let Some(i) = self.pick(p, zoom) {
                    self.remove(i);
                }
            }
            Tool::Select => {
                self.selected = self.pick(p, zoom).map(|i| self.meta[i].id);
            }
        }
    }

    /// « Tissu de l'espace-temps » : une grille creusée par les masses.
    fn draw_grid(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        to_screen: &dyn Fn(Vec2) -> Pos2,
        to_world: &dyn Fn(Pos2) -> Vec2,
    ) {
        let heavy: Vec<&Body> = self.sim.bodies.iter().filter(|b| b.mass > 1.0).collect();
        let spacing = 34.0;
        let warp = |s: Pos2| -> Pos2 {
            let w = to_world(s);
            let mut d = Vec2::ZERO;
            for b in &heavy {
                let r = b.pos - w;
                let dist = r.norm();
                if dist < 1e-6 {
                    continue;
                }
                let pull =
                    (0.25 * b.mass / (dist * dist + 6.0)).min(dist * 0.85 / dist.max(1e-6)) * dist;
                d += r * (pull.min(dist * 0.85) / dist);
            }
            to_screen(w + d)
        };
        let color = Color32::from_rgba_unmultiplied(110, 100, 220, 38);
        let stroke = Stroke::new(1.0, color);
        let mut x = rect.left() - (rect.left() % spacing);
        while x <= rect.right() {
            let pts: Vec<Pos2> = (0..=((rect.height() / 12.0) as i32))
                .map(|k| warp(Pos2::new(x, rect.top() + k as f32 * 12.0)))
                .collect();
            painter.add(Shape::line(pts, stroke));
            x += spacing;
        }
        let mut y = rect.top() - (rect.top() % spacing);
        while y <= rect.bottom() {
            let pts: Vec<Pos2> = (0..=((rect.width() / 12.0) as i32))
                .map(|k| warp(Pos2::new(rect.left() + k as f32 * 12.0, y)))
                .collect();
            painter.add(Shape::line(pts, stroke));
            y += spacing;
        }
    }
}

// ---------------------------------------------------------------------------- utilitaires

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).size(11.5).strong().color(VIOLET));
    ui.add_space(2.0);
}

fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).color(TEXT));
        });
    });
}

fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL_2;
    v.extreme_bg_color = Color32::from_rgb(10, 11, 26);
    v.faint_bg_color = PANEL_2;
    v.selection.bg_fill = Color32::from_rgb(60, 50, 150);
    v.selection.stroke = Stroke::new(1.0, CYAN);
    v.hyperlink_color = CYAN;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_fill = PANEL_2;
    v.widgets.inactive.weak_bg_fill = PANEL_2;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(36, 40, 80);
    v.widgets.hovered.bg_fill = Color32::from_rgb(36, 40, 80);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(60, 50, 150);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(8);
    }
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = EVec2::new(8.0, 7.0);
        s.spacing.button_padding = EVec2::new(10.0, 6.0);
        s.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.5));
        s.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(14.5));
    });
}

fn progress_file() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME"))?;
    Some(
        std::path::PathBuf::from(base)
            .join("GravityStudio")
            .join("missions.txt"),
    )
}

fn load_progress() -> [bool; 3] {
    let mut done = [false; 3];
    if let Some(text) = progress_file().and_then(|p| std::fs::read_to_string(p).ok()) {
        for (i, c) in text.chars().take(3).enumerate() {
            done[i] = c == '1';
        }
    }
    done
}

fn save_progress(done: &[bool; 3]) {
    if let Some(path) = progress_file() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text: String = done.iter().map(|d| if *d { '1' } else { '0' }).collect();
        let _ = std::fs::write(path, text);
    }
}
