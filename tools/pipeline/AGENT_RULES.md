# Agent rules

Every AI working on Garth's games follows these. A game's own `AGENTS.md`, `docs/` and `pipeline.json` win on game decisions.
Full reasoning: `PIPELINE.md`, `DESIGN.md`, `ENGINEERING.md`, `ASSETS.md` in https://github.com/Garthnesss/gamedev-pipeline (on Garth's Mac: `~/Projects/gamedev-pipeline`). `gdp` is `bin/gdp` there.

## 0. Before anything

The first two lines apply to projects that have a `pipeline.json`. Older projects keep their own status rules until they are migrated.

- Run `gdp next`, or `gdp doctor` for the details. Too many UNPLAYED features means **no new feature**: fix, tune, or prepare something for Garth to play.
- You may write `PLANNED` or `UNPLAYED` in `STATUS.md`. Only Garth writes `PLAYED` or `VERIFIED`.
- You do not review, approve, or gate your own work.
- Original games only: never reproduce another game's name, characters, art, music, levels or look.

## 1. Machine

- Engine pin: **4.7.2 stable** everywhere. `gdp setup` finds or installs it.
- Anywhere but Garth's Mac (cloud agent, CI): code, tests and headless checks only. No generation, no captures, no feel judgements; leave those as play-queue entries.

On Garth's Mac (non-negotiable):
- Apple Silicon, 32 GB. No Windows paths, no Windows steps.
- Godot: `/Applications/Godot.app/Contents/MacOS/Godot`. Do not upgrade without a migration branch (see `CHECKLISTS.md` → Engine upgrade).
- **One generation job at a time.** Before any ComfyUI / TRELLIS.2 job run
  `gdp gen status` (it is `tools/assets/genq.py`; older copies in game repos share the same queue) and queue behind anything running.
  TRELLIS.2 only through `gdp gen foundry --image <abs cutout> --name <slug>` (it drives Elf Works `/foundry`). Never `generate.sh`, never a direct `/api/foundry` call.
- Never run ComfyUI, TRELLIS.2, ACE-Step, and Ollama together. Let `comfy_runner.py` / `foundry.py` switch modes.

## 2. How to edit a Godot project

- **Plain text first.** Edit `.gd`, `.tscn`, `.tres`, `project.godot` directly, then validate with the headless CLI. It is deterministic, diffable, and needs no editor open.
- Use an editor/runtime MCP bridge **only** for things text can't do: live playtest screenshots, UI click simulation, runtime stack traces.
- After every change, run the project's verify step (see §6). "It parses in my head" is not verification.
- **Moving or renaming a script/shader: move its `.uid` sidecar with it** (`foo.gd` + `foo.gd.uid`). Never delete, ignore, or regenerate `.uid` or `.import` files. Prefer renames inside the Godot FileSystem dock when a human is present.
- Commit (or at least `git stash`-able state) before each agent session so any change can be rolled back.
- Don't touch `.godot/` (cache) or `export_presets.cfg` secrets.

## 3. Code rules (GDScript)

- **Static typing everywhere**: typed vars, params, returns. `var hp: int = 10`, `func hit(dmg: int) -> void:`. It turns typos into parse errors, and it is faster.
- **Inheritance for classes, composition for scenes.** Class inheritance max 1–2 levels (`Interactable → Door`). Never build multi-level *inherited scenes*; never inherit scenes from `.glb` imports (causes override loss and `unique_id` diff noise in 4.6+). Wrap imports by instancing them as a child.
- **Call down, signal up** inside a scene. Across distant branches use a small **domain bus** (`CombatEvents`, `UIEvents`), never one giant `Events.gd`. Disconnect bus listeners in `_exit_tree()`.
- Autoloads: tiny and single-purpose (`SaveManager`, `AudioManager`). No `GameManager.gd` god object.
- Don't paper over init-order bugs with `call_deferred()` in `_ready()`. Fix ownership. (`queue_free()` is already deferred.)
- No cyclic `preload()` chains. No RefCounted reference cycles.
- `snake_case` for every file and folder (C# `.cs` excepted). Exported builds are case-sensitive; macOS is not — a wrong case works locally and breaks after export.
- Cache node refs in `@onready var`. No per-frame allocations, string building, or `get_node()` in `_process`.

## 4. Data and saves

- **Static data (blueprints)** = Custom `Resource` `.tres` (or Rust structs). **Read-only at runtime.** Never mutate a loaded `.tres`; every instance shares it (20 goblins sharing `goblin_stats.tres` all die together).
- **Runtime state** lives on the node / a RefCounted / a Dictionary that holds an ID pointing at its blueprint. Use `duplicate(true)` only for intentional per-instance engine resources (unique material params).
- **Player saves are never `.tres`, `.res`, `ConfigFile` with objects, or `str_to_var()`** — those can execute code from a crafted file. Use `JSON.parse_string()` (no objects) or `FileAccess.get_var(false)`.
- Every save: `schema_version` int at the root, one central migration pipeline (`v1→v2→v3` pure functions), atomic write (`.tmp` → verify → rotate old to `.bak` → rename), loader falls back to `.bak`, validate every slice before applying any.
- JSON traps: ints come back as floats, dict keys become strings, 64-bit ints lose precision (store seeds/IDs as strings). Under strict typing, `int(data["x"])` is a parse error — type-check, assign to a typed `float` local, then `int()` it.
- Use `systems/save/save_manager.gd` from the starter; it has tests. Do not write another.

## 5. Assets

- Models enter Godot as **`.glb`**, never raw `.blend` in a shared/CI project. `1 Blender unit = 1 m`, Apply All Transforms, forward = `-Y` in Blender (→ `-Z` in Godot), bottom-center origin at `Z = 0`.
- Collision via import suffixes (`-col`, `-convcolonly`) or a post-import script — never hand-edit imported scenes.
- Say the route and tier first (`ASSETS.md`), or use the game's own asset skill if it has one. Hard-surface → A (no TRELLIS). Organic → B (concept → cutout → `/foundry` → game-ready). Nothing above Tier 1 before Gate 1.
- Never run raw 2D img2img on a TRELLIS xatlas UV atlas.
- Every asset passes its tier's gate (`ASSETS.md` §5). `gdp assets` is the automated half; looking at it is the other half.
- Track AI provenance per asset (`ASSETS.md` → Provenance). Player-facing AI art triggers Steam disclosure — Garth decides per game.

## 6. Verify before saying "done"

```bash
tools/pipeline/godot_verify.sh .        # in a game made by gdp new
gdp verify <folder with project.godot>   # any other
```
It imports, parse-checks every script, runs every `*_check.gd`, audits assets, and runs `cargo test` when there is a `rust/` crate. It fails on any `SCRIPT ERROR`, because Godot exports do not.

Report exactly what ran and what passed. If it needs a display or a person, say so. Never "should work". A new check must fail without your change.

## 7. Performance — measure before you optimize

Ladder, in order, stop when fixed: profile (release build, Max FPS 0) → fix algorithm/culling → batch (atlases, MultiMesh per spatial chunk) → light/shadow budgets → Servers/RIDs → C#/Rust/GDExtension. Never escalate language without a profiler screenshot showing the hot path. Never build a 3D object from dozens of separate nodes when one mesh or MultiMesh will do.

## 8. Don't

- Don't build frameworks, plugin systems, or ORMs before the core loop is proven fun in a `gyms/` prototype.
- Don't add C#/.NET to a GDScript project (kills Web export, slows iteration) without Garth's call.
- Don't commit secrets (keystores, API keys, `BUTLER_API_KEY`).
- Don't install `npx` MCP packages blind; build from source or pin a reviewed version.
