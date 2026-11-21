shader_type spatial;
render_mode cull_disabled;

// Stylized meadow grass for MultiMesh tufts (grass.gd).
// UV.y runs 0 at the blade root to 1 at the tip: colour ramps base -> tip
// and the wind sway grows with height squared, so roots stay planted.
// The MultiMesh instance colour (COLOR) tints each tuft for meadow
// variation. Normals point straight up so thin blades light like the
// ground under them instead of flickering as they face the sun or not.
// tint_only = 1 uses the instance colour as-is (flower heads).

uniform vec4 base_color : source_color = vec4(0.17, 0.37, 0.13, 1.0);
uniform vec4 tip_color : source_color = vec4(0.42, 0.68, 0.24, 1.0);
uniform float tint_only : hint_range(0.0, 1.0) = 0.0;
uniform float wind_strength = 0.07;
uniform float wind_speed = 1.3;

varying float blade_h;
varying vec3 tint;

void vertex() {
	blade_h = UV.y;
	tint = COLOR.rgb;
	vec3 wp = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	float t = TIME * wind_speed;
	// Slow rolling gusts across the field plus a faster per-blade flutter.
	float gust = sin(wp.x * 0.35 + wp.z * 0.2 + t) * 0.5 + 0.5;
	float flutter = sin(wp.x * 3.1 + wp.z * 2.3 + t * 2.7);
	vec3 offs = vec3(0.6 + 0.4 * flutter, 0.0, 0.3 * flutter) * gust * wind_strength * UV.y * UV.y;
	mat3 inv = inverse(mat3(MODEL_MATRIX));
	VERTEX += inv * offs;
	NORMAL = normalize(inv * vec3(0.0, 1.0, 0.0));
}

void fragment() {
	vec3 ramp = mix(base_color.rgb, tip_color.rgb, smoothstep(0.0, 1.0, blade_h));
	ALBEDO = mix(ramp * tint, tint, tint_only);
	ROUGHNESS = 0.9;
	SPECULAR = 0.15;
	// Low sun through the blade tips (golden hour).
	BACKLIGHT = vec3(0.16, 0.20, 0.08) * blade_h;
}
