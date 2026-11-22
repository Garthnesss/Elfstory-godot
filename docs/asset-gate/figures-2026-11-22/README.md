# Gate pack — first miniature figures (Nov 22, 2026)

**Status: APPROVED by Garth Nov 22, 2026 — ship-with-exceptions for all four figures, sentinel keeps its teal cloak, rat v2 accepted as-is. Committed and merged to main on his go-ahead (supersedes the awaiting-call note; tools/ASSET_GATE.md §2.4 satisfied).**

Report Option A, Day 1–2: replace capsule actors with painted tabletop miniatures on the game's own walnut plinth (faction-coloured rim drawn in `Main.gd` `make_actor`).

Route: DreamShaper XL Turbo concept ("painted tabletop miniature figurine …", `docs/concepts/figures/`) → BiRefNet cutout (`cutouts/`) → TRELLIS.2 via `/foundry`, queued through `tools/genq.py` (pipeline 512, tex 1024) → `tools/blender_decimate_source.py --rebake --tex-size 2048 --cut-base` (base sliced off, base debris removed, collapse decimate, Smart-UV re-bake of **colour + metallic + roughness**).

Raw sources (keep forever): `~/Projects/GLocalLLM/workspace/3d/`
`fig_vanguard-20261122-122908.glb`, `fig_lynx-20261122-123724.glb`, `fig_blight_rat-20261122-124414.glb`, `fig_crypt_sentinel-20261122-125335.glb`.

## Numbers

| Figure | Tris | File | Watertight (boundary / non-manifold / degenerate) | Base cut |
|---|---|---|---|---|
| fig_vanguard (player, Vanguard) | 24,000 | 1.96 MB | 11,720 / 177 / 5 | auto, 7.3% height |
| fig_lynx (Silverwind Lynx) | 16,000 | 1.47 MB | 6,969 / 25 / 0 | none (no base) |
| fig_blight_rat (Blight Rat) | 36,000 | 3.39 MB | 36,160 / 47 / 0 | manual `--cut-z 0.085` |
| fig_crypt_sentinel (Crypt Sentinel) | 20,000 | 1.44 MB | 3,483 / 54 / 4 | auto, 4.7% height |

All four fail the watertight gate and are over the 600 KB budget. This is the same class as the platypus exception: the decimate-source route keeps TRELLIS's open shells to keep fine detail.

## Images

- `<figure>_source_vs_candidate.png`: left is the raw TRELLIS render, right is the shipped candidate (same renderer).
- `ingame_close_zoom.png`: closest camera zoom, mid-combat. Blue rim = player, teal = companion, red = enemy.
- `ingame_full_frame.png`: normal play framing.

## Adversarial review (independent subagent), round 1, and what changed

**All four.** The review found the bake was colour-only, so metal turned into chalk: plaster-white pauldrons, bleached blades, a matte rat.
- **Fixed:** the bake now also carries metallic and roughness. Pauldrons, blades and the rat's gloss match the source in the round-2 renders.

**Vanguard.** Review verdict: REDO the bake.
- **Fixed:** metal is restored.
- **Still open:**
  - The shield emblem is slightly soft compared with the source.
  - The left boot sole is a little ragged where the base was cut.
  - The sword is wavier than the concept's (this comes from TRELLIS).

**Lynx.** Review verdict: SHIP-WITH-EXCEPTION.
- Small flecks on the cheek and ear.
- No whiskers.
- Low contrast against pale floors; the dark plinth separates it.

**Blight rat.** Review verdict: REDO (round 1).
- **Fixed:** rebuilt at 36k triangles, which rounds out the ears, plus the new metallic/roughness bake.
- **Still open:**
  - The moss mask is torn in places. The source sculpt has this too.
  - The feet are not on one plane after the cut, so one foot sits a little low.
  - The heaviest file of the four at 3.39 MB.
  - It drifted from the concept: browner body, red spikes, short upright tail.

**Crypt sentinel.** Review verdict: SHIP-WITH-EXCEPTION.
- Its teal cloak is the same hue family as the companion.
- Its sword pokes into the next tile.

## Garth's call needed

1. Ship-with-exception for the four (non-watertight, 1.4–3.4 MB)? Or set a figure budget, e.g. 16k tris and 1024 px textures for enemies?
2. Blight rat: accept it, or regenerate it (four-legged, closer to the concept)?
3. Sentinel cloak: keep the teal, or regenerate it in a colour that can't be confused with allies?

## Rat redo (Garth asked, Nov 22)
- New concept `docs/concepts/figures/fig_blight_rat_v2_s303_nodelight.png`: crouched, grey, fangs, moss spine, low tail. It was rendered **without** comfy_runner's delighting pass, which turned out to cause the white smears seen on the first concepts (likely the lynx's cheek flecks too).
- BiRefNet removed the sculpted base along with the background, so no base cut was needed. Raw: `fig_blight_rat_v2-20261122-132218.glb`. Shipped: 20,000 tris, 1.90 MB, with the colour + metallic/roughness bake. In the source-vs-candidate render it is a close match with no shards.
- **Not yet done:** the in-game capture, the watertight numbers, and a second adversarial review of the new rat.

## Decision (Garth, Nov 22, 2026)

- **Ship-with-exceptions: YES for all four** (non-watertight, over 600 KB budget) — same exception class as the platypus/oak foliage.
- **Blight rat v2: accepted** as shipped (20,000 tris, 1.90 MB, no base cut needed). The outstanding in-game capture, watertight numbers and second adversarial review were waived by Garth's approval — noted here so they are not mistaken for done.
- **Sentinel cloak: keep the teal.**
- **Delighting: turned OFF by default** in `tools/comfy_runner.py` (opt-in with `--delight`; `--no-delight` kept as a compatible no-op), because the pass caused the white smears on the first concepts and likely the lynx's cheek flecks.
