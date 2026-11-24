shader_type spatial;

// Painterly spring-pond water for Oakhaven: soft layered ripples, mossy-teal
// depths, warm shallow shoreline tint, and subtle golden-hour glints.

uniform vec4 shallow_color : source_color = vec4(0.18, 0.46, 0.52, 0.86);
uniform vec4 deep_color : source_color = vec4(0.06, 0.22, 0.34, 0.94);
uniform float wave_speed : hint_range(0.0, 4.0) = 0.85;

varying vec2 world_uv;

void vertex() {
	world_uv = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xz;
}

void fragment() {
	float w1 = sin(world_uv.x * 2.4 + TIME * wave_speed + sin(world_uv.y * 1.9 + TIME * 0.6));
	float w2 = sin(world_uv.y * 3.1 - TIME * wave_speed * 0.75 + world_uv.x * 1.4);
	float w3 = sin((world_uv.x + world_uv.y) * 4.5 + TIME * 1.3);
	float mixv = clamp(0.48 + 0.22 * w1 + 0.18 * w2 + 0.10 * w3, 0.0, 1.0);
	vec4 col = mix(deep_color, shallow_color, mixv);
	float glint = smoothstep(0.90, 0.995, (w1 * 0.5 + 0.5) * (w3 * 0.5 + 0.5));
	col = mix(col, vec4(0.96, 0.92, 0.78, col.a), glint * 0.32);
	ALBEDO = col.rgb;
	ALPHA = col.a;
	ROUGHNESS = 0.16;
	SPECULAR = 0.55;
}
