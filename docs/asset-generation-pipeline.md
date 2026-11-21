# Asset Generation Pipeline — Garth's Mac (Nov 21, 2026)

How 3D game assets get made on this machine: concept image → clean cutout → TRELLIS.2 sculpt → game-ready `.glb`. Written for Claude and Antigravity. Everything below is installed and verified working unless marked otherwise.

## Machine rules (read first)

- Apple Silicon Mac, **32 GB unified memory**. Never run ComfyUI, TRELLIS.2, ACE-Step, and Ollama at the same time. `tools/comfy_runner.py` (in `~/Projects/elfstoryreborn`) and `~/GLocalLM/ops/foundry.py` arbitrate AGENT ↔ STUDIO mode — use them, don't hand-launch heavy services.
- **All TRELLIS.2 jobs go through Elf Works `/foundry`** (`http://127.0.0.1:8765/foundry`, or `POST /api/foundry`) so progress and results are visible on the dashboard. Never invoke `generate.sh` directly in a hidden terminal. Raw outputs land in `~/GLocalLM/workspace/3d/<slug>-<stamp>.glb`.
- Hugging Face downloads on this connection: XET stalls. Use `HF_HUB_DISABLE_XET=1` or a direct resumable `curl -C -`, and always verify the final file size before declaring a download done.

## The stack

| Stage | Tool | Location / notes |
|---|---|---|
| Concept art | ComfyUI + **DreamShaper XL Turbo v2.1** | `~/ComfyUI/models/checkpoints/DreamShaperXL_Turbo_v2_1.safetensors` (6.94 GB). Wired as `DEFAULT_CKPT` in `comfy_runner.py`. SDXL-quality stylized concepts in ~30 s. |
| Background removal | **BiRefNet** via `rembg` | Isolated venv at `~/asset-tools/venv` (do NOT install into the ComfyUI venv — the ComfyUI-RMBG node pack pulls onnxruntime-gpu/decord, a macOS hazard; it was tried and removed). Model: `~/.rembg/models/birefnet-general.onnx` (~973 MB). |
| Image → 3D | **TRELLIS.2** (Elf Works foundry) | DINOv3 ViT-L is TRELLIS.2's internal image encoder — there is nothing separate to install. |
| Retopo / bake / decimate | Blender 5.3 | `/Applications/Blender.app/Contents/MacOS/Blender` (not on PATH). Project scripts in `~/Projects/elfstoryreborn/tools/`. |
| Rigging (experimental) | **UniRig** | `~/Projects/UniRig` — installed, **not production-ready**, see below. |

## Workflow — organic assets (creatures, trees, rocks, statues, hero props)

1. **Concept.** `python3 tools/comfy_runner.py --mode concept` (from `~/Projects/elfstoryreborn`). Prompt for a single isolated subject on a **neutral mid-gray background**, unlit/delighted look, 1024×1024. No scenery, no ground clutter — TRELLIS reads the whole image.
2. **Cutout (new step).** `~/asset-tools/venv/bin/rembg i -m birefnet-general concept.png cutout.png`. Verify the alpha visually (composite onto gray) before continuing — a bad cutout poisons the sculpt. Verified clean on fur/feather edges (platypus test, Nov 21).
3. **Sculpt.** Submit the cutout to the foundry: `pipeline_type: "512"`, `texture_size: 1024`. Output: `~/GLocalLM/workspace/3d/<slug>-<stamp>.glb`. **Keep this raw file forever** — it is the fidelity source for every later comparison.
4. **Game-ready conversion.** Two proven routes:
   - *Retopo + bake* (`tools/blender_retopo_bake.py`): ~2,500 clean quads, 1024px Diffuse/Normal/AO bake, target < 600 KB. Good for creatures/props. **Warning:** the script's `keep_largest_only` deletes all but the biggest island — fatal for multi-island assets (trees, buildings). QuadriFlow also fails on multi-island meshes.
   - *UV-preserving decimation of the raw output*: for foliage and other intersecting-shell assets, decimate the raw TRELLIS mesh directly (collapse modifier, textures re-exported as WebP). The ElfStory oak ships at **25%** (48,692 tris) — judged indistinguishable-enough at chase-camera distance; 12% showed visible shards/speckle up close and was rejected.
5. **Gate.** Nothing ships without the shipping gate below.

## Workflow — architecture & hard-surface props

Standing route (crisp stylized, Tunic / Link's Awakening look):

1. **Textures, not sculpts.** `python3 tools/comfy_runner.py --mode pbr-texture` → 1024×1024 seamless PBR set (albedo, normal, roughness).
2. **Geometry by construction.** `tools/blender_arch_kit.py`: clean modular quads, 1,500–4,000 quads, beveled edges, weighted normals, < 500 KB `.glb`, bottom-center origin at Z = 0.
3. **Never run TRELLIS.2 on straight-edged buildings** — it melts them.

**Project note (ElfStory, Nov 21):** Garth found the flat-color arch-kit style "drastically bland" next to the textured concepts and switched ElfStory's buildings to the **textured TRELLIS originals at ~25% decimation** (same treatment as the oak). That is now ElfStory's shipping direction. Other projects: the arch-kit route stands unless Garth calls it per project.

## Hard guardrails

- **Never** run raw 2D ComfyUI img2img (`--mode refine-texture`) on a TRELLIS.2 `xatlas` UV atlas — ~17K fragmented UV islands bleed across seams and ruin the texture.
- **Never** use naive `DECIMATE` + `flat_shade=True`.
- Models must be **watertight** — with one exception class: raw TRELLIS foliage is intersecting thin shells and cannot be both strictly watertight and faithful. Foliage ships under the exception, and the trade-off is **Garth's call per asset**, recorded in the project's `tools/ASSET_GATE.md`.
- Blender "polys" from `len(data.polygons)` are quads on retopo/voxel meshes — glTF triangle count is ≈ 2×. Weld verts at 1e-4 before trusting boundary/watertight counts on exported glTF.

## Shipping gate (all projects)

Codified in ElfStory's `tools/ASSET_GATE.md`; the standard applies everywhere:

1. **Source first** — render the raw generator output before repairing or rebuilding anything. Never work from a derivative alone.
2. **Side-by-side or it didn't happen** — candidate judged next to the source render, same framing, at **close (chase-camera) zoom**, not full-frame.
3. **A describable flaw is a fail** — "reads fine at game distance" requires the in-game screenshot, not a sentence.
4. Adversarial review + **Garth approves before commit**.
5. Topology/file-size checks are **not** visual verification.

## UniRig — status: parked, not production-ready

Cloned to `~/Projects/UniRig` with its own `.venv` (torch 2.15, MPS verified) and checkpoints downloaded. Flow would be `extract.sh` → `generate_skeleton.sh` → `generate_skin.sh` → `merge.sh`. Two blockers before any rigged output can be promised:

1. `torch_scatter` — no arm64 wheel for torch 2.15; needs a source build or a pure-torch shim.
2. The skin stage hard-imports `flash_attn` (CUDA-only) at `src/model/unirig_skin.py:10`, used as `MHA(embed_dim=feat_dim, num_heads=num_heads, cross_attn=True)` — needs an MHA shim replicating flash_attn's parameter layout.

Do not schedule rigging work until a single-asset skeleton smoke test passes end to end.

## Housekeeping note

The pipeline documentation inside `~/Projects/elfstoryreborn` (`.agents/skills/oakhaven-asset-pipeline/SKILL.md` — DreamShaper default, the BiRefNet step, UniRig status) and the `DEFAULT_CKPT` change in its `tools/comfy_runner.py` are **uncommitted working-tree edits** in that frozen repo, as of Nov 21. They are live and in effect; committing them is Garth's call.
