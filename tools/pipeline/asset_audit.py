#!/usr/bin/env python3
"""Automated half of the Asset Gate: budgets, provenance, licenses, import sidecars.

    asset_audit.py <dir containing project.godot>

Reads limits from pipeline.json. Standard library only (Python 3.9+). Exit 0 = pass.
This never replaces looking at the asset: the visual gate in ASSETS.md is still yours.
"""
import csv
import json
import re
import struct
import sys
from pathlib import Path

TRACKED = {".glb", ".gltf", ".png", ".jpg", ".jpeg", ".webp", ".exr", ".hdr", ".wav", ".ogg", ".mp3", ".ttf", ".otf"}
IMAGES = {".png", ".jpg", ".jpeg", ".webp"}
REQUIRED = ["asset_path", "tier", "route", "generator", "license", "raw_source_path",
            "exception", "human_edit_level", "approved_by", "date"]
# Terms that cannot ship in a commercial game without a separate written clearance.
BLOCKED = [(r"cc[ -]?by[ -]?nc", "non-commercial license"),
           (r"hunyuan3d", "Hunyuan3D license excludes EU, UK and South Korea"),
           (r"rmbg[ -]?2", "RMBG-2.0 is CC BY-NC"),
           (r"non[ -]?commercial", "non-commercial license")]
UNKNOWN = {"", "?", "unknown", "tbd", "todo", "pending"}


class BadGlb(Exception):
    pass


def read_glb(path):
    data = path.read_bytes()
    if len(data) < 20 or data[:4] != b"glTF":
        raise BadGlb("not a binary glTF file (a Git LFS pointer? run `git lfs pull`)")
    length, kind = struct.unpack_from("<II", data, 12)
    if kind != 0x4E4F534A or 20 + length > len(data):
        raise BadGlb("damaged JSON chunk")
    doc = json.loads(data[20:20 + length].decode("utf-8"))
    body = b""
    at = 20 + length
    if at + 8 <= len(data):
        blen, _ = struct.unpack_from("<II", data, at)
        body = data[at + 8:at + 8 + blen]
    return doc, body


def image_size(b):
    """(width, height) from PNG, JPEG or WebP bytes, or None."""
    if b[:8] == b"\x89PNG\r\n\x1a\n" and len(b) >= 24:
        return struct.unpack(">II", b[16:24])
    if b[:4] == b"RIFF" and b[8:12] == b"WEBP" and len(b) >= 30:
        tag = b[12:16]
        if tag == b"VP8X":
            return 1 + int.from_bytes(b[24:27], "little"), 1 + int.from_bytes(b[27:30], "little")
        if tag == b"VP8 ":
            w, h = struct.unpack_from("<HH", b, 26)
            return w & 0x3FFF, h & 0x3FFF
        if tag == b"VP8L":
            bits = int.from_bytes(b[21:25], "little")
            return (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1
    if b[:2] == b"\xff\xd8":
        i = 2
        while i + 9 < len(b):
            if b[i] != 0xFF:
                i += 1
                continue
            m = b[i + 1]
            if 0xC0 <= m <= 0xCF and m not in (0xC4, 0xC8, 0xCC):
                h, w = struct.unpack(">HH", b[i + 5:i + 9])
                return w, h
            if m in (0xFF, 0x01) or 0xD0 <= m <= 0xD9:
                i += 1 if m == 0xFF else 2
                continue
            i += 2 + struct.unpack(">H", b[i + 2:i + 4])[0]
    return None


def triangles(doc):
    total = 0
    acc = doc.get("accessors", [])
    for mesh in doc.get("meshes", []):
        for p in mesh.get("primitives", []):
            mode = p.get("mode", 4)
            idx = p.get("indices", p.get("attributes", {}).get("POSITION"))
            if idx is None or idx >= len(acc):
                continue
            n = acc[idx].get("count", 0)
            total += n // 3 if mode == 4 else max(0, n - 2) if mode in (5, 6) else 0
    return total


def glb_images(doc, body):
    views = doc.get("bufferViews", [])
    for i, img in enumerate(doc.get("images", [])):
        v = img.get("bufferView")
        if v is None or v >= len(views):
            continue
        off = views[v].get("byteOffset", 0)
        yield img.get("name") or "image%d" % i, image_size(body[off:off + views[v]["byteLength"]])


def limit(table, route):
    return table.get(route, table.get("default")) if isinstance(table, dict) else table


def load_rows(root, errors):
    for rel in ("docs/asset_provenance.csv", "../docs/asset_provenance.csv"):
        path = root / rel
        if path.is_file():
            break
    else:
        return {}, None
    with path.open(newline="", encoding="utf-8-sig") as f:
        reader = csv.DictReader(f)
        missing = [c for c in REQUIRED if c not in (reader.fieldnames or [])]
        if missing:
            errors.append("%s is missing columns: %s" % (rel, ", ".join(missing)))
        rows = {}
        for r in reader:
            key = (r.get("asset_path") or "").strip().replace("res://", "")
            if rel.startswith("../") and key.startswith(root.name + "/"):
                key = key[len(root.name) + 1:]  # repo-relative path, project in a subfolder
            if key in rows:
                errors.append("%s lists %s twice" % (rel, key))
            rows[key] = {k: (v or "").strip() for k, v in r.items() if isinstance(k, str) and not isinstance(v, list)}
    return rows, rel


def audit(root):
    errors, warnings = [], []
    cfg = json.loads((root / "pipeline.json").read_text())
    a = cfg.get("assets", {})
    phase = int(cfg.get("phase", 0))
    rows, csv_rel = load_rows(root, errors)
    ship, proxy = a.get("ship_dirs", []), a.get("proxy_dirs", [])
    checked, seen = 0, set()
    for base, is_ship in [(d, True) for d in ship] + [(d, False) for d in proxy]:
        folder = root / base
        if not folder.is_dir():
            continue
        for path in sorted(folder.rglob("*")):
            if path.suffix.lower() not in TRACKED or not path.is_file():
                continue
            # Godot writes a model's textures out beside it: same asset, same row.
            if path.suffix.lower() in IMAGES and any(
                    path.name.startswith(g.stem + "_") for g in path.parent.glob("*.glb")):
                continue
            checked += 1
            rel = path.relative_to(root).as_posix()
            seen.add(rel)
            row = rows.get(rel)
            waived = bool(row and row.get("exception"))
            over = warnings if waived else errors
            if rel != rel.lower() or " " in rel:
                errors.append("%s: use snake_case (exports are case-sensitive, this Mac is not)" % rel)
            if not path.with_name(path.name + ".import").is_file():
                errors.append("%s: no .import file beside it (open the project once, then commit it)" % rel)
            if row is None:
                errors.append("%s: no row in %s" % (rel, csv_rel or "docs/asset_provenance.csv"))
            else:
                text = (row.get("license", "") + " " + row.get("generator", "")).lower()
                for pattern, why in BLOCKED:
                    if re.search(pattern, text):
                        errors.append("%s: %s" % (rel, why))
                        break
                if is_ship:
                    for col in ("license", "generator", "route"):
                        if row.get(col, "").lower() in UNKNOWN:
                            errors.append("%s: provenance '%s' is not filled in" % (rel, col))
                    if row.get("approved_by", "").lower() in UNKNOWN or row.get("approved_by", "").lower().startswith("pending"):
                        (warnings if a.get("approval") == "warn" else errors).append(
                            "%s: in a ship folder but not approved" % rel)
            route = (row or {}).get("route", "default")
            kb = path.stat().st_size / 1024
            if path.suffix.lower() == ".glb":
                try:
                    doc, body = read_glb(path)
                except (BadGlb, ValueError, struct.error) as e:
                    errors.append("%s: %s" % (rel, e))
                    continue
                max_kb = limit(a.get("max_glb_kb"), route) if is_ship else a.get("max_proxy_kb")
                if max_kb and kb > max_kb:
                    over.append("%s: %.0f KB, budget %d KB" % (rel, kb, max_kb))
                if is_ship:
                    tris, cap = triangles(doc), limit(a.get("max_tris"), route)
                    if cap and tris > cap:
                        over.append("%s: %d triangles, budget %d" % (rel, tris, cap))
                    for name, size in glb_images(doc, body):
                        if size and max(size) > a.get("max_texture_px", 2048):
                            over.append("%s: texture %s is %dx%d" % (rel, name, size[0], size[1]))
                    for m in doc.get("materials", []):
                        if m.get("doubleSided"):
                            warnings.append("%s: material %s is double-sided (costs fill rate)" % (rel, m.get("name", "?")))
                        if "normalTexture" in m and "occlusionTexture" not in m:
                            warnings.append("%s: material %s has no packed occlusion (ORM)" % (rel, m.get("name", "?")))
            elif is_ship and path.suffix.lower() in (".png", ".jpg", ".jpeg", ".webp"):
                size = image_size(path.read_bytes())
                if size and max(size) > a.get("max_texture_px", 2048):
                    over.append("%s: %dx%d, budget %d px" % (rel, size[0], size[1], a.get("max_texture_px", 2048)))
    for key in rows:
        if key and key not in seen and not (root / key).exists() and not (root.parent / key).exists():
            warnings.append("%s lists a file that does not exist: %s" % (csv_rel, key))
    if phase >= 3 and cfg.get("ai_policy", "undecided") == "undecided" and any(
            re.search(r"trellis|comfy|sdxl|flux|ace-step|generat", r.get("generator", ""), re.I) for r in rows.values()):
        errors.append("pipeline.json ai_policy is still 'undecided' in production, and AI-made assets exist")
    for group in (ship, proxy):
        for d in group:
            if d.startswith("/") or ".." in d:
                errors.append("pipeline.json: asset folder '%s' must be inside the project" % d)
    return checked, errors, warnings


def main():
    if len(sys.argv) != 2 or not (Path(sys.argv[1]) / "pipeline.json").is_file():
        print("usage: asset_audit.py <dir containing project.godot and pipeline.json>", file=sys.stderr)
        return 2
    checked, errors, warnings = audit(Path(sys.argv[1]).resolve())
    for w in warnings:
        print("warn asset %s" % w)
    for e in errors:
        print("FAIL asset %s" % e)
    if not errors:
        print("ok   assets (%d checked)" % checked)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
