//! Frontend Bevy : rendu GPU 2D interactif.
//!
//! Fenêtre Bevy (winit + wgpu), traces des trajectoires et panneau d'état.
//!
//! Contrôles clavier :
//! - `1/2/3/4`   changer de preset
//! - `E/V/L`     Euler / Verlet / Leapfrog
//! - `+` / `-`   accélérer / ralentir le temps
//! - `R`         remettre à zéro (recharger le preset)
//! - `P`         pause ; `Échap` quitter

use bevy::prelude::*;
use gravity::integrator::IntegratorKind;
use gravity::preset::{build_preset, Preset};
use gravity::simulation::{Simulation, SimulationConfig};

/// Nombre de points de trace gardés par corps.
const TRAIL_LEN: usize = 160;
const BODY_COLOR: Color = Color::srgb(0.18, 0.43, 0.32);
const TRAIL_COLOR: Color = Color::srgba(0.57, 0.37, 0.31, 0.5);
const BG: Color = Color::srgb(0.96, 0.94, 0.90);

#[derive(Resource)]
struct SimState {
    sim: Simulation,
    preset: Preset,
    paused: bool,
    speed: usize,
    body_entities: Vec<Entity>,
    trail_entities: Vec<Entity>,
}

impl SimState {
    fn new() -> Self {
        Self {
            sim: Simulation::new(
                SimulationConfig::default(),
                build_preset(Preset::SolarSystem),
            ),
            preset: Preset::SolarSystem,
            paused: false,
            speed: 2,
            body_entities: Vec::new(),
            trail_entities: Vec::new(),
        }
    }

    /// Reconstruit la simulation, invalide les sprites (recréés ensuite).
    fn rebuild(&mut self) {
        let config = SimulationConfig {
            dt: self.sim.config.dt,
            integrator: self.sim.config.integrator,
            ..Default::default()
        };
        self.sim = Simulation::new(config, build_preset(self.preset));
        self.body_entities.clear();
        self.trail_entities.clear();
    }
}

#[derive(Component)]
struct BodySprite;

#[derive(Component)]
struct TrailSprite;

#[derive(Component)]
struct StatusLabel;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gravity Simulation — Bevy (GPU)".into(),
                resolution: Vec2::new(1280.0_f32, 800.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(BG))
        .insert_resource(SimState::new())
        .add_systems(Startup, setup)
        .add_systems(Update, (physics, render_trails, keyboard, ui_update))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: 15.0,
                color: Color::srgb(0.15, 0.15, 0.15),
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        }),
        StatusLabel,
    ));
}

/// Crée les sprites de corps et de traces si l'état l'exige (init ou reset).
fn rebuild_sprites(
    commands: &mut Commands,
    state: &mut SimState,
    bodies: &[gravity::simulation::Body],
) {
    if state.body_entities.is_empty() {
        for body in bodies {
            let r = body.radius.unwrap_or(0.3).max(0.5).clamp(0.5, 4.0);
            let e = commands
                .spawn(SpriteBundle {
                    sprite: Sprite {
                        color: BODY_COLOR,
                        custom_size: Some(Vec2::splat(r as f32 * 8.0)),
                        ..default()
                    },
                    ..default()
                })
                .id();
            state.body_entities.push(e);
        }
    }
    if state.trail_entities.is_empty() {
        for _ in 0..bodies.len().max(1) * TRAIL_LEN {
            let e = commands
                .spawn((
                    SpriteBundle {
                        sprite: Sprite {
                            color: TRAIL_COLOR,
                            custom_size: Some(Vec2::splat(2.0)),
                            ..default()
                        },
                        ..default()
                    },
                    TrailSprite,
                ))
                .id();
            state.trail_entities.push(e);
        }
    }
}

/// Avance la physique puis positionne les sprites de corps.
#[allow(clippy::type_complexity)]
fn physics(
    mut state: ResMut<SimState>,
    mut commands: Commands,
    mut body_q: Query<(&mut Transform, &mut Sprite), (With<BodySprite>, Without<TrailSprite>)>,
) {
    if !state.paused {
        for _ in 0..state.speed {
            state.sim.step();
        }
        state.sim.merge_collisions();
    }

    let bodies = state.sim.bodies.clone();
    rebuild_sprites(&mut commands, &mut state, &bodies);

    for (e, body) in state.body_entities.iter().zip(bodies.iter()) {
        if let Ok((mut tf, mut sp)) = body_q.get_mut(*e) {
            let r = body.radius.unwrap_or(0.3).max(0.5).clamp(0.5, 4.0);
            sp.custom_size = Some(Vec2::splat(r as f32 * 8.0));
            tf.translation = Vec3::new(body.pos.x as f32, body.pos.y as f32, 0.0);
        }
    }
}

/// Trace les dernières positions de chaque corps sous forme de points.
fn render_trails(state: Res<SimState>, mut trail_q: Query<&mut Transform, With<TrailSprite>>) {
    let history = &state.sim.history;
    let bodies = state.sim.bodies.len();
    if bodies == 0 {
        return;
    }
    let step = 4usize;
    let mut iter = trail_q.iter_mut();
    for i in 0..bodies * TRAIL_LEN {
        let Some(mut tf) = iter.next() else { break };
        let body_idx = i / TRAIL_LEN;
        let trail_idx = i % TRAIL_LEN;
        let snap_len = history.len();
        let k = snap_len.saturating_sub(1).saturating_sub(trail_idx * step);
        let pt = history
            .get(k)
            .and_then(|snap| snap.get(body_idx))
            .copied()
            .unwrap_or_default();
        tf.translation = Vec3::new(pt.x as f32, pt.y as f32, 0.0);
    }
}

fn keyboard(
    mut state: ResMut<SimState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: EventWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::Digit1) {
        state.preset = Preset::SolarSystem;
        state.rebuild();
    }
    if keys.just_pressed(KeyCode::Digit2) {
        state.preset = Preset::Binary;
        state.rebuild();
    }
    if keys.just_pressed(KeyCode::Digit3) {
        state.preset = Preset::FigureEight;
        state.rebuild();
    }
    if keys.just_pressed(KeyCode::Digit4) {
        state.preset = Preset::RandomCluster;
        state.rebuild();
    }
    if keys.just_pressed(KeyCode::KeyE) {
        state.sim.config.integrator = IntegratorKind::Euler;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        state.sim.config.integrator = IntegratorKind::Verlet;
    }
    if keys.just_pressed(KeyCode::KeyL) {
        state.sim.config.integrator = IntegratorKind::Leapfrog;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        state.paused = !state.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        state.rebuild();
    }
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        state.speed = (state.speed + 1).min(64);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        state.speed = state.speed.saturating_sub(1).max(1);
    }
}

fn ui_update(state: Res<SimState>, mut text_q: Query<&mut Text, With<StatusLabel>>) {
    let label = format!(
        "{}\n{}\nCorps: {}   Vitesse: x{}\n{} · É = {:.3} · |p| = {:.4}\n\n1-4 preset · E/V/L intégrateur · +/- vitesse\nR reset · P pause · Échap quitter",
        state.preset.label(),
        state.sim.config.integrator.label(),
        state.sim.bodies.len(),
        state.speed,
        if state.paused { "PAUSE" } else { "en cours" },
        state.sim.total_energy(),
        state.sim.total_momentum().norm(),
    );
    for mut text in text_q.iter_mut() {
        text.sections[0].value = label.clone();
    }
}
