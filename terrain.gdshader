shader_type spatial;

uniform sampler2D texture_albedo : source_color;
uniform sampler2D texture_normal : hint_normal;
uniform float texture_scale : hint_range(0.01, 100, 0.01);
uniform float texture_blend : hint_range(0, 1, 0.01);

void vertex() {
    // Transform vertex position to world space
    VERTEX = (MODELVIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
    // Calculate texture coordinates for each axis
    vec3 world_pos = (vec4(VERTEX, 1.0) * MODEL_MATRIX / texture_scale).xyz;
    vec2 tex_coords_x = world_pos.yz;
    vec2 tex_coords_y = world_pos.zx;
    vec2 tex_coords_z = world_pos.xy;

    // Sample textures using triplanar mapping
    vec4 tex_albedo_x = texture(texture_albedo, tex_coords_x);
    vec4 tex_albedo_y = texture(texture_albedo, tex_coords_y);
    vec4 tex_albedo_z = texture(texture_albedo, tex_coords_z);
    vec4 tex_normal_x = texture(texture_normal, tex_coords_x);
    vec4 tex_normal_y = texture(texture_normal, tex_coords_y);
    vec4 tex_normal_z = texture(texture_normal, tex_coords_z);

    // Blend textures based on surface normal
    vec3 normal = normalize(NORMAL);
    vec4 tex_albedo = mix(mix(tex_albedo_x, tex_albedo_y, normal.y), tex_albedo_z, normal.z);
    vec4 tex_normal = mix(mix(tex_normal_x, tex_normal_y, normal.y), tex_normal_z, normal.z);

    // Apply texture blend factor
    tex_albedo = mix(COLOR, tex_albedo, texture_blend);

    // Calculate final color using normal map
    vec3 normal_map = normalize(tex_normal.rgb * 2.0 - 1.0);
    vec3 light_dir = normalize(vec3(1.0, 1.0, 1.0));
    float diffuse = max(dot(normal, light_dir), 0.0);
    ALBEDO = tex_albedo.rgb * diffuse + vec3(0.1);
    ALPHA = tex_albedo.a;
}