#!/usr/bin/env python3
"""
ElfStory Reborn — V2 Local ComfyUI Runner (Apple Silicon MPS, 32 GB Memory-Safe).

Supports:
  1. Track A (--mode pbr-texture):
     Generates 1024x1024 seamless tileable stylized PBR textures:
       - <slug>.png (Seamless Albedo with cross-edge toroid blending)
       - <slug>_normal.png (Derived Tangent-Space Normal Map)
       - <slug>_roughness.png (Derived Stylized Matte Roughness Map)
  2. Track B (--mode concept):
     Generates 1024x1024 Unlit / Delighted isolated concept art with flat
     diffuse overcast studio lighting (zero harsh specular glare or dark drop
     shadows) + automatic shadow/highlight delighting pass before TRELLIS.2.
  3. Legacy (--mode texture, --mode refine-texture)

Automatically respects the 32 GB Mac memory rule:
  - Stops ACE-Step Music Studio if running (via Studio Switch)
  - Flips Elf Works AGENT -> STUDIO before loading checkpoints
  - Starts ~/ComfyUI on 127.0.0.1:8188 if not already running
  - Frees VRAM/RAM via POST /free and flips back to AGENT when done
"""
import argparse
import json
import os
import random
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HOME = Path.home()
COMFY_DIR = Path(os.environ.get("COMFYUI_DIR", str(HOME / "ComfyUI")))
COMFY_VENV_PY = COMFY_DIR / ".venv" / "bin" / "python"

# Ensure PIL + numpy are available by re-execing into ComfyUI's venv if needed
try:
    import numpy as np
    from PIL import Image
except ImportError:
    if COMFY_VENV_PY.is_file() and Path(sys.executable).resolve() != COMFY_VENV_PY.resolve():
        os.execv(str(COMFY_VENV_PY), [str(COMFY_VENV_PY), *sys.argv])
    raise

COMFY_URL = os.environ.get("COMFYUI_URL", "http://127.0.0.1:8188")
SWITCH_URL = os.environ.get("SWITCH_URL", "http://127.0.0.1:3100")
ELFMODE_PY = HOME / "GLocalLM" / "ops" / "elfmode.py"
DEFAULT_CKPT = "sd_xl_turbo_1.0_fp16.safetensors"

# V2 Track B: Unlit / Delighted flat diffuse overcast studio lighting (subject leads first tokens!)
CONCEPT_STYLE_SUFFIX = (
    ", isolated single 3d game asset centered in frame, three-quarter view, "
    "crisp stylized low-poly fantasy art style like Tunic and Links Awakening, "
    "flat diffuse overcast studio lighting, unlit albedo texture look, even omnidirectional soft light, "
    "zero specular glare, zero cast shadows, no drop shadow, plain solid neutral mid-gray background, "
    "no ground plane, no grass pad, no sky, no text"
)
CONCEPT_NEGATIVE = (
    "harsh shadows, drop shadow, cast shadow, specular highlight, glossy glare, rim light, vignette, "
    "dark underbelly, black shadows, photorealistic, noisy, blurry, cluttered background, grass island, "
    "ground pedestal, sky, clouds, text, watermark, hud, frame, border, multiple objects, cropped, cut off"
)

TEXTURE_STYLE_PREFIX = (
    "seamless tileable 2d game texture, top-down orthographic flat diffuse lighting, "
    "crisp stylized low-poly hand-painted fantasy RPG material, clean shapes, even illumination, zero shadows, "
)
TEXTURE_NEGATIVE = (
    "perspective, 3d render, vignette, harsh shadows, directional light, specular glare, "
    "watermark, text, border, seam, blurry, photo"
)


def _http_json(url, method="GET", payload=None, timeout=5):
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    headers = {"Content-Type": "application/json"} if data is not None else {}
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        raw = resp.read()
        return json.loads(raw.decode("utf-8", "replace")) if raw else {}


def comfy_is_up():
    try:
        _http_json(COMFY_URL + "/system_stats", timeout=2)
        return True
    except Exception:
        return False


def free_comfyui():
    """Ask a running ComfyUI server to unload all models and release GPU/RAM."""
    try:
        _http_json(COMFY_URL + "/free", method="POST", payload={"unload_models": True, "free_memory": True}, timeout=5)
        return True
    except Exception:
        return False


def _stop_music_studio():
    try:
        st = _http_json(SWITCH_URL + "/api/status", timeout=2)
        if st.get("state") in ("ready", "running"):
            req = urllib.request.Request(SWITCH_URL + "/api/stop", data=b"", method="POST")
            with urllib.request.urlopen(req, timeout=8) as r:
                r.read()
            time.sleep(1.5)
            return True
    except Exception:
        pass
    return False


def _elfmode(cmd):
    if not ELFMODE_PY.is_file():
        return {"ok": True, "mode": "agent", "changed": False}
    try:
        out = subprocess.check_output(["/usr/bin/python3", str(ELFMODE_PY), cmd], text=True, timeout=30)
        return json.loads(out)
    except Exception as e:
        return {"ok": False, "error": str(e)}


def _list_checkpoints():
    ckpt_dir = COMFY_DIR / "models" / "checkpoints"
    if not ckpt_dir.is_dir():
        return []
    return sorted(p.name for p in ckpt_dir.iterdir() if p.is_file() and p.suffix in (".safetensors", ".ckpt"))


def _upload_image(path: Path) -> str:
    """Upload a local image to ComfyUI's /upload/image endpoint and return its server filename."""
    boundary = f"----ComfyBoundary{random.randint(100000, 999999)}"
    raw = path.read_bytes()
    body = (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="image"; filename="{path.name}"\r\n'
        f"Content-Type: image/png\r\n\r\n"
    ).encode("utf-8") + raw + f"\r\n--{boundary}--\r\n".encode("utf-8")
    req = urllib.request.Request(
        COMFY_URL + "/upload/image",
        data=body,
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        info = json.loads(r.read().decode("utf-8", "replace"))
    return info["name"]


def _build_sdxl_turbo_img2img_workflow(ckpt_name, image_name, positive, negative, steps, cfg, denoise, seed, prefix):
    return {
        "1": {
            "class_type": "CheckpointLoaderSimple",
            "inputs": {"ckpt_name": ckpt_name},
        },
        "2": {
            "class_type": "CLIPTextEncode",
            "inputs": {"text": positive, "clip": ["1", 1]},
        },
        "3": {
            "class_type": "CLIPTextEncode",
            "inputs": {"text": negative, "clip": ["1", 1]},
        },
        "4": {
            "class_type": "LoadImage",
            "inputs": {"image": image_name},
        },
        "5": {
            "class_type": "VAEEncode",
            "inputs": {"pixels": ["4", 0], "vae": ["1", 2]},
        },
        "6": {
            "class_type": "KSampler",
            "inputs": {
                "seed": seed,
                "steps": steps,
                "cfg": cfg,
                "sampler_name": "euler_ancestral",
                "scheduler": "sgm_uniform",
                "denoise": denoise,
                "model": ["1", 0],
                "positive": ["2", 0],
                "negative": ["3", 0],
                "latent_image": ["5", 0],
            },
        },
        "7": {
            "class_type": "VAEDecode",
            "inputs": {"samples": ["6", 0], "vae": ["1", 2]},
        },
        "8": {
            "class_type": "SaveImage",
            "inputs": {"filename_prefix": prefix, "images": ["7", 0]},
        },
    }


def _build_sdxl_workflow(ckpt_name, positive, negative, width, height, steps, cfg, seed, prefix):
    is_turbo = "turbo" in ckpt_name.lower()
    return {
        "1": {
            "class_type": "CheckpointLoaderSimple",
            "inputs": {"ckpt_name": ckpt_name},
        },
        "2": {
            "class_type": "CLIPTextEncode",
            "inputs": {"text": positive, "clip": ["1", 1]},
        },
        "3": {
            "class_type": "CLIPTextEncode",
            "inputs": {"text": negative, "clip": ["1", 1]},
        },
        "4": {
            "class_type": "EmptyLatentImage",
            "inputs": {"width": width, "height": height, "batch_size": 1},
        },
        "5": {
            "class_type": "KSampler",
            "inputs": {
                "seed": seed,
                "steps": steps,
                "cfg": cfg,
                "sampler_name": "dpmpp_sde" if is_turbo else "dpmpp_2m",
                "scheduler": "karras",
                "denoise": 1.0,
                "model": ["1", 0],
                "positive": ["2", 0],
                "negative": ["3", 0],
                "latent_image": ["4", 0],
            },
        },
        "6": {
            "class_type": "VAEDecode",
            "inputs": {"samples": ["5", 0], "vae": ["1", 2]},
        },
        "7": {
            "class_type": "SaveImage",
            "inputs": {"filename_prefix": prefix, "images": ["6", 0]},
        },
    }


def delight_concept_image(img_path: Path):
    """
    Delight a concept render in place:
    - Flattens vignette / background gradient to clean neutral gray
    - Lifts dark shadow troughs and compresses harsh specular hot-spots on the subject
      so TRELLIS.2 receives a clean, evenly lit diffuse albedo.
    """
    im = Image.open(img_path).convert("RGB")
    arr = np.asarray(im, dtype=np.float32) / 255.0
    h, w, _ = arr.shape

    # Estimate background color from outer 6% border ring
    b = max(4, int(min(h, w) * 0.06))
    border_pixels = np.concatenate([
        arr[:b, :, :].reshape(-1, 3),
        arr[-b:, :, :].reshape(-1, 3),
        arr[:, :b, :].reshape(-1, 3),
        arr[:, -b:, :].reshape(-1, 3),
    ], axis=0)
    bg_median = np.median(border_pixels, axis=0)

    # Foreground mask: pixels differing from neutral border background
    diff = np.linalg.norm(arr - bg_median, axis=2)
    fg_mask = diff > 0.11

    if np.any(fg_mask):
        fg = arr[fg_mask]
        luma = 0.2126 * fg[:, 0] + 0.7152 * fg[:, 1] + 0.0722 * fg[:, 2]
        # Lift dark shadow troughs (< 0.35) and soften harsh glare (> 0.88) while keeping hue/chroma
        target_luma = np.power(np.clip(luma, 1e-4, 1.0), 0.78)
        target_luma = np.clip(target_luma, 0.14, 0.90)
        gain = target_luma / np.maximum(luma, 1e-4)
        gain = np.clip(gain, 0.75, 1.85)[:, None]
        arr[fg_mask] = np.clip(fg * gain, 0.0, 1.0)

    # Flatten background vignette outside subject mask to uniform neutral studio gray (0.84)
    bg_mask = ~fg_mask
    if np.any(bg_mask):
        arr[bg_mask] = 0.84

    out = Image.fromarray(np.clip(arr * 255.0, 0, 255).astype(np.uint8), mode="RGB")
    out.save(img_path, format="PNG")


def make_seamless_and_derive_pbr_maps(albedo_path: Path, normal_strength: float = 2.4):
    """
    1. Enforces toroidal seamless wrap on `albedo_path` using a cosine cross-blend border band
       so the 1024x1024 texture tiles with zero visible seam.
    2. Derives a 1024x1024 Tangent-Space Normal Map (`<stem>_normal.png`).
    3. Derives a 1024x1024 Stylized Matte Roughness Map (`<stem>_roughness.png`, range [0.60, 0.94]).
    """
    im = Image.open(albedo_path).convert("RGB")
    arr = np.asarray(im, dtype=np.float32) / 255.0
    h, w, _ = arr.shape

    # Delight low-frequency lighting gradient across the texture tile
    luma = 0.2126 * arr[:, :, 0] + 0.7152 * arr[:, :, 1] + 0.0722 * arr[:, :, 2]
    mean_luma = float(np.mean(luma))
    # Box-blur low-frequency illumination field (64px kernel via cumulative sum / roll)
    low_freq = luma.copy()
    for shift in (8, 16, 32):
        low_freq = 0.25 * (
            np.roll(low_freq, shift, axis=0)
            + np.roll(low_freq, -shift, axis=0)
            + np.roll(low_freq, shift, axis=1)
            + np.roll(low_freq, -shift, axis=1)
        )
    corr = np.clip(mean_luma / np.maximum(low_freq, 0.08), 0.75, 1.35)[:, :, None]
    arr = np.clip(arr * corr, 0.0, 1.0)

    # Seamless toroidal edge synthesis: blend opposite edges over a 15% overlap band
    band_x = max(16, int(w * 0.15))
    band_y = max(16, int(h * 0.15))
    shifted_x = np.roll(arr, w // 2, axis=1)
    wx = np.ones((1, w, 1), dtype=np.float32)
    for i in range(band_x):
        t = 0.5 * (1.0 - np.cos(np.pi * (i / float(band_x))))
        wx[0, i, 0] = t
        wx[0, w - 1 - i, 0] = t
    arr = arr * wx + shifted_x * (1.0 - wx)

    shifted_y = np.roll(arr, h // 2, axis=0)
    wy = np.ones((h, 1, 1), dtype=np.float32)
    for j in range(band_y):
        t = 0.5 * (1.0 - np.cos(np.pi * (j / float(band_y))))
        wy[j, 0, 0] = t
        wy[h - 1 - j, 0, 0] = t
    arr = arr * wy + shifted_y * (1.0 - wy)

    # Save seamless delighted Albedo
    Image.fromarray(np.clip(arr * 255.0, 0, 255).astype(np.uint8), mode="RGB").save(albedo_path, format="PNG")

    # Derive height field from seamless luminance
    height_map = 0.2126 * arr[:, :, 0] + 0.7152 * arr[:, :, 1] + 0.0722 * arr[:, :, 2]

    # Periodic Sobel gradients (wrapping cleanly at tile edges)
    h_up = np.roll(height_map, -1, axis=0)
    h_dn = np.roll(height_map, 1, axis=0)
    h_lt = np.roll(height_map, -1, axis=1)
    h_rt = np.roll(height_map, 1, axis=1)

    dx = (h_lt - h_rt) * normal_strength
    dy = (h_up - h_dn) * normal_strength
    dz = np.ones_like(height_map)
    norm = np.sqrt(dx * dx + dy * dy + dz * dz)
    nx = -dx / norm
    ny = dy / norm
    nz = dz / norm

    nrm_rgb = np.stack([nx * 0.5 + 0.5, ny * 0.5 + 0.5, nz * 0.5 + 0.5], axis=-1)
    normal_path = albedo_path.with_name(f"{albedo_path.stem}_normal.png")
    Image.fromarray(np.clip(nrm_rgb * 255.0, 0, 255).astype(np.uint8), mode="RGB").save(normal_path, format="PNG")

    # Derive stylized matte roughness map: crevices/mortar rougher (0.94), raised stone/wood facets smoother (0.62)
    h_min, h_max = float(np.percentile(height_map, 5)), float(np.percentile(height_map, 95))
    h_norm = np.clip((height_map - h_min) / max(h_max - h_min, 1e-4), 0.0, 1.0)
    rough = 0.92 - 0.28 * h_norm
    rough_rgb = np.repeat(rough[:, :, None], 3, axis=-1)
    roughness_path = albedo_path.with_name(f"{albedo_path.stem}_roughness.png")
    Image.fromarray(np.clip(rough_rgb * 255.0, 0, 255).astype(np.uint8), mode="RGB").save(roughness_path, format="PNG")

    return {
        "albedo": str(albedo_path),
        "normal": str(normal_path),
        "roughness": str(roughness_path),
    }


def _start_comfy_server():
    py = COMFY_DIR / ".venv" / "bin" / "python"
    main_py = COMFY_DIR / "main.py"
    if not py.is_file() or not main_py.is_file():
        raise RuntimeError(f"ComfyUI not installed at {COMFY_DIR}")
    env = dict(os.environ)
    env["PYTORCH_ENABLE_MPS_FALLBACK"] = "1"
    proc = subprocess.Popen(
        [str(py), str(main_py), "--listen", "127.0.0.1", "--port", "8188", "--force-fp16"],
        cwd=str(COMFY_DIR),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=env,
    )
    deadline = time.time() + 60
    while time.time() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"ComfyUI exited early with code {proc.returncode}")
        if comfy_is_up():
            return proc
        time.sleep(0.5)
    proc.terminate()
    raise RuntimeError("Timed out waiting for ComfyUI on http://127.0.0.1:8188")


def main():
    # Machine-wide generation queue (tools/genq.py): one ComfyUI/TRELLIS job
    # at a time across every AI session. Re-enter through the queue unless
    # genq already gave us the slot.
    genq = Path(__file__).resolve().parent / "genq.py"
    if not os.environ.get("GENQ_SLOT") and genq.is_file() and "--free-only" not in sys.argv:
        label = "comfy_runner " + " ".join(sys.argv[1:])[:70]
        os.execv(sys.executable, [sys.executable, str(genq), "run", "--label", label, "--",
                                  sys.executable, str(Path(__file__).resolve()), *sys.argv[1:]])
    p = argparse.ArgumentParser(description="Run local ComfyUI for ElfStory Reborn V2 assets.")
    p.add_argument("--mode", choices=["concept", "pbr-texture", "texture", "refine-texture"], default="concept")
    p.add_argument("--prompt", default="", help="Subject or material description")
    p.add_argument("--input-image", default="", help="Input image path for refine-texture (img2img)")
    p.add_argument("--denoise", type=float, default=0.32, help="Img2img denoise strength")
    p.add_argument("--negative", default="", help="Optional extra negative prompt")
    p.add_argument("--output", default="", help="Output image path (.png)")
    p.add_argument("--checkpoint", default="", help="Checkpoint filename in ~/ComfyUI/models/checkpoints/")
    p.add_argument("--width", type=int, default=1024, help="Generation width (V2 default: 1024)")
    p.add_argument("--height", type=int, default=1024, help="Generation height (V2 default: 1024)")
    p.add_argument("--steps", type=int, default=4, help="Sampler steps (default: 4 for SDXL Turbo at 1024px)")
    p.add_argument("--cfg", type=float, default=1.35, help="CFG scale (default: 1.35 for SDXL Turbo at 1024px)")
    p.add_argument("--seed", type=int, default=-1)
    p.add_argument("--delight", action="store_true", help="Run the automatic concept delighting pass (OFF by default since Nov 22, 2026: the pass caused white smears on concepts, e.g. the first blight rat and likely the lynx cheek flecks)")
    p.add_argument("--no-delight", action="store_true", help="(compat) Delighting is off by default; this flag is a no-op")
    p.add_argument("--keep-running", action="store_true", help="Leave ComfyUI running after generation")
    p.add_argument("--keep-studio", action="store_true", help="Stay in STUDIO mode after generation")
    p.add_argument("--no-mode", action="store_true", help="Skip elfmode STUDIO/AGENT flip")
    p.add_argument("--free-only", action="store_true", help="Just call /free on running ComfyUI and exit")
    args = p.parse_args()

    if args.free_only:
        ok = free_comfyui()
        print(json.dumps({"ok": ok, "action": "free"}))
        return 0

    if not args.prompt or not args.output:
        p.error("--prompt and --output are required unless --free-only is set")

    ckpts = _list_checkpoints()
    if not ckpts:
        print(f"Error: no checkpoints found in {COMFY_DIR / 'models' / 'checkpoints'}", file=sys.stderr)
        return 1
    # Prefer non-turbo SDXL/Flux checkpoint if installed, else tuned 1024px SDXL Turbo
    non_turbo = [c for c in ckpts if "turbo" not in c.lower()]
    if args.checkpoint and args.checkpoint in ckpts:
        ckpt = args.checkpoint
    elif non_turbo:
        ckpt = non_turbo[0]
    else:
        ckpt = DEFAULT_CKPT if DEFAULT_CKPT in ckpts else ckpts[0]

    seed = args.seed if args.seed >= 0 else random.randint(1, 2_000_000_000)
    if args.mode == "concept":
        # Lead with the subject prompt so CLIP's front-weighted 77 tokens prioritize the asset itself!
        positive = args.prompt + CONCEPT_STYLE_SUFFIX
        negative = CONCEPT_NEGATIVE + (", " + args.negative if args.negative else "")
    elif args.mode == "refine-texture":
        if not args.input_image:
            p.error("--input-image is required for --mode refine-texture")
        positive = (
            "crisp stylized low-poly hand-painted fantasy RPG UV texture atlas, clean vibrant colors, "
            "bright daylight albedo, sharp wood grain and stone detail, even flat lighting, " + args.prompt
        )
        negative = TEXTURE_NEGATIVE + (", dark shadows, mud, noise, " + args.negative if args.negative else ", dark shadows, mud, noise")
    else:
        positive = TEXTURE_STYLE_PREFIX + args.prompt
        negative = TEXTURE_NEGATIVE + (", " + args.negative if args.negative else "")

    out_path = Path(os.path.expanduser(args.output)).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    flipped_mode = False
    started_proc = None
    try:
        _stop_music_studio()
        if not args.no_mode:
            st = _elfmode("status")
            if st.get("mode") != "studio":
                res = _elfmode("studio")
                if not res.get("ok"):
                    print(f"Warning: could not enter STUDIO mode ({res}); continuing...", file=sys.stderr)
                else:
                    flipped_mode = True

        if not comfy_is_up():
            print("Starting local ComfyUI server on http://127.0.0.1:8188 ...", file=sys.stderr)
            started_proc = _start_comfy_server()

        prefix = f"elfstory_{out_path.stem}"
        if args.mode == "refine-texture":
            uploaded_name = _upload_image(Path(os.path.expanduser(args.input_image)).resolve())
            wf = _build_sdxl_turbo_img2img_workflow(
                ckpt_name=ckpt,
                image_name=uploaded_name,
                positive=positive,
                negative=negative,
                steps=args.steps,
                cfg=args.cfg,
                denoise=args.denoise,
                seed=seed,
                prefix=prefix,
            )
        else:
            wf = _build_sdxl_workflow(
                ckpt_name=ckpt,
                positive=positive,
                negative=negative,
                width=args.width,
                height=args.height,
                steps=args.steps,
                cfg=args.cfg,
                seed=seed,
                prefix=prefix,
            )
        t0 = time.time()
        queued = _http_json(COMFY_URL + "/prompt", method="POST", payload={"prompt": wf}, timeout=15)
        prompt_id = queued.get("prompt_id")
        if not prompt_id:
            raise RuntimeError(f"ComfyUI rejected prompt: {queued}")

        deadline = time.time() + 240
        img_info = None
        while time.time() < deadline:
            hist = _http_json(f"{COMFY_URL}/history/{prompt_id}", timeout=10)
            entry = hist.get(prompt_id)
            if entry:
                status = entry.get("status", {})
                if status.get("status_str") == "error":
                    raise RuntimeError(f"ComfyUI execution error: {status}")
                outputs = entry.get("outputs", {})
                for node_out in outputs.values():
                    imgs = node_out.get("images") or []
                    if imgs:
                        img_info = imgs[0]
                        break
                if img_info:
                    break
            time.sleep(0.5)

        if not img_info:
            raise RuntimeError("Timed out waiting for ComfyUI image output")

        qs = urllib.parse.urlencode({
            "filename": img_info["filename"],
            "subfolder": img_info.get("subfolder", ""),
            "type": img_info.get("type", "output"),
        })
        with urllib.request.urlopen(f"{COMFY_URL}/view?{qs}", timeout=15) as r:
            out_path.write_bytes(r.read())

        pbr_maps = None
        if args.mode == "concept" and args.delight and not args.no_delight:
            delight_concept_image(out_path)
        elif args.mode in ("pbr-texture", "texture"):
            pbr_maps = make_seamless_and_derive_pbr_maps(out_path)

        elapsed = time.time() - t0
        size_kb = out_path.stat().st_size / 1024.0
        result = {
            "ok": True,
            "mode": args.mode,
            "checkpoint": ckpt,
            "seed": seed,
            "width": args.width,
            "height": args.height,
            "delighted": args.mode == "concept" and args.delight and not args.no_delight,
            "elapsed_s": round(elapsed, 2),
            "size_kb": round(size_kb, 1),
            "output": str(out_path),
        }
        if pbr_maps:
            result["pbr_maps"] = pbr_maps
        print(json.dumps(result))
        return 0
    finally:
        free_comfyui()
        if started_proc is not None and not args.keep_running:
            started_proc.terminate()
            try:
                started_proc.wait(timeout=5)
            except Exception:
                started_proc.kill()
        if flipped_mode and not args.keep_studio:
            _elfmode("agent")


if __name__ == "__main__":
    raise SystemExit(main())
