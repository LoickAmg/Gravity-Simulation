# Gravity Simulation

Simulation gravitationnelle **N-corps** en Rust. Le moteur de calcul est
indépendant de tout rendu, et trois frontends le consomment.

## Architecture

```
src/
  lib.rs          crate `gravity` (zéro dépendance)
  math.rs         Vec2, constante G
  integrator.rs   Euler / Velocity Verlet / Leapfrog + trait Integrator
  simulation.rs   Simulation (N-corps, énergie, momentum, collisions, historique)
  preset.rs       4 présets réutilisables
  bin/
    cli.rs        frontend terminal (feature: cli, défaut)
    pixels.rs     frontend 2D CPU — winit + softbuffer (feature: pixels)
    bevy.rs       frontend 2D GPU — Bevy (feature: bevy)
tests/
  core.rs         7 tests d'intégration (conservation, stabilité, collisions)
```

Le moteur (`gravity`) n'a **aucune dépendance externe** : chaque frontend est
un binaire séparé conditionné par une feature Cargo.

## Build & run

```bash
# CLI (par défaut)
cargo run --release --bin gravity-cli -- solar 5000 0.01 verlet

# Renderer CPU 2D (winit + softbuffer)
cargo run --features pixels --bin gravity-pixels

# Renderer GPU 2D (Bevy)
cargo run --features bevy --bin gravity-bevy
```

Lors du premier appel, Bevy compile un grand nombre de crates : la construction
prend quelques minutes, puis est mise en cache.

## Contrôles (pixels & bevy)

| Touche | Action |
|--------|--------|
| `1/2/3/4` | changer de preset (système solaire, binaire, 8 de chiffre, amas) |
| `E/V/L`   | Euler / Velocity Verlet / Leapfrog |
| `+` / `-` | accélérer / ralentir le temps |
| `R`       | remettre à zéro (recharger le preset) |
| `P`       | pause |
| molette   | zoom, clic-glisser pour déplacer (pixels uniquement) |
| `Échap`   | quitter |

## CLI

```
gravity-cli [PRESET] [PAS] [DT] [INTEGRATEUR]
PRESET       solar | binary | eight | cluster   (défaut: solar)
PAS          nombre de pas                      (défaut: 2000)
DT           pas de temps                       (défaut: 0.01)
INTEGRATEUR  euler | verlet | leapfrog          (défaut: verlet)
```

La sortie CLI affiche l'énergie et le momentum tout au long de la simulation :
la dérive d'énergie `Δ` sert de vérification de stabilité de l'intégrateur.

## Physique

- Gravité adimensionnée avec `G = 10`.
- Intégrateurs symplectiques (Velocity Verlet, Leapfrog) stables sur de longues
  périodes ; Euler explicite conserve mal l'énergie et sert de comparaison.
- Pas de temps `dt = 0.01` avec des orbites de période ≈ 8–25 unités.
- Le rapport de masse étoile/planète ≈ 600:1.
- Le « 8 de chiffre » (3 corps) utilise les conditions initiales de
  Chenciner–Montgomery ré-échelonnées par √G.
- Les collisions (si rayons définis) fusionnent les corps en conservant le
  momentum linéaire.

## Tests & qualité

```bash
cargo test
cargo clippy --all-features
cargo fmt --all
```

7 tests d'intégration sur le moteur (conservation d'énergie/momentum,
stabilité des orbites, fusion de collisions), clippy sans warning (`-D
warnings`, toutes features), rustfmt appliqué. CI GitHub Actions
(`.github/workflows/ci.yml`) sur push/PR : fmt, clippy, tests et build sur
les trois features (`cli`, `pixels`, `bevy`).

## Licence

MIT — voir [LICENSE](./LICENSE).

## Projections

- Frontend Web (WASM) réutilisant le moteur.
- Ajout de la constante de gravitation en unités physiques réelles.
- Barns–Hut pour accélérer le calcul N-corps à grande échelle.
