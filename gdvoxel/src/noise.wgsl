@group(0)
@binding(0)
var output_noise: texture_storage_3d<r32float, write>; // this is used as both input and output for convenience

struct NoiseSettings {
    frequency: f32,
    offset_x: f32,
    offset_y: f32,
    offset_z: f32,
    generated_size_x: f32,
    generated_size_y: f32,
    generated_size_z: f32,
};

// uniform
@group(0)
@binding(1)
var<uniform> settings: NoiseSettings;

fn permute(x: vec4<f32>) -> vec4<f32> {
    return ((x * 34.0) + 1.0) * x % 289.0;
}

fn taylorInvSqrt(r: vec4<f32>) -> vec4<f32> {
    return 1.79284291400159 - 0.85373472095314 * r;
}

fn snoise(v: vec3<f32>) -> f32 {
    let C: vec2<f32> = vec2<f32>(1.0 / 6.0, 1.0 / 3.0);
    let D: vec4<f32> = vec4<f32>(0.0, 0.5, 1.0, 2.0);

    var i: vec3<f32> = floor(v + dot(v, C.yyy));
    var x0: vec3<f32> = v - i + dot(i, C.xxx);

    var g: vec3<f32> = step(x0.yzx, x0.xyz);
    var l: vec3<f32> = 1.0 - g;
    var i1: vec3<f32> = min(g.xyz, l.zxy);
    var i2: vec3<f32> = max(g.xyz, l.zxy);

    var x1: vec3<f32> = x0 - i1 + 1.0 * C.xxx;
    var x2: vec3<f32> = x0 - i2 + 2.0 * C.xxx;
    var x3: vec3<f32> = x0 - 1. + 3.0 * C.xxx;

    i = i % 289.0 ;
    var p: vec4<f32> = permute(permute(permute(
        i.z + vec4<f32>(0.0, i1.z, i2.z, 1.0)
    ) + i.y + vec4<f32>(0.0, i1.y, i2.y, 1.0)) + i.x + vec4<f32>(0.0, i1.x, i2.x, 1.0));

    let n_: f32 = 1.0 / 7.0;
    var ns: vec3<f32> = n_ * D.wyz - D.xzx;

    var j: vec4<f32> = p - 49.0 * floor(p * ns.z * ns.z);

    var x_: vec4<f32> = floor(j * ns.z);
    var y_: vec4<f32> = floor(j - 7.0 * x_);

    var x: vec4<f32> = x_ * ns.x + ns.yyyy;
    var y: vec4<f32> = y_ * ns.x + ns.yyyy;
    var h: vec4<f32> = 1.0 - abs(x) - abs(y);

    var b0: vec4<f32> = vec4<f32>(x.xy, y.xy);
    var b1: vec4<f32> = vec4<f32>(x.zw, y.zw);

    var s0: vec4<f32> = floor(b0) * 2.0 + 1.0;
    var s1: vec4<f32> = floor(b1) * 2.0 + 1.0;
    var sh: vec4<f32> = -step(h, vec4<f32>(0.0));

    var a0: vec4<f32> = b0.xzyw + s0.xzyw * sh.xxyy ;
    var a1: vec4<f32> = b1.xzyw + s1.xzyw * sh.zzww ;

    var p0: vec3<f32> = vec3<f32>(a0.xy, h.x);
    var p1: vec3<f32> = vec3<f32>(a0.zw, h.y);
    var p2: vec3<f32> = vec3<f32>(a1.xy, h.z);
    var p3: vec3<f32> = vec3<f32>(a1.zw, h.w);

    var norm: vec4<f32> = taylorInvSqrt(vec4<f32>(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));
    p0 *= norm.x;
    p1 *= norm.y;
    p2 *= norm.z;
    p3 *= norm.w;

    var m: vec4<f32> = max(0.6 - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), vec4<f32>(0.0));
    m = m * m;
    return 42.0 * dot(m * m, vec4<f32>(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}

const m3: mat3x3<f32> = mat3x3<f32>(vec3<f32>(0.8, 0.6, 0.0), vec3<f32>(-0.6, 0.8, 0.0), vec3<f32>(0.0, 0.0, 1.0));
fn fbm(p: vec3<f32>) -> f32 {
    var p_mut = p;
    var f: f32 = 0.0;
    f = f + 0.5000 * snoise(p_mut); p_mut = m3 * p_mut * 2.02;
    f = f + 0.2500 * snoise(p_mut); p_mut = m3 * p_mut * 2.03;
    f = f + 0.1250 * snoise(p_mut); p_mut = m3 * p_mut * 2.01;
    f = f + 0.0625 * snoise(p_mut);
    return f / 0.9375;
}

fn cubic_interpolation(points: array<vec2<f32>, 4>, x: f32) -> f32 {
    let a: vec2<f32> = points[1];
    let b: vec2<f32> = points[2] - points[0];
    let c: vec2<f32> = 2.0 * points[0] - 5.0 * points[1] + 4.0 * points[2] - points[3];
    let d: vec2<f32> = -points[0] + 3.0 * points[1] - 3.0 * points[2] + points[3];
    let t: f32 = x - a.x;
    let t2: f32 = t * t;
    let t3: f32 = t2 * t;
    return a.y + b.y * t + c.y * t2 + d.y * t3;
}

const SURFACE_SPLINE = array<vec2<f32>, 4>(
    vec2<f32>(-1.0, 0.0),
    vec2<f32>(0.3, 40.0),
    vec2<f32>(0.4, 55.0),
    vec2<f32>(1.0, 60.0)
);

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    
    // if any ids are greater than generated_size
    if global_id.x >= u32(settings.generated_size_x) || global_id.y >= u32(settings.generated_size_y) || global_id.z >= u32(settings.generated_size_z) {
        return;
    }

    let tex_dims = textureDimensions(output_noise);
    var noise_coord = vec3<f32>(f32(global_id.x), f32(global_id.y), f32(global_id.z)) * vec3<f32>(settings.frequency);
    noise_coord.x += settings.offset_x;
    noise_coord.y += settings.offset_y;
    noise_coord.z += settings.offset_z;

    var surface_noise = cubic_interpolation(SURFACE_SPLINE, fbm(vec3<f32>(noise_coord.x * 0.2, 0.0, noise_coord.z * 0.2)));

    var density_offset = clamp(surface_noise - (f32(global_id.y) - 150.0), -1.0, 0.2);

    var sdf_value = vec4<f32>(clamp(fbm(noise_coord * 0.5) + density_offset, -1.0, 1.0), 0.0, 0.0, 1.0);
    textureStore(output_noise, vec3<i32>(i32(global_id.x), i32(global_id.y), i32(global_id.z)), sdf_value);
}