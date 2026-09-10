//! wgpu 渲染封装：图像纹理 + 通道查看/棋盘格/HDR tonemap shader。
//!
//! 架构（egui_wgpu 0.27 paint callback 模式）：
//! - `SharedGpu`（管线/顶点/ uniform buffer）创建一次，放入 callback_resources
//! - `CurrentBindGroup`（图像纹理 bind group）换图时替换
//! - 每帧通过 `new_paint_callback` 提交轻量指令，`paint` 阶段从
//!   callback_resources 取回资源，在 egui 的 render pass 内追加绘制
//! - paint 前回调的 viewport 已被 egui_wgpu 设为 callback rect

use std::sync::Arc;

use eframe::egui;
use eframe::egui_wgpu::wgpu;
use eframe::egui_wgpu::wgpu::util::DeviceExt;
use eframe::egui_wgpu::{CallbackResources, CallbackTrait, RenderState};
use iv_core::decode::PixelData;

const SHADER: &str = include_str!("image.wgsl");

/// 通道显示模式（对应 shader 中 channel_mode）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    Rgb = 0,
    R = 1,
    G = 2,
    B = 3,
    A = 4,
    /// RGB 忽略 alpha
    RgbOpaque = 5,
}

impl ChannelMode {
    #[allow(dead_code)] // 状态栏/信息面板后续使用
    pub fn label(self) -> &'static str {
        match self {
            ChannelMode::Rgb => "RGB",
            ChannelMode::R => "R",
            ChannelMode::G => "G",
            ChannelMode::B => "B",
            ChannelMode::A => "A",
            ChannelMode::RgbOpaque => "RGB (无 Alpha)",
        }
    }
}

/// uniform 布局（与 image.wgsl 的 Uniforms 一致，48 字节，含 padding）。
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// 画布物理像素尺寸
    pub canvas_size: [f32; 2],
    /// 当前显示 mip 的图像像素尺寸
    pub image_size: [f32; 2],
    /// 图像左上角在画布中的物理像素位置
    pub screen_offset: [f32; 2],
    /// 物理像素 / 图像像素
    pub scale: f32,
    pub channel_mode: u32,
    pub exposure: f32,
    /// bit0: nearest 采样；bit1: 显示棋盘格；bit2: HDR（曝光+tonemap+gamma）
    pub flags: u32,
    /// 对齐填充（uniform 结构需 16 字节对齐）
    pub _pad: [f32; 2],
}

/// 一次性创建的 GPU 静态资源。
pub struct SharedGpu {
    pipeline: wgpu::RenderPipeline,
    vertex_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
}

/// 当前图像的 bind group（换图时整体替换）。
pub struct CurrentBindGroup(pub Option<wgpu::BindGroup>);

/// 每帧的绘制指令（无状态，资源全部从 callback_resources 获取）。
struct ImagePaintCmd;

impl CallbackTrait for ImagePaintCmd {
    fn paint<'a>(
        &'a self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'a>,
        resources: &'a CallbackResources,
    ) {
        let Some(shared) = resources.get::<Arc<SharedGpu>>() else {
            return;
        };
        let Some(bind_group) = resources.get::<CurrentBindGroup>() else {
            return;
        };
        let Some(bind_group) = &bind_group.0 else {
            return;
        };
        // 此处 render_pass 的 viewport 已由 egui_wgpu 设为 callback rect
        render_pass.set_pipeline(&shared.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, shared.vertex_buf.slice(..));
        render_pass.draw(0..4, 0..1);
    }
}

/// 构造一帧的 paint callback（绘制到 `rect` 区域）。
pub fn new_paint_callback(rect: egui::Rect) -> egui::PaintCallback {
    eframe::egui_wgpu::Callback::new_paint_callback(rect, ImagePaintCmd)
}

/// 主线程持有的 GPU 资源管理器。
pub struct Renderer {
    rs: RenderState,
    shared: Arc<SharedGpu>,
    layout: wgpu::BindGroupLayout,
    sampler_linear: wgpu::Sampler,
    sampler_nearest: wgpu::Sampler,
    has_image: bool,
}

impl Renderer {
    pub fn new(rs: &RenderState) -> Self {
        let device = &*rs.device;
        let target_format = rs.target_format;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iv-image-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iv-image-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iv-image-pl"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iv-image-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // 4 顶点 triangle-strip（[0,1]² corner），覆盖整个 viewport
        let vertices: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("iv-vertex"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("iv-uniform"),
            contents: &[0u8; std::mem::size_of::<Uniforms>()],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iv-sampler-linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iv-sampler-nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let shared = Arc::new(SharedGpu {
            pipeline,
            vertex_buf,
            uniform_buf,
        });
        // 注册到 callback_resources 供 paint 阶段取用
        rs.renderer
            .write()
            .callback_resources
            .insert(shared.clone());
        rs.renderer
            .write()
            .callback_resources
            .insert(CurrentBindGroup(None));

        Self {
            rs: rs.clone(),
            shared,
            layout,
            sampler_linear,
            sampler_nearest,
            has_image: false,
        }
    }

    pub fn has_image(&self) -> bool {
        self.has_image
    }

    /// 上传一个 mip 的像素数据（替换旧纹理与 bind group）。
    pub fn upload_texture(&mut self, width: u32, height: u32, data: &PixelData) {
        let device = &*self.rs.device;
        let queue = &*self.rs.queue;

        let (format, bytes, bytes_per_pixel): (wgpu::TextureFormat, Vec<u8>, u32) = match data {
            PixelData::Rgba8(v) => (wgpu::TextureFormat::Rgba8Unorm, v.clone(), 4),
            PixelData::RgbaF32(v) => {
                // Rgba16Float 纹理：f32 → f16
                let mut out = Vec::with_capacity(v.len() * 2);
                for f in v {
                    out.extend_from_slice(&f32_to_f16(*f).to_le_bytes());
                }
                (wgpu::TextureFormat::Rgba16Float, out, 8)
            }
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iv-image-tex"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_pixel * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iv-image-bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shared.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_nearest),
                },
            ],
        });
        self.rs
            .renderer
            .write()
            .callback_resources
            .insert(CurrentBindGroup(Some(bind_group)));
        self.has_image = true;
    }

    /// 写入本帧 uniform。
    pub fn write_uniforms(&self, uniforms: &Uniforms) {
        self.rs
            .queue
            .write_buffer(&self.shared.uniform_buf, 0, bytemuck::bytes_of(uniforms));
    }
}

/// f32 → f16 位级转换。
fn f32_to_f16(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let frac = bits & 0x007F_FFFF;
    if exp == 255 {
        // Inf / NaN
        return sign | 0x7C00 | if frac != 0 { 0x0200 } else { 0 };
    }
    let new_exp = exp - 127 + 15;
    if new_exp >= 0x1F {
        return sign | 0x7C00; // 溢出 → Inf
    }
    if new_exp <= 0 {
        // 次正规/下溢
        if new_exp < -10 {
            return sign;
        }
        let frac = frac | 0x0080_0000;
        let shift = (14 - new_exp) as u32;
        let mut half_frac = frac >> (shift + 13);
        // 四舍五入
        let round_bit = (frac >> (shift + 12)) & 1;
        if round_bit == 1 {
            half_frac += 1;
        }
        return sign | half_frac as u16;
    }
    let mut half = sign | ((new_exp as u16) << 10) | ((frac >> 13) as u16);
    // 四舍五入到最近偶数
    let round_bit = (frac >> 12) & 1;
    let sticky = frac & 0xFFF;
    if round_bit == 1 && (sticky != 0 || (half & 1) == 1) {
        half = half.wrapping_add(1);
    }
    half
}
