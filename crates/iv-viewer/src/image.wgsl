//! 图像显示 shader：通道查看、棋盘格 Alpha 背景、HDR 曝光/tonemap、
//! 线性/最近邻采样切换。所有显示逻辑在 GPU 侧，切换零开销。

struct Uniforms {
    canvas_size: vec2f,     // 画布物理像素尺寸（= viewport，NDC [-1,1] 覆盖它）
    image_size: vec2f,      // 当前 mip 图像像素尺寸
    screen_offset: vec2f,   // 图像左上角在画布中的物理像素位置
    scale: f32,             // 物理像素 / 图像像素
    channel_mode: u32,      // 0 RGB / 1 R / 2 G / 3 B / 4 A / 5 RGB 不透明
    exposure: f32,          // HDR 曝光倍数
    flags: u32,             // bit0 nearest, bit1 棋盘格, bit2 HDR
    _pad: vec2f,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_image: texture_2d<f32>;
@group(0) @binding(2) var s_linear: sampler;
@group(0) @binding(3) var s_nearest: sampler;

struct VsOut {
    @builtin(position) position: vec4f,
    @location(0) screen_px: vec2f,   // 画布内物理像素坐标
    @location(1) img_px: vec2f,      // 图像像素坐标（浮点）
};

@vertex
fn vs_main(@location(0) corner: vec2f) -> VsOut {
    var out: VsOut;
    // viewport 已被 egui_wgpu 设为画布区域，corner∈[0,1]² 直接映射 NDC
    let px = corner * u.canvas_size; // 画布内物理像素坐标（y 向下）
    out.screen_px = px;
    out.img_px = (px - u.screen_offset) / u.scale;
    let ndc = vec2f(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0);
    out.position = vec4f(ndc, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    // 图像范围外：discard，透出 egui 背景
    if (in.img_px.x < 0.0 || in.img_px.y < 0.0
        || in.img_px.x >= u.image_size.x || in.img_px.y >= u.image_size.y) {
        discard;
    }
    let uv = in.img_px / u.image_size;

    var c: vec4f;
    if ((u.flags & 1u) != 0u) {
        c = textureSampleLevel(t_image, s_nearest, uv, 0.0);
    } else {
        c = textureSample(t_image, s_linear, uv);
    }

    // HDR：曝光 → Reinhard tonemap → gamma（WGSL 不支持 swizzle 赋值，用局部变量）
    if ((u.flags & 4u) != 0u) {
        let e = max(u.exposure, 0.0001);
        var hdr = c.rgb * e;
        hdr = hdr / (vec3f(1.0) + hdr);
        hdr = pow(max(hdr, vec3f(0.0)), vec3f(1.0 / 2.2));
        c = vec4f(hdr, c.a);
    }

    // 通道模式
    switch u.channel_mode {
        case 1u: { c = vec4f(c.rrr, 1.0); }
        case 2u: { c = vec4f(c.ggg, 1.0); }
        case 3u: { c = vec4f(c.bbb, 1.0); }
        case 4u: { c = vec4f(c.aaa, 1.0); }
        case 5u: { c = vec4f(c.rgb, 1.0); }
        default: {}
    }

    // RGB 模式下含 alpha：棋盘格背景混合
    if ((u.flags & 2u) != 0u && u.channel_mode == 0u && c.a < 1.0) {
        let cell = 8.0; // 物理像素
        let cx = floor(in.screen_px.x / cell);
        let cy = floor(in.screen_px.y / cell);
        let light = ((cx + cy) % 2.0) == 0.0;
        let bg = select(vec3f(0.78), vec3f(0.92), light);
        return vec4f(mix(bg, c.rgb, c.a), 1.0);
    }

    return c;
}
