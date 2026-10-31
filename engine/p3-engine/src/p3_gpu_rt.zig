// =============================================================================
// P³ GPU RT — РЕАЛЬНЫЙ GPU PIPELINE (ZGPU / DAWN / WEBGPU)
// =============================================================================
//
// Это СЕРДЦЕВА Фазы 6. Шейдеры WGSL из p3_gpu.zig больше НЕ строки —
// они компилируются в GPU pipeline и ИСПОЛНЯЮТСЯ.
//
// Архитектура:
//
//   p3_gpu.zig (comptime)  →  WGSL strings
//       ↓
//   p3_gpu_rt.zig (runtime) →  zgpu.GraphicsContext
//       ↓                        .createShaderModule(wgsl)
//       ↓                        .createComputePipeline()
//       ↓                        .createRenderPipeline()
//       ↓                        .createBuffer()
//       ↓
//   dispatch / render  →  GPU EXECUTION
//
// Донор: zig-gamedev/zgpu — handle-based resource pool поверх Dawn
//   (https://github.com/zig-gamedev/zgpu)
//
// КЛЮЧЕВАЯ ИДЕЯ: мы НЕ переписываем шейдеры. Мы КОРМИМ существующие
// WGSL строки из p3_gpu.zig в zgpu.createShaderModule().
// Шейдеры уже правильные — им нужен только реальный GPU device.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const zgpu = @import("zgpu");
const wgpu = zgpu.wgpu;
const p3_gpu = @import("p3_gpu.zig");
const p3_kernel = @import("p3_kernel.zig");

pub const HomVec4 = p3_kernel.HomVec4;
pub const PGL4 = p3_kernel.PGL4;
pub const GpuHomVec4 = p3_gpu.GpuHomVec4;
pub const GpuPGL4 = p3_gpu.GpuPGL4;
pub const GpuGeodesicState = p3_gpu.GpuGeodesicState;
pub const GpuRK4Params = p3_gpu.GpuRK4Params;

// =============================================================================
// 1. WGSL МОДУЛИ ДЛЯ РЕАЛЬНОГО GPU
// =============================================================================
//
// Каждый compute shader нуждается в своих struct definitions.
// Мы собираем WGSL модуль = structs + shader_code.

/// WGSL для FS-distance compute: HomVec4 struct + shader
pub fn wgslFsDistanceModule() [:0]const u8 {
    return p3_gpu.wgsl_hom_vec4 ++ "\n" ++ p3_gpu.wgsl_fs_distance_batch;
}

/// WGSL для PGL-action compute: HomVec4 + PGL4 structs + shader
pub fn wgslPglActionModule() [:0]const u8 {
    return p3_gpu.wgsl_hom_vec4 ++ "\n" ++ p3_gpu.wgsl_pgl4 ++ "\n" ++ p3_gpu.wgsl_pgl_action_batch;
}

/// WGSL для Geodesic RK4 compute: GeodesicState struct + shader
pub fn wgslGeodesicRK4Module() [:0]const u8 {
    return p3_gpu.wgsl_geodesic_state ++ "\n" ++ p3_gpu.wgsl_geodesic_rk4_step;
}

/// WGSL для Idempotent Newton compute: shader only (raw array<f32>)
pub fn wgslIdempotentNewtonModule() [:0]const u8 {
    return p3_gpu.wgsl_idempotent_newton;
}

/// WGSL для Dehomogenize vertex shader: HomVec4 + MVP struct + shader
pub fn wgslDehomogenizeModule() [:0]const u8 {
    return p3_gpu.wgsl_hom_vec4 ++ "\n" ++ p3_gpu.wgsl_dehomogenize_vertex;
}

/// WGSL fragment shader: simple P³ color output
pub const wgsl_p3_fragment =
    \\@fragment fn p3_fragment(
    \\    @location(0) world_position: vec3<f32>,
    \\    @location(1) world_normal: vec3<f32>,
    \\    @location(2) uv: vec2<f32>,
    \\    @location(3) card_index: f32,
    \\) -> @location(0) vec4<f32> {
    \\    // FS-depth coloring: normalize position to [0,1] for color
    \\    let n = normalize(world_normal);
    \\    let base_color = vec3<f32>(
    \\        0.5 + 0.5 * n.x,
    \\        0.5 + 0.5 * n.y,
    \\        0.5 + 0.5 * n.z,
    \\    );
    \\    // Card index as subtle tint (visualize affine chart switching)
    \\    let card_tint = vec3<f32>(
    \\        1.0 - 0.1 * card_index,
    \\        1.0 - 0.05 * card_index,
    \\        1.0,
    \\    );
    \\    return vec4<f32>(base_color * card_tint, 1.0);
    \\}
;

// =============================================================================
// 2. P³ GPU CONTEXT — ВСЕ РЕСУРСЫ GPU
// =============================================================================
//
// Один struct владеет ВСЕМ: контекст, pipelines, buffers, bind groups.
// Нет singleton. Нет global state. Явный ownership.

pub const P3GpuContext = struct {
    gctx: *zgpu.GraphicsContext,
    allocator: std.mem.Allocator,

    // --- Shader modules (raw wgpu objects, not handles) ---
    fs_distance_sm: wgpu.ShaderModule,
    pgl_action_sm: wgpu.ShaderModule,
    geodesic_rk4_sm: wgpu.ShaderModule,
    idempotent_newton_sm: wgpu.ShaderModule,
    dehom_vertex_sm: wgpu.ShaderModule,
    p3_fragment_sm: wgpu.ShaderModule,

    // --- Compute pipelines (handle-based) ---
    fs_distance_pl: zgpu.PipelineLayoutHandle,
    fs_distance_bgl: zgpu.BindGroupLayoutHandle,
    fs_distance_pipeline: zgpu.ComputePipelineHandle,

    pgl_action_pl: zgpu.PipelineLayoutHandle,
    pgl_action_bgl: zgpu.BindGroupLayoutHandle,
    pgl_action_pipeline: zgpu.ComputePipelineHandle,

    geodesic_rk4_pl: zgpu.PipelineLayoutHandle,
    geodesic_rk4_bgl: zgpu.BindGroupLayoutHandle,
    geodesic_rk4_pipeline: zgpu.ComputePipelineHandle,

    idempotent_newton_pl: zgpu.PipelineLayoutHandle,
    idempotent_newton_bgl: zgpu.BindGroupLayoutHandle,
    idempotent_newton_pipeline: zgpu.ComputePipelineHandle,

    // --- Render pipeline ---
    render_pl: zgpu.PipelineLayoutHandle,
    render_bgl: zgpu.BindGroupLayoutHandle,
    render_pipeline: zgpu.RenderPipelineHandle,
    depth_texture: zgpu.TextureHandle,
    depth_texture_view: zgpu.TextureViewHandle,

    // --- GPU Buffers ---
    positions_buffer: zgpu.BufferHandle,
    distances_buffer: zgpu.BufferHandle,
    reference_buffer: zgpu.BufferHandle,
    transforms_buffer: zgpu.BufferHandle,
    transform_result_buffer: zgpu.BufferHandle,
    geodesic_in_buffer: zgpu.BufferHandle,
    geodesic_out_buffer: zgpu.BufferHandle,
    geodesic_params_buffer: zgpu.BufferHandle,
    mvp_buffer: zgpu.BufferHandle,
    vertex_buffer: zgpu.BufferHandle,

    // --- Bind groups ---
    fs_distance_bg: zgpu.BindGroupHandle,
    pgl_action_bg: zgpu.BindGroupHandle,
    geodesic_rk4_bg: zgpu.BindGroupHandle,
    idempotent_newton_bg: zgpu.BindGroupHandle,

    // --- Config ---
    max_points: u32,
    max_transforms: u32,

    // =====================================================================
    // INIT — СОЗДАЁМ ВСЕ GPU РЕСУРСЫ
    // =====================================================================

    pub fn init(
        allocator: std.mem.Allocator,
        gctx: *zgpu.GraphicsContext,
        max_points: u32,
        max_transforms: u32,
    ) !*P3GpuContext {
        const device = gctx.device;

        // --- 1. COMPILE SHADER MODULES ---
        // WGSL strings → GPU shader modules. Вот где comptime-строки
        // из p3_gpu.zig становятся РЕАЛЬНЫМИ GPU шейдерами.
        const fs_distance_sm = zgpu.createWgslShaderModule(
            device,
            wgslFsDistanceModule(),
            "p3_fs_distance",
        );
        const pgl_action_sm = zgpu.createWgslShaderModule(
            device,
            wgslPglActionModule(),
            "p3_pgl_action",
        );
        const geodesic_rk4_sm = zgpu.createWgslShaderModule(
            device,
            wgslGeodesicRK4Module(),
            "p3_geodesic_rk4",
        );
        const idempotent_newton_sm = zgpu.createWgslShaderModule(
            device,
            wgslIdempotentNewtonModule(),
            "p3_idempotent_newton",
        );
        const dehom_vertex_sm = zgpu.createWgslShaderModule(
            device,
            wgslDehomogenizeModule(),
            "p3_dehomogenize",
        );
        const p3_fragment_sm = zgpu.createWgslShaderModule(
            device,
            wgsl_p3_fragment,
            "p3_fragment",
        );

        // --- 2. CREATE COMPUTE PIPELINES ---
        // FS-distance: bindings = (positions: storage, distances: storage_rw, reference: uniform)
        const fs_distance_bgl = gctx.createBindGroupLayout(&.{
            zgpu.bufferEntry(0, .{ .compute = true }, .storage, false, 0),
            zgpu.bufferEntry(1, .{ .compute = true }, .storage, false, 0),
            zgpu.bufferEntry(2, .{ .compute = true }, .uniform, false, 0),
        });
        const fs_distance_pl = gctx.createPipelineLayout(&.{fs_distance_bgl});
        const fs_distance_pipeline = gctx.createComputePipeline(
            fs_distance_pl,
            wgpu.ComputePipelineDescriptor{
                .compute = .{
                    .module = fs_distance_sm,
                    .entry_point = "fs_distance_batch",
                },
            },
        );

        // PGL-action: bindings = (input: storage, output: storage_rw, transform: uniform)
        const pgl_action_bgl = gctx.createBindGroupLayout(&.{
            zgpu.bufferEntry(0, .{ .compute = true }, .storage, false, 0),
            zgpu.bufferEntry(1, .{ .compute = true }, .storage, false, 0),
            zgpu.bufferEntry(2, .{ .compute = true }, .uniform, false, 0),
        });
        const pgl_action_pl = gctx.createPipelineLayout(&.{pgl_action_bgl});
        const pgl_action_pipeline = gctx.createComputePipeline(
            pgl_action_pl,
            wgpu.ComputePipelineDescriptor{
                .compute = .{
                    .module = pgl_action_sm,
                    .entry_point = "pgl_action_batch",
                },
            },
        );

        // Geodesic RK4: bindings = (states_in: storage, states_out: storage_rw, params: uniform)
        const geodesic_rk4_bgl = gctx.createBindGroupLayout(&.{
            zgpu.bufferEntry(0, .{ .compute = true }, .storage, false, 0),
            zgpu.bufferEntry(1, .{ .compute = true }, .storage, false, 0),
            zgpu.bufferEntry(2, .{ .compute = true }, .uniform, false, 0),
        });
        const geodesic_rk4_pl = gctx.createPipelineLayout(&.{geodesic_rk4_bgl});
        const geodesic_rk4_pipeline = gctx.createComputePipeline(
            geodesic_rk4_pl,
            wgpu.ComputePipelineDescriptor{
                .compute = .{
                    .module = geodesic_rk4_sm,
                    .entry_point = "geodesic_rk4_step",
                },
            },
        );

        // Idempotent Newton: bindings = (matrices: storage_rw)
        const idempotent_newton_bgl = gctx.createBindGroupLayout(&.{
            zgpu.bufferEntry(0, .{ .compute = true }, .storage, false, 0),
        });
        const idempotent_newton_pl = gctx.createPipelineLayout(&.{idempotent_newton_bgl});
        const idempotent_newton_pipeline = gctx.createComputePipeline(
            idempotent_newton_pl,
            wgpu.ComputePipelineDescriptor{
                .compute = .{
                    .module = idempotent_newton_sm,
                    .entry_point = "idempotent_newton_step",
                },
            },
        );

        // --- 3. CREATE RENDER PIPELINE ---
        // Dehomogenize vertex + P³ fragment
        const render_bgl = gctx.createBindGroupLayout(&.{
            zgpu.bufferEntry(0, .{ .vertex = true, .fragment = true }, .uniform, true, 0),
        });
        const render_pl = gctx.createPipelineLayout(&.{render_bgl});

        // Vertex attributes: HomVec4 position + HomVec4 normal + vec2 uv
        const vertex_attributes = [_]wgpu.VertexAttribute{
            .{ .format = .float32x4, .offset = 0, .shader_location = 0 }, // position
            .{ .format = .float32x4, .offset = 16, .shader_location = 1 }, // normal
            .{ .format = .float32x2, .offset = 32, .shader_location = 2 }, // uv
        };
        const vertex_buffers = [_]wgpu.VertexBufferLayout{.{
            .array_stride = 40, // 4+4+2 f32 = 40 bytes (P³ vertex)
            .attribute_count = vertex_attributes.len,
            .attributes = &vertex_attributes,
        }};

        const color_targets = [_]wgpu.ColorTargetState{.{
            .format = zgpu.GraphicsContext.swapchain_format,
        }};

        const render_pipeline = gctx.createRenderPipeline(
            render_pl,
            wgpu.RenderPipelineDescriptor{
                .vertex = .{
                    .module = dehom_vertex_sm,
                    .entry_point = "dehomogenize_vertex",
                    .buffer_count = vertex_buffers.len,
                    .buffers = &vertex_buffers,
                },
                .primitive = .{
                    .front_face = .ccw,
                    .cull_mode = .none,
                    .topology = .triangle_list,
                },
                .depth_stencil = &.{
                    .format = .depth32_float,
                    .depth_write_enabled = true,
                    .depth_compare = .less,
                },
                .fragment = &.{
                    .module = p3_fragment_sm,
                    .entry_point = "p3_fragment",
                    .target_count = color_targets.len,
                    .targets = &color_targets,
                },
            },
        );

        // --- 4. CREATE GPU BUFFERS ---
        const positions_buffer = gctx.createBuffer(.{
            .usage = .{ .storage = true, .copy_dst = true },
            .size = max_points * @sizeOf(GpuHomVec4),
        });
        const distances_buffer = gctx.createBuffer(.{
            .usage = .{ .storage = true, .copy_src = true },
            .size = max_points * @sizeOf(f32),
        });
        const reference_buffer = gctx.createBuffer(.{
            .usage = .{ .uniform = true, .copy_dst = true },
            .size = @sizeOf(GpuHomVec4),
        });
        const transforms_buffer = gctx.createBuffer(.{
            .usage = .{ .storage = true, .copy_dst = true },
            .size = max_transforms * @sizeOf(GpuPGL4),
        });
        const transform_result_buffer = gctx.createBuffer(.{
            .usage = .{ .storage = true, .copy_src = true },
            .size = max_points * @sizeOf(GpuHomVec4),
        });
        const geodesic_in_buffer = gctx.createBuffer(.{
            .usage = .{ .storage = true, .copy_dst = true },
            .size = max_points * @sizeOf(GpuGeodesicState),
        });
        const geodesic_out_buffer = gctx.createBuffer(.{
            .usage = .{ .storage = true, .copy_src = true },
            .size = max_points * @sizeOf(GpuGeodesicState),
        });
        const geodesic_params_buffer = gctx.createBuffer(.{
            .usage = .{ .uniform = true, .copy_dst = true },
            .size = @sizeOf(GpuRK4Params),
        });
        const mvp_buffer = gctx.createBuffer(.{
            .usage = .{ .uniform = true, .copy_dst = true },
            .size = @sizeOf(GpuPGL4),
        });
        const vertex_buffer = gctx.createBuffer(.{
            .usage = .{ .vertex = true, .copy_dst = true },
            .size = max_points * 40, // 40 bytes per P³ vertex
        });

        // --- 5. CREATE BIND GROUPS ---
        const fs_distance_bg = gctx.createBindGroup(fs_distance_bgl, &.{
            .{ .binding = 0, .buffer_handle = positions_buffer, .offset = 0, .size = max_points * @sizeOf(GpuHomVec4) },
            .{ .binding = 1, .buffer_handle = distances_buffer, .offset = 0, .size = max_points * @sizeOf(f32) },
            .{ .binding = 2, .buffer_handle = reference_buffer, .offset = 0, .size = @sizeOf(GpuHomVec4) },
        });
        const pgl_action_bg = gctx.createBindGroup(pgl_action_bgl, &.{
            .{ .binding = 0, .buffer_handle = positions_buffer, .offset = 0, .size = max_points * @sizeOf(GpuHomVec4) },
            .{ .binding = 1, .buffer_handle = transform_result_buffer, .offset = 0, .size = max_points * @sizeOf(GpuHomVec4) },
            .{ .binding = 2, .buffer_handle = transforms_buffer, .offset = 0, .size = @sizeOf(GpuPGL4) },
        });
        const geodesic_rk4_bg = gctx.createBindGroup(geodesic_rk4_bgl, &.{
            .{ .binding = 0, .buffer_handle = geodesic_in_buffer, .offset = 0, .size = max_points * @sizeOf(GpuGeodesicState) },
            .{ .binding = 1, .buffer_handle = geodesic_out_buffer, .offset = 0, .size = max_points * @sizeOf(GpuGeodesicState) },
            .{ .binding = 2, .buffer_handle = geodesic_params_buffer, .offset = 0, .size = @sizeOf(GpuRK4Params) },
        });
        const idempotent_newton_bg = gctx.createBindGroup(idempotent_newton_bgl, &.{
            .{ .binding = 0, .buffer_handle = transforms_buffer, .offset = 0, .size = max_transforms * @sizeOf(GpuPGL4) },
        });

        // --- 6. DEPTH TEXTURE ---
        const depth_texture = gctx.createTexture(.{
            .usage = .{ .render_attachment = true },
            .dimension = .tdim_2d,
            .format = .depth32_float,
            .size = .{
                .width = gctx.swapchain_descriptor.width,
                .height = gctx.swapchain_descriptor.height,
                .depth_or_array_layers = 1,
            },
        });
        const depth_texture_view = gctx.createTextureView(depth_texture, .{
            .format = .depth32_float,
            .dimension = .tvdim_2d,
        });

        const self = try allocator.create(P3GpuContext);
        self.* = .{
            .gctx = gctx,
            .allocator = allocator,
            // Shader modules
            .fs_distance_sm = fs_distance_sm,
            .pgl_action_sm = pgl_action_sm,
            .geodesic_rk4_sm = geodesic_rk4_sm,
            .idempotent_newton_sm = idempotent_newton_sm,
            .dehom_vertex_sm = dehom_vertex_sm,
            .p3_fragment_sm = p3_fragment_sm,
            // FS-distance
            .fs_distance_pl = fs_distance_pl,
            .fs_distance_bgl = fs_distance_bgl,
            .fs_distance_pipeline = fs_distance_pipeline,
            // PGL-action
            .pgl_action_pl = pgl_action_pl,
            .pgl_action_bgl = pgl_action_bgl,
            .pgl_action_pipeline = pgl_action_pipeline,
            // Geodesic RK4
            .geodesic_rk4_pl = geodesic_rk4_pl,
            .geodesic_rk4_bgl = geodesic_rk4_bgl,
            .geodesic_rk4_pipeline = geodesic_rk4_pipeline,
            // Idempotent Newton
            .idempotent_newton_pl = idempotent_newton_pl,
            .idempotent_newton_bgl = idempotent_newton_bgl,
            .idempotent_newton_pipeline = idempotent_newton_pipeline,
            // Render
            .render_pl = render_pl,
            .render_bgl = render_bgl,
            .render_pipeline = render_pipeline,
            .depth_texture = depth_texture,
            .depth_texture_view = depth_texture_view,
            // Buffers
            .positions_buffer = positions_buffer,
            .distances_buffer = distances_buffer,
            .reference_buffer = reference_buffer,
            .transforms_buffer = transforms_buffer,
            .transform_result_buffer = transform_result_buffer,
            .geodesic_in_buffer = geodesic_in_buffer,
            .geodesic_out_buffer = geodesic_out_buffer,
            .geodesic_params_buffer = geodesic_params_buffer,
            .mvp_buffer = mvp_buffer,
            .vertex_buffer = vertex_buffer,
            // Bind groups
            .fs_distance_bg = fs_distance_bg,
            .pgl_action_bg = pgl_action_bg,
            .geodesic_rk4_bg = geodesic_rk4_bg,
            .idempotent_newton_bg = idempotent_newton_bg,
            // Config
            .max_points = max_points,
            .max_transforms = max_transforms,
        };
        return self;
    }

    // =====================================================================
    // DEINIT — ОСВОБОЖДАЕМ ВСЕ РЕСУРСЫ
    // =====================================================================

    pub fn deinit(self: *P3GpuContext, allocator: std.mem.Allocator) void {
        // Release shader modules (raw wgpu objects)
        self.fs_distance_sm.release();
        self.pgl_action_sm.release();
        self.geodesic_rk4_sm.release();
        self.idempotent_newton_sm.release();
        self.dehom_vertex_sm.release();
        self.p3_fragment_sm.release();

        // Release GPU context (destroys all handle-based resources)
        self.gctx.destroy(allocator);
        allocator.destroy(self);
    }

    // =====================================================================
    // UPLOAD — CPU → GPU
    // =====================================================================

    /// Загрузить HomVec4 точки на GPU
    pub fn uploadPositions(self: *P3GpuContext, points: []const GpuHomVec4) void {
        const n = @min(points.len, self.max_points);
        const buf = self.gctx.lookupResource(self.positions_buffer) orelse return;
        self.gctx.queue.writeBuffer(buf, 0, GpuHomVec4, points[0..n]);
    }

    /// Загрузить reference point для FS-distance
    pub fn uploadReference(self: *P3GpuContext, ref_point: *const GpuHomVec4) void {
        const buf = self.gctx.lookupResource(self.reference_buffer) orelse return;
        self.gctx.queue.writeBuffer(buf, 0, GpuHomVec4, ref_point[0..1]);
    }

    /// Загрузить PGL4 transforms на GPU
    pub fn uploadTransforms(self: *P3GpuContext, transforms: []const GpuPGL4) void {
        const n = @min(transforms.len, self.max_transforms);
        const buf = self.gctx.lookupResource(self.transforms_buffer) orelse return;
        self.gctx.queue.writeBuffer(buf, 0, GpuPGL4, transforms[0..n]);
    }

    /// Загрузить geodesic states на GPU
    pub fn uploadGeodesicStates(self: *P3GpuContext, states: []const GpuGeodesicState) void {
        const n = @min(states.len, self.max_points);
        const buf = self.gctx.lookupResource(self.geodesic_in_buffer) orelse return;
        self.gctx.queue.writeBuffer(buf, 0, GpuGeodesicState, states[0..n]);
    }

    /// Загрузить RK4 params на GPU
    pub fn uploadRK4Params(self: *P3GpuContext, params: *const GpuRK4Params) void {
        const buf = self.gctx.lookupResource(self.geodesic_params_buffer) orelse return;
        self.gctx.queue.writeBuffer(buf, 0, GpuRK4Params, params[0..1]);
    }

    /// Загрузить MVP matrix на GPU
    pub fn uploadMVP(self: *P3GpuContext, mvp: *const GpuPGL4) void {
        const buf = self.gctx.lookupResource(self.mvp_buffer) orelse return;
        self.gctx.queue.writeBuffer(buf, 0, GpuPGL4, mvp[0..1]);
    }

    // =====================================================================
    // DISPATCH — GPU COMPUTE EXECUTION
    // =====================================================================

    /// Запустить FS-distance compute shader
    /// dispatches ceil(n / WORKGROUP_SIZE) workgroups
    pub fn dispatchFSDistance(self: *P3GpuContext, n_points: u32) void {
        const encoder = self.gctx.device.createCommandEncoder(null);
        defer encoder.release();

        const pass = encoder.beginComputePass(null);
        defer {
            pass.end();
            pass.release();
        }

        const pipeline = self.gctx.lookupResource(self.fs_distance_pipeline) orelse return;
        const bind_group = self.gctx.lookupResource(self.fs_distance_bg) orelse return;

        pass.setPipeline(pipeline);
        pass.setBindGroup(0, bind_group, &.{});
        const wg_count = (n_points + p3_gpu.WORKGROUP_SIZE - 1) / p3_gpu.WORKGROUP_SIZE;
        pass.dispatchWorkgroups(wg_count, 1, 1);

        const cmd = encoder.finish(null);
        defer cmd.release();
        self.gctx.submit(&.{cmd});
    }

    /// Запустить PGL-action compute shader
    pub fn dispatchPGLAction(self: *P3GpuContext, n_points: u32) void {
        const encoder = self.gctx.device.createCommandEncoder(null);
        defer encoder.release();

        const pass = encoder.beginComputePass(null);
        defer {
            pass.end();
            pass.release();
        }

        const pipeline = self.gctx.lookupResource(self.pgl_action_pipeline) orelse return;
        const bind_group = self.gctx.lookupResource(self.pgl_action_bg) orelse return;

        pass.setPipeline(pipeline);
        pass.setBindGroup(0, bind_group, &.{});
        const wg_count = (n_points + p3_gpu.WORKGROUP_SIZE - 1) / p3_gpu.WORKGROUP_SIZE;
        pass.dispatchWorkgroups(wg_count, 1, 1);

        const cmd = encoder.finish(null);
        defer cmd.release();
        self.gctx.submit(&.{cmd});
    }

    /// Запустить Geodesic RK4 compute shader
    pub fn dispatchGeodesicRK4(self: *P3GpuContext, n_states: u32) void {
        const encoder = self.gctx.device.createCommandEncoder(null);
        defer encoder.release();

        const pass = encoder.beginComputePass(null);
        defer {
            pass.end();
            pass.release();
        }

        const pipeline = self.gctx.lookupResource(self.geodesic_rk4_pipeline) orelse return;
        const bind_group = self.gctx.lookupResource(self.geodesic_rk4_bg) orelse return;

        pass.setPipeline(pipeline);
        pass.setBindGroup(0, bind_group, &.{});
        const wg_count = (n_states + p3_gpu.WORKGROUP_SIZE - 1) / p3_gpu.WORKGROUP_SIZE;
        pass.dispatchWorkgroups(wg_count, 1, 1);

        const cmd = encoder.finish(null);
        defer cmd.release();
        self.gctx.submit(&.{cmd});
    }

    /// Запустить Idempotent Newton compute shader
    /// Workgroup size = 16 (16 matrices per workgroup)
    pub fn dispatchIdempotentNewton(self: *P3GpuContext, n_matrices: u32) void {
        const encoder = self.gctx.device.createCommandEncoder(null);
        defer encoder.release();

        const pass = encoder.beginComputePass(null);
        defer {
            pass.end();
            pass.release();
        }

        const pipeline = self.gctx.lookupResource(self.idempotent_newton_pipeline) orelse return;
        const bind_group = self.gctx.lookupResource(self.idempotent_newton_bg) orelse return;

        pass.setPipeline(pipeline);
        pass.setBindGroup(0, bind_group, &.{});
        const wg_count = (n_matrices + 15) / 16; // workgroup_size = 16
        pass.dispatchWorkgroups(wg_count, 1, 1);

        const cmd = encoder.finish(null);
        defer cmd.release();
        self.gctx.submit(&.{cmd});
    }

    // =====================================================================
    // RENDER — GPU RENDER EXECUTION
    // =====================================================================

    /// Отрендерить кадр с P³ dehomogenize pipeline
    pub fn renderFrame(self: *P3GpuContext, n_vertices: u32, n_instances: u32) void {
        const back_buffer_view = self.gctx.swapchain.getCurrentTextureView();
        defer back_buffer_view.release();

        const encoder = self.gctx.device.createCommandEncoder(null);
        defer encoder.release();

        const pipeline = self.gctx.lookupResource(self.render_pipeline) orelse return;
        const depth_view = self.gctx.lookupResource(self.depth_texture_view) orelse return;
        const vb_info = self.gctx.lookupResourceInfo(self.vertex_buffer) orelse return;

        const color_attachments = [_]wgpu.RenderPassColorAttachment{.{
            .view = back_buffer_view,
            .load_op = .clear,
            .store_op = .store,
            .clear_value = .{ .r = 0.02, .g = 0.02, .b = 0.04, .a = 1.0 }, // P³ dark blue
        }};
        const depth_attachment = wgpu.RenderPassDepthStencilAttachment{
            .view = depth_view,
            .depth_load_op = .clear,
            .depth_store_op = .store,
            .depth_clear_value = 1.0,
            .stencil_load_op = .undef,
            .stencil_store_op = .undef,
        };
        const pass = encoder.beginRenderPass(wgpu.RenderPassDescriptor{
            .color_attachment_count = color_attachments.len,
            .color_attachments = &color_attachments,
            .depth_stencil_attachment = &depth_attachment,
        });
        defer {
            pass.end();
            pass.release();
        }

        pass.setPipeline(pipeline);
        pass.setVertexBuffer(0, vb_info.gpuobj.?, 0, vb_info.size);

        // Set MVP bind group
        const bind_group = self.gctx.lookupResource(
            self.gctx.createBindGroup(self.render_bgl, &.{
                .{ .binding = 0, .buffer_handle = self.mvp_buffer, .offset = 0, .size = @sizeOf(GpuPGL4) },
            }),
        ) orelse return;
        pass.setBindGroup(0, bind_group, &.{});

        pass.draw(n_vertices, n_instances, 0, 0);

        const cmd = encoder.finish(null);
        defer cmd.release();
        self.gctx.submit(&.{cmd});
    }

    /// Present swapchain
    pub fn present(self: *P3GpuContext) @TypeOf(self.gctx.present()) {
        return self.gctx.present();
    }

    /// Recreate depth texture on resize
    pub fn recreateDepthTexture(self: *P3GpuContext) void {
        const width = self.gctx.swapchain_descriptor.width;
        const height = self.gctx.swapchain_descriptor.height;

        // Release old
        self.gctx.releaseResource(self.depth_texture_view);
        self.gctx.releaseResource(self.depth_texture);

        // Create new
        self.depth_texture = self.gctx.createTexture(.{
            .usage = .{ .render_attachment = true },
            .dimension = .tdim_2d,
            .format = .depth32_float,
            .size = .{
                .width = width,
                .height = height,
                .depth_or_array_layers = 1,
            },
        });
        self.depth_texture_view = self.gctx.createTextureView(self.depth_texture, .{
            .format = .depth32_float,
            .dimension = .tvdim_2d,
        });
    }

    // =====================================================================
    // READBACK — GPU → CPU
    // =====================================================================

    /// Прочитать FS-distances обратно (для verification/demo)
    /// Использует staging buffer + mapAsync
    pub fn readbackDistances(
        self: *P3GpuContext,
        n_points: u32,
        output: []f32,
    ) void {
        _ = self;
        _ = n_points;
        _ = output;
        // NOTE: Full readback requires staging buffer + mapAsync.
        // This is a placeholder — full implementation follows the
        // zgpu.readback pattern from zig-gamedev samples.
        // For now, distances are computed on GPU and verified
        // by running the CPU equivalent (p3_gpu.cpuFSDistanceBatch).
    }
};

// =============================================================================
// 3. P³ DEMO VERTICES — ТОЧКИ НА S³ ДЛЯ ДЕМО
// =============================================================================
//
// Генерируем вершины на S³ (единичная сфера в R⁴) для первого
// запускаемого демо. Это НЕ финальный mesh — это proof-of-concept.

/// P³ vertex: position (HomVec4 f32) + normal (HomVec4 f32) + uv (vec2 f32) = 40 bytes
pub const P3Vertex = extern struct {
    px: f32, py: f32, pz: f32, pw: f32, // position in P³
    nx: f32, ny: f32, nz: f32, nw: f32, // normal in P³
    u: f32, v: f32, // texture coordinates
};

/// Генерировать N точек на S³ (fibonacci sphere в R⁴)
pub fn generateS3Points(allocator: std.mem.Allocator, n: u32) ![]P3Vertex {
    var vertices = try allocator.alloc(P3Vertex, n);

    const golden_ratio: f64 = (1.0 + @sqrt(5.0)) / 2.0;

    for (0..n) |i| {
        const t: f64 = @as(f64, @floatFromInt(i)) / @as(f64, @floatFromInt(n));
        const inclination: f64 = std.math.acos(1.0 - 2.0 * t);
        const azimuth: f64 = 2.0 * std.math.pi * golden_ratio * @as(f64, @floatFromInt(i));

        // S³ parametrization: (sin θ cos φ, sin θ sin φ, cos θ, 0) + w component
        // For S³ in R⁴, we use hyperspherical coordinates
        const psi: f64 = 2.0 * std.math.pi * t * golden_ratio;
        const sin_i = @sin(inclination);
        const cos_i = @cos(inclination);
        const sin_a = @sin(azimuth);
        const cos_a = @cos(azimuth);
        const sin_p = @sin(psi);
        const cos_p = @cos(psi);

        // Hyperspherical: (sin θ sin ψ cos φ, sin θ sin ψ sin φ, sin θ cos ψ, cos θ)
        const x = sin_i * sin_p * cos_a;
        const y = sin_i * sin_p * sin_a;
        const z = sin_i * cos_p;
        const w = cos_i;

        // Normal = position on S³ (outward normal = radial)
        vertices[i] = .{
            .px = @floatCast(x),
            .py = @floatCast(y),
            .pz = @floatCast(z),
            .pw = @floatCast(w),
            .nx = @floatCast(x),
            .ny = @floatCast(y),
            .nz = @floatCast(z),
            .nw = @floatCast(w),
            .u = @floatCast(t),
            .v = @floatCast(@as(f64, @floatFromInt(i % 2))),
        };
    }

    return vertices;
}

/// Генерировать куб на S³ (8 вершин, 12 треугольников)
pub fn generateS3Cube(allocator: std.mem.Allocator) !struct { vertices: []P3Vertex, indices: []u32 } {
    // 8 corners of a cube, homogenized and normalized to S³
    const corners = [8][3]f64{
        .{ -1, -1, -1 }, .{ 1, -1, -1 }, .{ 1, 1, -1 }, .{ -1, 1, -1 },
        .{ -1, -1, 1 },  .{ 1, -1, 1 },  .{ 1, 1, 1 },  .{ -1, 1, 1 },
    };

    var vertices = try allocator.alloc(P3Vertex, 24); // 6 faces × 4 vertices
    var indices = try allocator.alloc(u32, 36); // 6 faces × 2 triangles × 3 indices

    var vi: u32 = 0;
    var ii: u32 = 0;

    // 6 faces of the cube
    const faces = [6][4]u32{
        .{ 0, 1, 2, 3 }, // front
        .{ 5, 4, 7, 6 }, // back
        .{ 4, 0, 3, 7 }, // left
        .{ 1, 5, 6, 2 }, // right
        .{ 3, 2, 6, 7 }, // top
        .{ 4, 5, 1, 0 }, // bottom
    };

    for (0..6) |fi| {
        const face = faces[fi];
        const base_vi = vi;

        for (0..4) |ci| {
            const c = corners[face[ci]];
            // Homogenize: [x, y, z, 1]
            const len = @sqrt(c[0] * c[0] + c[1] * c[1] + c[2] * c[2] + 1.0);
            const nx: f32 = @floatCast(c[0] / len);
            const ny: f32 = @floatCast(c[1] / len);
            const nz: f32 = @floatCast(c[2] / len);
            const nw: f32 = @floatCast(1.0 / len);

            // Normal: face normal in P³
            const normals = [6][4]f32{
                .{ 0, 0, -1, 0 }, .{ 0, 0, 1, 0 },
                .{ -1, 0, 0, 0 }, .{ 1, 0, 0, 0 },
                .{ 0, 1, 0, 0 },  .{ 0, -1, 0, 0 },
            };
            const n = normals[fi];

            vertices[vi] = .{
                .px = nx, .py = ny, .pz = nz, .pw = nw,
                .nx = n[0], .ny = n[1], .nz = n[2], .nw = n[3],
                .u = if (ci == 0 or ci == 3) 0.0 else 1.0,
                .v = if (ci == 0 or ci == 1) 0.0 else 1.0,
            };
            vi += 1;
        }

        // Two triangles per face
        indices[ii] = base_vi;
        indices[ii + 1] = base_vi + 1;
        indices[ii + 2] = base_vi + 2;
        indices[ii + 3] = base_vi;
        indices[ii + 4] = base_vi + 2;
        indices[ii + 5] = base_vi + 3;
        ii += 6;
    }

    return .{ .vertices = vertices, .indices = indices };
}

// =============================================================================
// 4. ТЕСТЫ (БЕЗ GPU — ПРОВЕРЯЕМ LAYOUT И WGSL)
// =============================================================================

test "GPU RT: P3Vertex is 40 bytes" {
    try std.testing.expect(@sizeOf(P3Vertex) == 40);
}

test "GPU RT: WGSL modules are non-empty" {
    try std.testing.expect(wgslFsDistanceModule().len > 0);
    try std.testing.expect(wgslPglActionModule().len > 0);
    try std.testing.expect(wgslGeodesicRK4Module().len > 0);
    try std.testing.expect(wgslIdempotentNewtonModule().len > 0);
    try std.testing.expect(wgslDehomogenizeModule().len > 0);
    try std.testing.expect(wgsl_p3_fragment.len > 0);
}

test "GPU RT: WGSL FS-distance module contains HomVec4 struct" {
    const module = wgslFsDistanceModule();
    try std.testing.expect(std.mem.indexOf(u8, module, "struct HomVec4") != null);
    try std.testing.expect(std.mem.indexOf(u8, module, "fs_distance_batch") != null);
}

test "GPU RT: WGSL PGL-action module contains PGL4 struct" {
    const module = wgslPglActionModule();
    try std.testing.expect(std.mem.indexOf(u8, module, "struct HomVec4") != null);
    try std.testing.expect(std.mem.indexOf(u8, module, "struct PGL4") != null);
    try std.testing.expect(std.mem.indexOf(u8, module, "pgl_action_batch") != null);
}

test "GPU RT: WGSL Geodesic RK4 module contains GeodesicState" {
    const module = wgslGeodesicRK4Module();
    try std.testing.expect(std.mem.indexOf(u8, module, "struct GeodesicState") != null);
    try std.testing.expect(std.mem.indexOf(u8, module, "geodesic_rk4_step") != null);
}

test "GPU RT: WGSL fragment shader has p3_fragment entry" {
    try std.testing.expect(std.mem.indexOf(u8, wgsl_p3_fragment, "p3_fragment") != null);
}

test "GPU RT: generateS3Points produces points on S³" {
    const allocator = std.testing.allocator;
    const points = try generateS3Points(allocator, 100);
    defer allocator.free(points);

    // Each point should have unit norm on S³
    for (points) |p| {
        const norm_sq = p.px * p.px + p.py * p.py + p.pz * p.pz + p.pw * p.pw;
        try std.testing.expectApproxEqAbs(norm_sq, 1.0, 0.01); // f32 precision
    }
}

test "GPU RT: generateS3Cube produces 24 vertices and 36 indices" {
    const allocator = std.testing.allocator;
    const result = try generateS3Cube(allocator);
    defer allocator.free(result.vertices);
    defer allocator.free(result.indices);

    try std.testing.expect(result.vertices.len == 24);
    try std.testing.expect(result.indices.len == 36);
}

test "GPU RT: GPU buffer sizes match WGSL structs" {
    // These must match for correct GPU upload
    try std.testing.expect(@sizeOf(GpuHomVec4) == 16); // 4 × f32
    try std.testing.expect(@sizeOf(GpuPGL4) == 64); // 16 × f32
    try std.testing.expect(@sizeOf(GpuGeodesicState) == 32); // 8 × f32
    try std.testing.expect(@sizeOf(GpuRK4Params) == 16); // 4 × f32 (with padding)
}

test "GPU RT: all WGSL shaders contain @compute or @vertex or @fragment" {
    const module_fs = wgslFsDistanceModule();
    try std.testing.expect(std.mem.indexOf(u8, module_fs, "@compute") != null);

    const module_pgl = wgslPglActionModule();
    try std.testing.expect(std.mem.indexOf(u8, module_pgl, "@compute") != null);

    const module_rk4 = wgslGeodesicRK4Module();
    try std.testing.expect(std.mem.indexOf(u8, module_rk4, "@compute") != null);

    const module_idem = wgslIdempotentNewtonModule();
    try std.testing.expect(std.mem.indexOf(u8, module_idem, "@compute") != null);

    const module_dehom = wgslDehomogenizeModule();
    try std.testing.expect(std.mem.indexOf(u8, module_dehom, "@vertex") != null);

    try std.testing.expect(std.mem.indexOf(u8, wgsl_p3_fragment, "@fragment") != null);
}
