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

        buffer.present().unwrap();
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
