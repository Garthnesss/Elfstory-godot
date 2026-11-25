shader_type spatial;

// Outdoor tree parts (world_props.gd). Instance colour is the base; a soft top
// light and darker underside give the puffs volume, and canopies sway a little.

uniform float sway = 0.0;
uniform float fog_on = 0.0;
uniform sampler2D fog_tex : filter_linear, repeat_disable;
uniform vec2 fog_size = vec2(1.0);

varying vec3 tint;
varying float up;
varying vec2 spot;

void vertex() {
	tint = COLOR.rgb;
	up = NORMAL.y * 0.5 + 0.5;
	spot = MODEL_MATRIX[3].xz;
	vec3 wp = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	VERTEX.x += sin(TIME * 1.1 + wp.x * 0.7 + wp.z * 0.5) * sway * wp.y;
}

void fragment() {
	float seen = mix(1.0, texture(fog_tex, (spot + 0.5) / fog_size).r, fog_on);
	if (seen < 0.02) {
		discard;
	}
	vec3 c = tint * mix(0.62, 1.18, up);
	ALBEDO = mix(vec3(dot(c, vec3(0.3, 0.59, 0.11))) * vec3(0.42, 0.45, 0.58), c, smoothstep(0.40, 0.95, seen));
	ROUGHNESS = 0.95;
	SPECULAR = 0.1;
}
