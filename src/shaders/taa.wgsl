#import bevy_hikari::mesh_view_bindings
#import bevy_hikari::deferred_bindings
#import bevy_hikari::utils

@group(2) @binding(0)
var nearest_sampler: sampler;
@group(2) @binding(1)
var linear_sampler: sampler;

@group(3) @binding(0)
var previous_render_texture: texture_2d<f32>;
@group(3) @binding(1)
var render_texture: texture_2d<f32>;

@group(4) @binding(0)
var output_texture: texture_storage_2d<rgba16float, read_write>;

// The following 3 functions are from Playdead
// https://github.com/playdeadgames/temporal/blob/master/Assets/Shaders/TemporalReprojection.shader
fn RGB_to_YCoCg(rgb: vec3<f32>) -> vec3<f32> {
    let y = (rgb.r / 4.0) + (rgb.g / 2.0) + (rgb.b / 4.0);
    let co = (rgb.r / 2.0) - (rgb.b / 2.0);
    let cg = (-rgb.r / 4.0) + (rgb.g / 2.0) - (rgb.b / 4.0);
    return vec3<f32>(y, co, cg);
}

fn YCoCg_to_RGB(ycocg: vec3<f32>) -> vec3<f32> {
    let r = ycocg.x + ycocg.y - ycocg.z;
    let g = ycocg.x + ycocg.z;
    let b = ycocg.x - ycocg.y - ycocg.z;
    return clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn clip_towards_aabb_center(previous_color: vec3<f32>, current_color: vec3<f32>, aabb_min: vec3<f32>, aabb_max: vec3<f32>) -> vec3<f32> {
    let p_clip = 0.5 * (aabb_max + aabb_min);
    let e_clip = 0.5 * (aabb_max - aabb_min);
    let v_clip = previous_color - p_clip;
    let v_unit = v_clip.xyz / e_clip;
    let a_unit = abs(v_unit);
    let ma_unit = max(a_unit.x, max(a_unit.y, a_unit.z));
    return select(previous_color, p_clip + v_clip / ma_unit, ma_unit > 1.0);
}

fn sample_previous_render_texture(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(previous_render_texture, linear_sampler, uv, 0.0).rgb;
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sample_render_texture(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(render_texture, nearest_sampler, uv, 0.0).rgb;
    return RGB_to_YCoCg(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
}

fn nearest_velocity(uv: vec2<f32>) -> vec2<f32> {
    let texel_size = 1.0 / vec2<f32>(textureDimensions(render_texture));

    var depths: vec4<f32>;
    depths[0] = textureSampleLevel(position_texture, nearest_sampler, uv + vec2<f32>(texel_size.x, texel_size.y), 0.0).w;
    depths[1] = textureSampleLevel(position_texture, nearest_sampler, uv + vec2<f32>(-texel_size.x, texel_size.y), 0.0).w;
    depths[2] = textureSampleLevel(position_texture, nearest_sampler, uv + vec2<f32>(texel_size.x, -texel_size.y), 0.0).w;
    depths[3] = textureSampleLevel(position_texture, nearest_sampler, uv + vec2<f32>(-texel_size.x, -texel_size.y), 0.0).w;
    let max_depth = max(max(depths[0], depths[1]), max(depths[2], depths[3]));

    let depth = textureSampleLevel(position_texture, nearest_sampler, uv, 0.0).w;
    var offset: vec2<f32>;
    if depth < max_depth {
        let x = dot(vec4<f32>(texel_size.x), select(vec4<f32>(0.0), vec4<f32>(1.0, -1.0, 1.0, -1.0), depths == vec4<f32>(max_depth)));
        let y = dot(vec4<f32>(texel_size.y), select(vec4<f32>(0.0), vec4<f32>(1.0, 1.0, -1.0, -1.0), depths == vec4<f32>(max_depth)));
        offset = vec2<f32>(x, y);
    }

    return textureSampleLevel(velocity_uv_texture, nearest_sampler, uv + offset, 0.0).xy;
}

@compute @workgroup_size(8, 8, 1)
fn taa_jasmine(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let output_size = textureDimensions(output_texture);
    let deferred_size = textureDimensions(position_texture);
    let size = vec2<f32>(output_size);
    let texel_size = 1.0 / size;

    let coords = vec2<i32>(invocation_id.xy);
    let uv = coords_to_uv(coords, textureDimensions(output_texture));

    // Fetch the current sample
    let original_color = textureSampleLevel(render_texture, nearest_sampler, uv, 0.0);
    let current_color = original_color.rgb;

    // Reproject to find the equivalent sample from the past, using 5-tap Catmull-Rom filtering
    // from https://gist.github.com/TheRealMJP/c83b8c0f46b63f3a88a5986f4fa982b1
    // and https://www.activision.com/cdn/research/Dynamic_Temporal_Antialiasing_and_Upsampling_in_Call_of_Duty_v4.pdf#page=68
    var velocity = nearest_velocity(uv);
    let previous_uv = uv - velocity;
    let boundary_miss = any(abs(previous_uv - 0.5) > vec2<f32>(0.5));

    var uv_biases: array<vec2<f32>, 5>;
    uv_biases[0] = vec2<f32>(0.0);
    uv_biases[1] = vec2<f32>(1.5, 1.5) * texel_size;
    uv_biases[2] = vec2<f32>(-1.5, 1.5) * texel_size;
    uv_biases[3] = vec2<f32>(1.5, -1.5) * texel_size;
    uv_biases[4] = vec2<f32>(-1.5, -1.5) * texel_size;

    let current_position_depth = textureSampleLevel(position_texture, nearest_sampler, uv, 0.0);

    var has_content = current_position_depth.w > 0.0;
    var depth_miss = current_position_depth.w == 0.0;
    var position_miss = current_position_depth.w == 0.0;

    for (var i = 0u; i < 5u; i += 1u) {
        let previous_depths = textureGather(3, previous_position_texture, linear_sampler, previous_uv + uv_biases[i]);
        let depth_ratio = select(vec4<f32>(current_position_depth.w) / previous_depths, vec4<f32>(1.0), previous_depths == vec4<f32>(0.0));
        has_content = has_content || any(previous_depths > vec4<f32>(0.0));
        depth_miss = depth_miss || any(depth_ratio < vec4<f32>(0.95));

        let previous_position = textureSampleLevel(previous_position_texture, nearest_sampler, previous_uv + uv_biases[i], 0.0).xyz;
        position_miss = position_miss || distance(current_position_depth.xyz, previous_position) > 0.5;
    }

    if !has_content {
        textureStore(output_texture, coords, frame.clear_color);
        return;
    }

    let previous_velocity = textureSampleLevel(previous_velocity_uv_texture, nearest_sampler, previous_uv, 0.0).xy;
    let velocity_miss = distance(velocity, previous_velocity) > 0.00005;

    let sample_position = (uv - velocity) * size;
    let texel_position_1 = floor(sample_position - 0.5) + 0.5;
    let f = sample_position - texel_position_1;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let offset12 = w2 / (w1 + w2);
    let texel_position_0 = (texel_position_1 - 1.0) * texel_size;
    let texel_position_3 = (texel_position_1 + 2.0) * texel_size;
    let texel_position_12 = (texel_position_1 + offset12) * texel_size;
    var previous_color = vec3<f32>(0.0);
    previous_color += sample_previous_render_texture(vec2<f32>(texel_position_12.x, texel_position_0.y)) * w12.x * w0.y;
    previous_color += sample_previous_render_texture(vec2<f32>(texel_position_0.x, texel_position_12.y)) * w0.x * w12.y;
    previous_color += sample_previous_render_texture(vec2<f32>(texel_position_12.x, texel_position_12.y)) * w12.x * w12.y;
    previous_color += sample_previous_render_texture(vec2<f32>(texel_position_3.x, texel_position_12.y)) * w3.x * w12.y;
    previous_color += sample_previous_render_texture(vec2<f32>(texel_position_12.x, texel_position_3.y)) * w12.x * w3.y;

    if boundary_miss || (position_miss && velocity_miss && depth_miss) {
        // Constrain past sample with 3x3 YCoCg variance clipping to handle disocclusion
        let s_tl = sample_render_texture(uv + vec2<f32>(-texel_size.x, texel_size.y));
        let s_tm = sample_render_texture(uv + vec2<f32>(0.0, texel_size.y));
        let s_tr = sample_render_texture(uv + texel_size);
        let s_ml = sample_render_texture(uv - vec2<f32>(texel_size.x, 0.0));
        let s_mm = RGB_to_YCoCg(current_color);
        let s_mr = sample_render_texture(uv + vec2<f32>(texel_size.x, 0.0));
        let s_bl = sample_render_texture(uv - texel_size);
        let s_bm = sample_render_texture(uv - vec2<f32>(0.0, texel_size.y));
        let s_br = sample_render_texture(uv + vec2<f32>(texel_size.x, -texel_size.y));
        let moment_1 = s_tl + s_tm + s_tr + s_ml + s_mm + s_mr + s_bl + s_bm + s_br;
        let moment_2 = (s_tl * s_tl) + (s_tm * s_tm) + (s_tr * s_tr) + (s_ml * s_ml) + (s_mm * s_mm) + (s_mr * s_mr) + (s_bl * s_bl) + (s_bm * s_bm) + (s_br * s_br);
        let mean = moment_1 / 9.0;
        let variance = sqrt((moment_2 / 9.0) - (mean * mean));
        previous_color = RGB_to_YCoCg(previous_color);
        previous_color = clip_towards_aabb_center(previous_color, s_mm, mean - variance, mean + variance);
        previous_color = YCoCg_to_RGB(previous_color);
    }

    // Blend current and past sample
    var output = mix(previous_color, current_color, 0.1 / frame.upscale_ratio);

    textureStore(output_texture, coords, vec4<f32>(output, original_color.a));
}
