"""Apply the Mira brightness lift ((v^0.85)*1.28) to the embedded Albedo
image of a .glb, in place. Repacks the BIN chunk with corrected offsets."""
import io, json, struct, sys
from PIL import Image

path = sys.argv[1]
gamma = float(sys.argv[2]) if len(sys.argv) > 2 else 0.85
gain = float(sys.argv[3]) if len(sys.argv) > 3 else 1.28

with open(path, "rb") as f:
    data = bytearray(f.read())
assert data[:4] == b"glTF", "not a glb"
jlen = struct.unpack_from("<I", data, 12)[0]
gltf = json.loads(bytes(data[20:20 + jlen]))
bin_off = 20 + jlen
blen = struct.unpack_from("<I", data, bin_off)[0]
bin_data = bytearray(data[bin_off + 8: bin_off + 8 + blen])

# find the Albedo image's bufferView
alb = None
for img in gltf["images"]:
    if img.get("name") == "Albedo":
        alb = img
        break
assert alb is not None, "no Albedo image"
bv = gltf["bufferViews"][alb["bufferView"]]
off = bv.get("byteOffset", 0)
old = bytes(bin_data[off: off + bv["byteLength"]])
im = Image.open(io.BytesIO(old)).convert("RGB")
lut = [min(255, int(round(((i / 255.0) ** gamma) * gain * 255))) for i in range(256)]
im = im.point(lut * 3)
out = io.BytesIO()
im.save(out, "WEBP", quality=90)
new = out.getvalue()

# rebuild BIN: replace that bufferView's bytes, shift later views
delta = len(new) - bv["byteLength"]
new_bin = bin_data[:off] + new + bin_data[off + bv["byteLength"]:]
for i, v in enumerate(gltf["bufferViews"]):
    vo = v.get("byteOffset", 0)
    if i != alb["bufferView"] and vo > off:
        v["byteOffset"] = vo + delta
bv["byteLength"] = len(new)
gltf["buffers"][0]["byteLength"] = len(new_bin)
# accessors with bufferView offsets into shifted views keep their own
# byteOffset (relative to view start), so nothing else changes.

pad = (-len(new_bin)) % 4
new_bin += b"\x00" * pad
j = json.dumps(gltf, separators=(",", ":")).encode()
jpad = (-len(j)) % 4
j += b" " * jpad
total = 12 + 8 + len(j) + 8 + len(new_bin)
out_data = struct.pack("<4sII", b"glTF", 2, total)
out_data += struct.pack("<I4s", len(j), b"JSON") + j
out_data += struct.pack("<I4s", len(new_bin), b"BIN\x00") + bytes(new_bin)
with open(path, "wb") as f:
    f.write(out_data)
print(f"LIFT_OK {path} albedo {bv['byteLength']} bytes, glb {total} bytes")
