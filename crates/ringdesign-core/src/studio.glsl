// Camera-space jewellery studio, shared by GL 3.3 and GLES 3.0.
// Analytic softboxes approximate a reflected environment; this is not ray tracing.
// Metal uses coloured Fresnel reflectance, with no diffuse/paint component.
uniform float u_roughness;

float studio_card(vec3 ray, vec3 centre, vec2 extent, float blur) {
    vec3 axis = normalize(centre);
    vec3 right = normalize(cross(vec3(0.0, 1.0, 0.0), axis));
    vec3 up = cross(axis, right);
    float facing = dot(ray, axis);
    vec2 uv = vec2(dot(ray, right), dot(ray, up)) / max(facing, 0.001);
    vec2 edge = abs(uv) - extent;
    float distance = length(max(edge, vec2(0.0))) + min(max(edge.x, edge.y), 0.0);
    float diffuser = exp(-0.55 * dot(uv / extent, uv / extent));
    return (1.0 - smoothstep(-blur, blur, distance)) * diffuser * smoothstep(0.0, 0.15, facing);
}

vec3 studio_environment(vec3 ray, float roughness, vec3 key, float ambient) {
    float blur = 0.14 + roughness * roughness * 1.8;
    float ceiling = smoothstep(-0.45, 0.95, ray.y);
    vec3 room = mix(vec3(0.035, 0.04, 0.05), vec3(0.38, 0.39, 0.42), ceiling);
    room += vec3(0.16, 0.15, 0.13) * pow(max(ray.z, 0.0), 3.0);
    room *= clamp(ambient / 0.20, 0.5, 2.0);
    // A broad overhead box, a tall strip, and a cool rim give curved metal
    // both bright reflections and dark separation as it is orbited.
    room += vec3(3.6, 3.45, 3.15) * studio_card(ray, key, vec2(0.48, 0.85), blur);
    room += vec3(2.5, 2.65, 2.9) * studio_card(ray, vec3(0.9, 0.2, 0.4), vec2(0.13, 1.35), blur);
    room += vec3(1.6, 1.7, 1.85) * studio_card(ray, vec3(-0.5, 0.7, -0.8), vec2(0.9, 0.26), blur);
    room += vec3(0.65, 0.58, 0.46) * studio_card(ray, vec3(0.2, -0.8, 0.55), vec2(1.3, 0.22), blur);
    // Approximate the wider reflection lobe of satin/rough metal.
    return mix(room, vec3(0.62, 0.64, 0.68), roughness * roughness * 0.55);
}

vec3 studio_display(vec3 linear_color) {
    // Compress HDR highlights before encoding for egui's gamma-space target.
    vec3 mapped = linear_color / (vec3(1.0) + linear_color);
    return mix(12.92 * mapped, 1.055 * pow(mapped, vec3(1.0 / 2.4)) - 0.055,
               step(vec3(0.0031308), mapped));
}

vec3 studio_metal(vec3 n, vec3 f0, vec3 key, float ambient, float cavity) {
    float nv = clamp(n.z, 0.0, 1.0);
    vec3 fresnel = f0 + (vec3(1.0) - f0) * pow(1.0 - nv, 5.0);
    vec3 reflected = reflect(vec3(0.0, 0.0, -1.0), n);
    return studio_display(studio_environment(reflected, u_roughness, key, ambient) * fresnel * mix(1.0, 0.40, cavity));
}

vec3 studio_gem(vec3 n, vec3 tint, vec3 key, float ambient) {
    // Separate dielectric preview: neutral reflections over a coloured body.
    // Facet normals remain flat. Refraction/dispersion are not simulated.
    float nv = clamp(n.z, 0.0, 1.0);
    float fresnel = 0.055 + 0.945 * pow(1.0 - nv, 5.0);
    vec3 reflected = studio_environment(reflect(vec3(0.0, 0.0, -1.0), n), 0.06, key, ambient);
    float body = 0.12 + 0.65 * max(dot(n, normalize(key)), 0.0);
    return studio_display(tint * body * (1.0 - fresnel) + reflected * fresnel);
}
