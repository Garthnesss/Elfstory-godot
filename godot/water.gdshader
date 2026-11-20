shader_type spatial;

// Animated stylized water for the Oakhaven pond.
// Two scrolling sine waves mix a deep and a shallow blue; a third, faster
// band pushes foam-white glints onto the wave crests. World-space UVs keep
// the waves locked to the pond instead of stretching with the plane.

uniform vec4 shallow_color : source_color = vec4(0.25, 0.55, 0.78, 0.88);
uniform vec4 deep_color : source_color = vec4(0.04, 0.24, 0.48, 0.94);
uniform float wave_speed : hint_range(0.0, 4.0) = 1.2;

varying vec2 world_uv;

void vertex() {
	world_uv = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xz;
}

void fragment() {
	float w1 = sin(world_uv.x * 1.8 + TIME * wave_speed + sin(world_uv.y * 1.3 + TIME * 0.7));
	float w2 = sin(world_uv.y * 2.6 - TIME * wave_speed * 0.8 + world_uv.x * 0.9);
	float mixv = clamp(0.5 + 0.25 * w1 + 0.25 * w2, 0.0, 1.0);
	vec4 col = mix(deep_color, shallow_color, mixv);
	// Foam: bright band where the first wave crests.
	float foam = smoothstep(0.86, 0.99, w1 * 0.5 + 0.5);
	col = mix(col, vec4(0.95, 0.99, 1.0, col.a), foam * 0.45);
	ALBEDO = col.rgb;
	ALPHA = col.a;
	ROUGHNESS = 0.12;
	SPECULAR = 0.65;
}
