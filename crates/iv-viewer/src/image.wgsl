//! 统一场景 shader：先合成画布、透明衬底和图像，再仅在玻璃遮罩内模糊。
//! 这样面板跨过图片边缘时仍采样同一张完整画布，不会出现材质断层。

struct Uniforms {
    canvas_size: vec2f,     // 画布物理像素尺寸（= viewport，NDC [-1,1] 覆盖它）
    image_size: vec2f,      // 当前 mip 图像像素尺寸
    screen_offset: vec2f,   // 图像左上角在画布中的物理像素位置
    scale: f32,             // 物理像素 / 图像像素
    channel_mode: u32,      // 0 RGB / 1 R / 2 G / 3 B / 4 A / 5 RGB 不透明
    exposure: f32,          // HDR 曝光倍数
    flags: u32,             // bit0 nearest, bit1 棋盘格, bit2 HDR
    glass_count: u32,       // 玻璃区域数量（0..=6）
    _pad: f32,
    canvas_top: vec4f,      // 回退画布顶部颜色（线性 RGBA）
    canvas_bottom: vec4f,   // 回退画布底部颜色（线性 RGBA）
    backdrop_params: vec4f, // 亮度、tint、深色标记、面板模糊物理半径
    glass_rects: array<vec4f, 8>,   // min.xy, max.xy（画布物理像素）
    glass_alpha: array<vec4f, 2>,   // 每区域模糊混合系数（打包 2×vec4）
    glass_corner: array<vec4f, 2>,  // 每区域圆角半径（画布物理像素，打包 2×vec4）
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_image: texture_2d<f32>;
@group(0) @binding(2) var s_linear: sampler;
@group(0) @binding(3) var s_nearest: sampler;
@group(0) @binding(4) var t_backdrop: texture_2d<f32>;

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

/// 当前像素被玻璃区域覆盖的程度（0 = 无覆盖；随悬浮层淡入淡出）。
/// 用圆角矩形有向距离场（SDF）判定 + 2px 边缘羽化：模糊严格收敛在悬浮层
/// 圆角轮廓内——既消除直角硬边，又不向圆角/投影区渗出（按钮投影保持锐利）。
fn glass_alpha_at(px: vec2f) -> f32 {
    var a = 0.0;
    for (var i = 0u; i < u.glass_count; i = i + 1u) {
        let r = u.glass_rects[i];
        let center = (r.xy + r.zw) * 0.5;
        let half = (r.zw - r.xy) * 0.5;
        // 圆角半径不能超半宽/半高（瘦高/扁宽区域退化为胶囊圆头）
        let cr = min(u.glass_corner[i / 4u][i % 4u], min(half.x, half.y));
        let q = abs(px - center) - (half - vec2f(cr, cr));
        let sdf = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - cr;
        // sdf<0 在内部；向内 2px 才满权重，边缘 2px 平滑淡出（消除硬边）
        let inside = 1.0 - smoothstep(-2.0, 0.0, sdf);
        a = max(a, u.glass_alpha[i / 4u][i % 4u] * inside);
    }
    return a;
}

/// 画布背景：桌面快照（已由 CPU 降采样模糊）或主题渐变。
fn canvas_at(screen_px: vec2f) -> vec3f {
    if ((u.flags & 8u) != 0u) {
        let uv = clamp(screen_px / u.canvas_size, vec2f(0.0), vec2f(1.0));
        var c = textureSampleLevel(t_backdrop, s_linear, uv, 0.0).rgb;
        let brightness = clamp(u.backdrop_params.x, 0.5, 1.5);
        if (brightness <= 1.0) {
            c *= brightness;
        } else {
            let lift = clamp((brightness - 1.0) / 0.5, 0.0, 1.0) * (110.0 / 255.0);
            c = mix(c, vec3f(1.0), lift);
        }
        let tint = clamp(u.backdrop_params.y, 0.0, 1.0) * (220.0 / 255.0);
        let tint_color = select(vec3f(1.0), vec3f(0.0), u.backdrop_params.z > 0.5);
        return mix(c, tint_color, tint);
    }
    let t = clamp(screen_px.y / max(u.canvas_size.y, 1.0), 0.0, 1.0);
    return mix(u.canvas_top.rgb, u.canvas_bottom.rgb, t);
}

/// 在一个屏幕位置取得已经完成通道、HDR 与 Alpha 合成的最终场景颜色。
fn scene_at(screen_px: vec2f, nearest: bool) -> vec3f {
    let bg = canvas_at(screen_px);
    let img_px = (screen_px - u.screen_offset) / u.scale;
    if (img_px.x < 0.0 || img_px.y < 0.0
        || img_px.x >= u.image_size.x || img_px.y >= u.image_size.y) {
        return bg;
    }
    let uv = img_px / u.image_size;
    var c = textureSampleLevel(t_image, s_linear, uv, 0.0);
    if (nearest) {
        c = textureSampleLevel(t_image, s_nearest, uv, 0.0);
    }

    // HDR：曝光 → Reinhard tonemap → gamma（保持现有显示顺序）
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

    // RGB 模式下含 alpha：先与棋盘格或画布合成，模糊采样因此不会产生透明暗边。
    if ((u.flags & 2u) != 0u && u.channel_mode == 0u && c.a < 1.0) {
        let cell = 8.0; // 物理像素
        let cx = floor(screen_px.x / cell);
        let cy = floor(screen_px.y / cell);
        let light = ((cx + cy) % 2.0) == 0.0;
        let checker = select(vec3f(0.78), vec3f(0.92), light);
        return mix(checker, c.rgb, c.a);
    }
    return mix(bg, c.rgb, c.a);
}

fn gaussian_weight(i: u32) -> f32 {
    switch i {
        case 0u, 4u: { return 1.0; }
        case 1u, 3u: { return 4.0; }
        default: { return 6.0; }
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    let nearest = (u.flags & 1u) != 0u;
    let sharp = scene_at(in.screen_px, nearest);
    let ga = glass_alpha_at(in.screen_px);
    if (ga <= 0.001) {
        return vec4f(sharp, 1.0);
    }

    // 5×5 二项式高斯核（外积 [1,4,6,4,1]²），25 次采样与旧实现成本相当，
    // 但权重对称、可预测，没有 Vogel 螺线的方向性和散点感。半径为 20pt × DPI。
    let radius = max(u.backdrop_params.w, 1.5);
    let step = radius * 0.5;
    var sum = vec3f(0.0);
    var wsum = 0.0;
    for (var y = 0u; y < 5u; y = y + 1u) {
        for (var x = 0u; x < 5u; x = x + 1u) {
            let weight = gaussian_weight(x) * gaussian_weight(y);
            let offset = vec2f(f32(x) - 2.0, f32(y) - 2.0) * step;
            sum += scene_at(in.screen_px + offset, false) * weight;
            wsum += weight;
        }
    }
    return vec4f(mix(sharp, sum / wsum, ga), 1.0);
}
