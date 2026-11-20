# ElfStory Reborn (Godot 4 + Rust)

A standalone 3D roguelite dungeon crawler where you rebuild Oakhaven between runs, and the rebuilt town changes your next runs. Built with **Godot 4.7.2** (Forward+ / Metal renderer + native RPG UI) and **Rust** (`elfstory-core` simulation crate + `elfstory-gdext` GDExtension bridge).

## Start Here
- [PROJECT_TRUTH.md](PROJECT_TRUTH.md) — Core Combined Model design truth (Dark Cloud majors + ActRaiser growth) & V2 Hybrid 3D Asset Pipeline.
- [HANDOFF.md](HANDOFF.md) — Complete agent handoff brief, TS→Rust port map, audit guardrails, and Vertical Slice 1 instructions.
- [ARCHITECTURE.md](ARCHITECTURE.md) — Rust crate layout, Godot scene structure, and visual/camera design.
- [ROADMAP.md](ROADMAP.md) & [STATUS.md](STATUS.md) — Milestones and current verified state.
- [AGENTS.md](AGENTS.md) — Project rules for Muse Code, Claude Code, Codex, and Antigravity.

## Quick Commands
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd rust && cargo test
/Applications/Godot.app/Contents/MacOS/Godot --path godot
```
