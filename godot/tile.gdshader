shader_type spatial;

// Painted-diorama tile material for the terrain MultiMeshes (Main.gd).
// The MultiMesh instance colour (COLOR) is the tile's base colour; this
// shader adds what flat boxes lack: per-tile tone jitter, painted mottling,
// a darkened rim on every face so each box reads as a bevelled block, and
// stone courses + a dark foot on tall sides so walls sit *in* the ground.
//
// Cutaway (tall MultiMesh only): fragments inside a cylinder from the
// player toward the camera dissolve with a screen-door dither, so walls
// and trees in front of the player never hide them or their telegraphs.

uniform float courses = 0.0;          // 1 = draw stone courses on tall sides
uniform float cutaway = 0.0;          // 1 = enable the camera cutaway
uniform vec3 player_pos = vec3(0.0);
uniform vec3 cam_pos = vec3(0.0, 30.0, 14.0); // real camera, also used in the shadow pass
uniform float cut_radius = 1.8;
uniform vec4 cap_tint : source_color = vec4(0.16, 0.15, 0.19, 1.0);

varying vec3 local_pos;
varying vec3 local_nrm;
varying vec3 world_pos;
varying vec3 tile_origin;
varying vec3 box_scale;
varying float interior;

float hash12(vec2 p) {
	vec3 p3 = fract(vec3(p.xyx) * 0.1031);
	p3 += dot(p3, p3.yzx + 33.33);
	return fract((p3.x + p3.y) * p3.z);
}

float vnoise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash12(i), hash12(i + vec2(1.0, 0.0)), u.x),
		mix(hash12(i + vec2(0.0, 1.0)), hash12(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
	return vnoise(p) * 0.55 + vnoise(p * 2.13 + 7.1) * 0.3 + vnoise(p * 4.7 + 3.3) * 0.15;
}

// 4x4 Bayer threshold for the screen-door cutaway.
float bayer4(vec2 frag) {
	int x = int(mod(frag.x, 4.0));
	int y = int(mod(frag.y, 4.0));
	int idx = x + y * 4;
	float m[16] = float[16](0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0,
		3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
	return (m[idx] + 0.5) / 16.0;
}

void vertex() {
	local_pos = VERTEX;
	interior = INSTANCE_CUSTOM.r;
	local_nrm = NORMAL;
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	tile_origin = MODEL_MATRIX[3].xyz;
	box_scale = vec3(length(MODEL_MATRIX[0].xyz), length(MODEL_MATRIX[1].xyz), length(MODEL_MATRIX[2].xyz));
}

void fragment() {
	vec3 base = COLOR.rgb;
	// Per-tile tone jitter: no two blocks are quite the same paint.
	float jitter = hash12(floor(tile_origin.xz) + 0.37) - 0.5;
	base *= 1.0 + jitter * 0.14;

	bool top = local_nrm.y > 0.5;
	vec2 face_uv = top ? world_pos.xz : (abs(local_nrm.x) > 0.5 ? world_pos.zy : world_pos.xy);
	float mott = fbm(face_uv * 2.6 + tile_origin.xz * 0.13);
	base *= mix(0.86, 1.1, mott);

	// Rim: distance to the nearest edge of the face, in world units.
	vec3 half_ext = box_scale * 0.5;
	vec3 d = half_ext - abs(local_pos * box_scale);
	float edge;
	if (top) {
		edge = min(d.x, d.z);
	} else if (abs(local_nrm.x) > 0.5) {
		edge = min(d.z, d.y);
	} else {
		edge = min(d.x, d.y);
	}
	float rim = smoothstep(0.0, 0.07, edge);
	base *= mix(0.68, 1.0, rim);

	if (!top) {
		// Dark foot where the side meets the floor (fake contact shadow).
		float foot = smoothstep(0.0, 0.55, world_pos.y);
		base *= mix(0.55, 1.0, foot);
		if (courses > 0.5) {
			float fy = world_pos.y * 2.2;
			float row = floor(fy);
			float av = ((abs(local_nrm.x) > 0.5 ? world_pos.z : world_pos.x) + mod(row, 2.0) * 0.45) * 1.1;
			float dh = min(fract(fy), 1.0 - fract(fy)) / 2.2;
			float dv = min(fract(av), 1.0 - fract(av)) / 1.1;
			float grout = 1.0 - smoothstep(0.012, 0.03, min(dh, dv));
			base *= mix(1.0, 0.7, grout);
			base *= 1.0 + (hash12(vec2(row, floor(av))) - 0.5) * 0.14;
		}
	}

	if (cutaway > 0.5) {
		vec3 cam = cam_pos;
		vec3 a = player_pos + vec3(0.0, 0.6, 0.0);
		vec3 ab = cam - a;
		float t = clamp(dot(world_pos - a, ab) / dot(ab, ab), 0.0, 1.0);
		float dist = length(world_pos - (a + ab * t));
		// Only geometry between the player and the camera, above their feet.
		float inside = (1.0 - smoothstep(cut_radius * 0.55, cut_radius, dist)) * step(0.04, t)
			* step(0.15, world_pos.y);
		if (inside > bayer4(FRAGCOORD.xy)) {
			discard;
		}
	}

	if (top && interior > 0.5) {
		// Rock enclosed by rock: one dark earth mass the rooms are carved
		// into. No per-block rim, so it never reads as a grid of boxes.
		float m2 = fbm(world_pos.xz * 0.9);
		base = cap_tint.rgb * mix(0.75, 1.2, m2) * mix(0.9, 1.05, mott);
	}

	ALBEDO = base;
	ROUGHNESS = 0.92;
	SPECULAR = 0.25;
}
