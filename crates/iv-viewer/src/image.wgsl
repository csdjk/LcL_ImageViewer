//! 图像 shader：通道查看、HDR tonemap、棋盘格、线性/最近邻采样切换，
//! 以及玻璃拟态背景模糊（像素落在玻璃区域内时对图像做泊松盘模糊，
//! egui 再叠上半透明面板即成磨砂玻璃）。所有显示逻辑在 GPU 侧，切换零开销。

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
    glass_rects: array<vec4f, 6>,   // min.xy, max.xy（画布物理像素）
    glass_alpha: array<vec4f, 2>,   // 每区域模糊混合系数（打包 2×vec4）
    glass_corner: array<vec4f, 2>,  // 每区域圆角半径（画布物理像素，打包 2×vec4）
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    // 图像范围外：discard，透出 egui 背景
    if (in.img_px.x < 0.0 || in.img_px.y < 0.0
        || in.img_px.x >= u.image_size.x || in.img_px.y >= u.image_size.y) {
        discard;
    }
    let uv = in.img_px / u.image_size;
    let ga = glass_alpha_at(in.screen_px);

    // 全部用 textureSampleLevel（显式 LOD）：玻璃判断使控制流按像素分叉，
    // 隐式导数的 textureSample 在非均匀控制流中是未定义行为
    var c: vec4f;
    if (ga > 0.001) {
        // 磨砂玻璃：Vogel 螺线（黄金角）高斯加权 24 采样 + 中心采样。
        // 均匀均值的碟式模糊会在亮点周围留下硬边光斑/环状伪影；
        // 高斯权重（中心密、边缘疏）让磨砂如真实高斯般干净细腻。
        // 注：naga 的模块级常量数组只允许常量索引，故采样盘改为程序化生成
        let radius = max(18.0 / u.scale, 1.5); // 屏幕 18px 模糊半径 → 图像像素
        let sigma = radius * 0.5;
        let inv_2s2 = 1.0 / (2.0 * sigma * sigma);
        var sum = textureSampleLevel(t_image, s_linear, uv, 0.0);
        var wsum = 1.0;
        for (var k = 0u; k < 24u; k = k + 1u) {
            let fi = f32(k) + 0.5;
            let rr = sqrt(fi / 24.0) * radius;
            let th = fi * 2.39996; // 黄金角
            let w = exp(-(rr * rr) * inv_2s2);
            let tap = (in.img_px + vec2f(cos(th), sin(th)) * rr) / u.image_size;
            sum += textureSampleLevel(t_image, s_linear, tap, 0.0) * w;
            wsum += w;
        }
        let blurred = sum / wsum;
        var sharp: vec4f;
        if ((u.flags & 1u) != 0u) {
            sharp = textureSampleLevel(t_image, s_nearest, uv, 0.0);
        } else {
            sharp = textureSampleLevel(t_image, s_linear, uv, 0.0);
        }
        c = mix(sharp, blurred, ga);
    } else if ((u.flags & 1u) != 0u) {
        c = textureSampleLevel(t_image, s_nearest, uv, 0.0);
    } else {
        c = textureSampleLevel(t_image, s_linear, uv, 0.0);
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
