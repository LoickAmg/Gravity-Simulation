//! Frontend « Pixels » : rendu 2D sur CPU via `softbuffer`.
//!
//! Fenêtre `winit` + framebuffer `softbuffer`, sans GPU externe. Affiche les
//! corps, leurs traces, et permet de changer de preset / modèle d'intégration /
//! vitesse en direct.
//!
//! Contrôles :
//! - `1/2/3/4`       changer de preset
//! - `E/V/L`         Euler / Verlet / Leapfrog
//! - `+` / `-`       accélérer / ralentir le temps
//! - `R`             remettre à zéro (recharger le preset)
//! - `P`             pause
//! - molette         zoom ; clic-glisser pan
//! - `Échap`         quitter
//!
//! Un panneau d'état (HUD) en haut à gauche rappelle en permanence le preset,
//! l'intégrateur, le multiplicateur de vitesse, l'état pause/marche, le
//! nombre de pas et l'énergie courants — sans lui, une touche pressée par
//! erreur (ou dont on a oublié l'effet) ne laissait aucune trace visible : le
//! frontend Bevy a un `StatusLabel` équivalent, mais Pixels n'affichait rien
//! du tout. Rendu via une police bitmap 3x5 maison (`glyph`), suffisante pour
//! un HUD (majuscules, chiffres, ponctuation minimale) mais pas un rendu de
//! texte général.

use std::num::NonZeroU32;
use std::sync::Arc;

use gravity::integrator::IntegratorKind;
use gravity::preset::{build_preset, Preset};
use gravity::simulation::{Simulation, SimulationConfig};
use softbuffer::Surface;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WIDTH: u32 = 900;
const HEIGHT: u32 = 700;

struct View {
    center_x: f64,
    center_y: f64,
    scale: f64,
    dragging: bool,
    last_mouse: (f64, f64),
}

impl View {
    fn new() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            scale: 40.0,
            dragging: false,
            last_mouse: (0.0, 0.0),
        }
    }

    fn world_to_screen(&self, x: f64, y: f64, w: u32, h: u32) -> (i64, i64) {
        let sx = w as f64 / 2.0 + (x - self.center_x) * self.scale;
        let sy = h as f64 / 2.0 - (y - self.center_y) * self.scale;
        (sx.round() as i64, sy.round() as i64)
    }
}

struct App {
    sim: Simulation,
    preset: Preset,
    paused: bool,
    speed: usize,
    view: View,
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    width: u32,
    height: u32,
}

impl App {
    fn rebuild(&mut self) {
        let config = SimulationConfig {
            dt: 0.01,
            integrator: self.sim.config.integrator,
            ..Default::default()
        };
        self.sim = Simulation::new(config, build_preset(self.preset));
    }

    fn draw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let w = window.inner_size().width;
        let h = window.inner_size().height;
        let mut buffer = surface.buffer_mut().unwrap();
        let buf: &mut [u32] = &mut buffer;
        let wh = h as usize;
        let ww = w as usize;

        // Fond papier crème.
        for pixel in buf.chunks_exact_mut(1) {
            let r = (245u32 << 16) | (241u32 << 8) | 231;
            pixel[0] = r;
        }

        // Traces des trajectoires.
        let mut prev: Vec<Option<(i64, i64)>> = vec![None; self.sim.bodies.len()];
        for snapshot in &self.sim.history {
            for (b, point) in snapshot.iter().enumerate() {
                let (sx, sy) = self.view.world_to_screen(point.x, point.y, w, h);
                if let Some((px, py)) = prev[b] {
                    draw_line(buf, ww, wh, px, py, sx, sy, 0x915E4E);
                }
                prev[b] = Some((sx, sy));
            }
        }

        // Corps.
        for body in &self.sim.bodies {
            let (sx, sy) = self.view.world_to_screen(body.pos.x, body.pos.y, w, h);
            let radius = body.radius.unwrap_or(0.3).max(0.5) * self.view.scale * 0.5;
            let radius = radius.clamp(2.0, 18.0) as i64;
            draw_disc(buf, ww, wh, sx, sy, radius, 0x2E6E52);
        }

        draw_hud(
            buf,
            ww,
            wh,
            self.preset,
            self.sim.config.integrator,
            self.speed,
            self.paused,
            self.sim.step_count,
            self.sim.total_energy(),
        );

        buffer.present().unwrap();
    }
}

/// Panneau d'état en haut à gauche : preset, intégrateur, vitesse,
/// pause/marche, nombre de pas, énergie totale. Seule confirmation visuelle
/// qu'une touche a bien été prise en compte (voir la note en tête de
/// module). Fonction libre (plutôt que méthode de `App`) car `App::draw`
/// détient déjà un emprunt mutable de `self.surface` au moment de l'appel.
#[allow(clippy::too_many_arguments)]
fn draw_hud(
    buf: &mut [u32],
    ww: usize,
    wh: usize,
    preset: Preset,
    integrator: IntegratorKind,
    speed: usize,
    paused: bool,
    step_count: u64,
    energy: f64,
) {
    const PANEL_BG: u32 = 0xE4DCC6;
    const TEXT_COLOR: u32 = 0x33291F;
    const SCALE: i64 = 2;
    const LINE_HEIGHT: i64 = 7 * SCALE;
    const MARGIN: i64 = 6;

    let status = if paused { "PAUSED" } else { "RUNNING" };
    let lines = [
        format!("PRESET: {}", hud_preset_label(preset)),
        format!(
            "INTEGRATOR: {}  SPEED: {}X",
            hud_integrator_label(integrator),
            speed
        ),
        format!("{status}  STEP: {step_count}"),
        format!("ENERGY: {energy:.3}"),
    ];
    let panel_w = lines.iter().map(|l| l.len()).max().unwrap_or(0) as i64 * 4 * SCALE + 8;
    let panel_h = lines.len() as i64 * LINE_HEIGHT + 8;
    draw_rect(buf, ww, wh, MARGIN, MARGIN, panel_w, panel_h, PANEL_BG);
    for (i, line) in lines.iter().enumerate() {
        draw_text(
            buf,
            ww,
            wh,
            MARGIN + 4,
            MARGIN + 4 + i as i64 * LINE_HEIGHT,
            line,
            SCALE,
            TEXT_COLOR,
        );
    }
}

/// Libellés HUD volontairement ASCII/majuscules — distincts de
/// `Preset::label()`/`IntegratorKind::label()` (accents, minuscules,
/// parenthèses) que la police bitmap 3x5 ci-dessous ne sait pas rendre.
fn hud_preset_label(preset: Preset) -> &'static str {
    match preset {
        Preset::SolarSystem => "SOLAR SYSTEM",
        Preset::Binary => "BINARY + PLANET",
        Preset::FigureEight => "FIGURE EIGHT",
        Preset::RandomCluster => "RANDOM CLUSTER",
    }
}

fn hud_integrator_label(kind: IntegratorKind) -> &'static str {
    match kind {
        IntegratorKind::Euler => "EULER",
        IntegratorKind::Verlet => "VERLET",
        IntegratorKind::Leapfrog => "LEAPFROG",
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title("Gravity Simulation — Pixels (CPU)")
            .with_inner_size(LogicalSize::new(WIDTH as f64, HEIGHT as f64));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(PhysicalSize { width, height }) => {
                self.width = width;
                self.height = height;
                if let Some(surface) = &mut self.surface {
                    let _ = surface.resize(
                        NonZeroU32::new(width.max(1)).unwrap(),
                        NonZeroU32::new(height.max(1)).unwrap(),
                    );
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Released {
                    return;
                }
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Character(ref c) => match c.as_str() {
                        "1" => {
                            self.preset = Preset::SolarSystem;
                            self.rebuild();
                        }
                        "2" => {
                            self.preset = Preset::Binary;
                            self.rebuild();
                        }
                        "3" => {
                            self.preset = Preset::FigureEight;
                            self.rebuild();
                        }
                        "4" => {
                            self.preset = Preset::RandomCluster;
                            self.rebuild();
                        }
                        "e" => self.sim.config.integrator = IntegratorKind::Euler,
                        "v" => self.sim.config.integrator = IntegratorKind::Verlet,
                        "l" => self.sim.config.integrator = IntegratorKind::Leapfrog,
                        "p" => self.paused = !self.paused,
                        "r" => self.rebuild(),
                        "+" | "=" => self.speed = (self.speed + 1).min(64),
                        "-" => self.speed = self.speed.saturating_sub(1).max(1),
                        _ => {}
                    },
                    _ => {}
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let (mx, my) = (position.x, position.y);
                if self.view.dragging {
                    let dx = (mx - self.view.last_mouse.0) / self.view.scale;
                    let dy = (my - self.view.last_mouse.1) / self.view.scale;
                    self.view.center_x -= dx;
                    self.view.center_y += dy;
                }
                self.view.last_mouse = (mx, my);
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.view.dragging = state == ElementState::Pressed;
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let zoom = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 60.0,
                };
                self.view.scale = (self.view.scale * (1.0 + 0.1 * zoom)).clamp(2.0, 400.0);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                if !self.paused {
                    for _ in 0..self.speed {
                        self.sim.step();
                    }
                    self.sim.merge_collisions();
                }
                self.draw();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn put_pixel(buf: &mut [u32], ww: usize, wh: usize, x: i64, y: i64, color: u32) {
    if x < 0 || y < 0 || x >= ww as i64 || y >= wh as i64 {
        return;
    }
    let idx = y as usize * ww + x as usize;
    buf[idx] = color;
}

fn draw_disc(buf: &mut [u32], ww: usize, wh: usize, cx: i64, cy: i64, radius: i64, color: u32) {
    let r2 = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= r2 {
                put_pixel(buf, ww, wh, cx + dx, cy + dy, color);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    buf: &mut [u32],
    ww: usize,
    wh: usize,
    mut x0: i64,
    mut y0: i64,
    x1: i64,
    y1: i64,
    color: u32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        put_pixel(buf, ww, wh, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rect(buf: &mut [u32], ww: usize, wh: usize, x0: i64, y0: i64, w: i64, h: i64, color: u32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            put_pixel(buf, ww, wh, x, y, color);
        }
    }
}

/// Dessine `text` avec la police bitmap `glyph`, `scale` pixels par cellule.
/// Chaque glyphe occupe 3 colonnes + 1 d'espacement (4·`scale` au total).
#[allow(clippy::too_many_arguments)]
fn draw_text(
    buf: &mut [u32],
    ww: usize,
    wh: usize,
    x0: i64,
    y0: i64,
    text: &str,
    scale: i64,
    color: u32,
) {
    let mut cursor_x = x0;
    for ch in text.chars() {
        let rows = glyph(ch);
        for (ry, row) in rows.iter().enumerate() {
            for cx in 0..3i64 {
                if (row >> (2 - cx)) & 1 == 1 {
                    draw_rect(
                        buf,
                        ww,
                        wh,
                        cursor_x + cx * scale,
                        y0 + ry as i64 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cursor_x += 4 * scale;
    }
}

/// Police bitmap minimale 3 colonnes x 5 lignes (un `u8` par ligne, 3 bits de
/// poids faible = pixels de gauche à droite). Couvre l'alphabet, les
/// chiffres et une poignée de symboles — largement suffisant pour un HUD
/// d'état, pas un rendu de texte général. Tout caractère non couvert
/// (accents, minuscules non mappées, ponctuation rare) rend un blanc plutôt
/// que de paniquer.
fn glyph(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        _ => [0b000, 0b000, 0b000, 0b000, 0b000],
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App {
        sim: Simulation::new(
            SimulationConfig::default(),
            build_preset(Preset::SolarSystem),
        ),
        preset: Preset::SolarSystem,
        paused: false,
        speed: 2,
        view: View::new(),
        window: None,
        surface: None,
        width: WIDTH,
        height: HEIGHT,
    };
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_is_defined_for_every_character_the_hud_actually_prints() {
        // Toutes les chaînes réellement produites par `draw_hud` (labels
        // HUD + gabarit des lignes de format!()) : si l'une contient un
        // caractère non couvert par `glyph`, il se rendrait en blanc
        // silencieux plutôt que de planter — ce test l'aurait détecté ici.
        let sample = format!(
            "PRESET: {} INTEGRATOR: {} SPEED: 12X RUNNING PAUSED STEP: 999999 ENERGY: -3.821",
            hud_preset_label(Preset::Binary),
            hud_integrator_label(IntegratorKind::Leapfrog),
        );
        for ch in sample.chars() {
            if ch == ' ' {
                continue;
            }
            assert_ne!(
                glyph(ch),
                [0, 0, 0, 0, 0],
                "caractère '{ch}' rendu en blanc (non couvert par glyph())"
            );
        }
    }

    #[test]
    fn draw_text_stays_within_bounds_and_does_not_panic() {
        // Un HUD dessiné près du bord d'une petite fenêtre ne doit pas
        // paniquer (`put_pixel` doit absorber les coordonnées hors-cadre).
        let ww = 20usize;
        let wh = 20usize;
        let mut buf = vec![0u32; ww * wh];
        draw_text(&mut buf, ww, wh, 0, 0, "HUD TEST 123", 2, 0xFF0000);
        draw_text(&mut buf, ww, wh, -50, -50, "OFFSCREEN", 3, 0x00FF00);
        // Au moins un pixel du premier texte, dessiné dans le cadre, a bien
        // été peint (sinon `draw_hud` serait un panneau invisible).
        assert!(buf.contains(&0xFF0000));
    }

    #[test]
    fn hud_labels_are_ascii_so_they_render_with_the_bitmap_font() {
        for preset in Preset::ALL {
            assert!(hud_preset_label(preset).is_ascii());
        }
        for kind in IntegratorKind::ALL {
            assert!(hud_integrator_label(kind).is_ascii());
        }
    }
}
