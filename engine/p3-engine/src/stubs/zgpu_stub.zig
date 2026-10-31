// =============================================================================
// STUB: zgpu — интерфейс для компиляции без GPU
// =============================================================================
// Обеспечивает достаточный набор типов и методов, чтобы p3_gpu_rt.zig
// и p3_app.zig компилировались. Все методы возвращают undefined/0.
// Реальная работа GPU — только через `zig build p3-gpu` (реальный zgpu).

const std = @import("std");

pub const wgpu = struct {
    // ---- Opaque GPU object types (with stub methods) ----

    pub const ShaderModule = *extern struct {
        pub fn release(self: ShaderModule) void { _ = self; }
    };
    pub const ComputePipeline = *extern struct {
        pub fn release(self: ComputePipeline) void { _ = self; }
        pub fn getBindGroupLayout(self: ComputePipeline, group_index: u32) BindGroupLayout {
            _ = self; _ = group_index; return undefined;
        }
    };
    pub const RenderPipeline = *extern struct {
        pub fn release(self: RenderPipeline) void { _ = self; }
        pub fn getBindGroupLayout(self: RenderPipeline, group_index: u32) BindGroupLayout {
            _ = self; _ = group_index; return undefined;
        }
    };
    pub const CommandEncoder = *extern struct {
        pub fn beginComputePass(self: CommandEncoder, desc: ?*const ComputePassDescriptor) ComputePassEncoder {
            _ = self; _ = desc; return undefined;
        }
        pub fn beginRenderPass(self: CommandEncoder, desc: RenderPassDescriptor) RenderPassEncoder {
            _ = self; _ = desc; return undefined;
        }
        pub fn finish(self: CommandEncoder, desc: ?*const CommandBufferDescriptor) CommandBuffer {
            _ = self; _ = desc; return undefined;
        }
        pub fn release(self: CommandEncoder) void { _ = self; }
    };
    pub const ComputePassEncoder = *extern struct {
        pub fn setPipeline(self: ComputePassEncoder, pipeline: ComputePipeline) void { _ = self; _ = pipeline; }
        pub fn setBindGroup(self: ComputePassEncoder, group_index: u32, group: BindGroup, dynamic_offsets: []const u32) void { _ = self; _ = group_index; _ = group; _ = dynamic_offsets; }
        pub fn dispatchWorkgroups(self: ComputePassEncoder, workgroup_count_x: u32, workgroup_count_y: u32, workgroup_count_z: u32) void { _ = self; _ = workgroup_count_x; _ = workgroup_count_y; _ = workgroup_count_z; }
        pub fn end(self: ComputePassEncoder) void { _ = self; }
        pub fn release(self: ComputePassEncoder) void { _ = self; }
    };
    pub const RenderPassEncoder = *extern struct {
        pub fn setPipeline(self: RenderPassEncoder, pipeline: RenderPipeline) void { _ = self; _ = pipeline; }
        pub fn setBindGroup(self: RenderPassEncoder, group_index: u32, group: BindGroup, dynamic_offsets: []const u32) void { _ = self; _ = group_index; _ = group; _ = dynamic_offsets; }
        pub fn setVertexBuffer(self: RenderPassEncoder, slot: u32, buffer: Buffer, offset: u64, size: u64) void { _ = self; _ = slot; _ = buffer; _ = offset; _ = size; }
        pub fn draw(self: RenderPassEncoder, vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32) void { _ = self; _ = vertex_count; _ = instance_count; _ = first_vertex; _ = first_instance; }
        pub fn end(self: RenderPassEncoder) void { _ = self; }
        pub fn release(self: RenderPassEncoder) void { _ = self; }
    };
    pub const CommandBuffer = *extern struct {
        pub fn release(self: CommandBuffer) void { _ = self; }
    };
    pub const Device = *extern struct {
        pub fn createCommandEncoder(self: Device, desc: ?*const CommandEncoderDescriptor) CommandEncoder {
            _ = self; _ = desc; return undefined;
        }
        pub fn createSwapChain(self: Device, surface: Surface, desc: SwapChainDescriptor) SwapChain {
            _ = self; _ = surface; _ = desc; return undefined;
        }
        pub fn getQueue(self: Device) Queue { _ = self; return undefined; }
        pub fn release(self: Device) void { _ = self; }
        pub fn tick(self: Device) void { _ = self; }
    };
    pub const Queue = *extern struct {
        pub fn writeBuffer(self: Queue, buffer: Buffer, offset: u64, comptime T: type, data: []const T) void {
            _ = self; _ = buffer; _ = offset; _ = data;
        }
        pub fn submit(self: Queue, commands: []const CommandBuffer) void { _ = self; _ = commands; }
        pub fn release(self: Queue) void { _ = self; }
        pub fn onSubmittedWorkDone(self: Queue, signal_value: u64, callback: anytype, userdata: ?*anyopaque) void {
            _ = self; _ = signal_value; _ = callback; _ = userdata;
        }
    };
    pub const Buffer = *extern struct {
        pub fn release(self: Buffer) void { _ = self; }
        pub fn getMappedRange(self: Buffer, comptime T: type, offset: u64, size: u64) ?[]T {
            _ = self; _ = offset; _ = size; return null;
        }
        pub fn mapAsync(self: Buffer, mode: anytype, offset: u64, size: u64, callback: anytype, userdata: ?*anyopaque) void {
            _ = self; _ = mode; _ = offset; _ = size; _ = callback; _ = userdata;
        }
        pub fn unmap(self: Buffer) void { _ = self; }
    };
    pub const Texture = *extern struct {
        pub fn createView(self: Texture, desc: TextureViewDescriptor) TextureView { _ = self; _ = desc; return undefined; }
        pub fn release(self: Texture) void { _ = self; }
    };
    pub const TextureView = *extern struct {
        pub fn release(self: TextureView) void { _ = self; }
    };
    pub const BindGroup = *extern struct {
        pub fn release(self: BindGroup) void { _ = self; }
    };
    pub const BindGroupLayout = *extern struct {
        pub fn release(self: BindGroupLayout) void { _ = self; }
    };
    pub const PipelineLayout = *extern struct {
        pub fn release(self: PipelineLayout) void { _ = self; }
    };
    pub const Sampler = *extern struct {
        pub fn release(self: Sampler) void { _ = self; }
    };
    pub const Surface = *extern struct {
        pub fn release(self: Surface) void { _ = self; }
    };
    pub const SwapChain = *extern struct {
        pub fn getCurrentTextureView(self: SwapChain) TextureView { _ = self; return undefined; }
        pub fn present(self: SwapChain) void { _ = self; }
        pub fn release(self: SwapChain) void { _ = self; }
    };
    pub const Instance = *extern struct {
        pub fn release(self: Instance) void { _ = self; }
    };
    pub const QuerySet = *extern struct {};

    // ---- Descriptor types ----

    pub const ComputePipelineDescriptor = struct {
        label: ?[*:0]const u8 = null,
        layout: ?PipelineLayout = null,
        compute: ProgrammableStageDescriptor,
    };
    pub const ProgrammableStageDescriptor = struct {
        module: ShaderModule,
        entry_point: [*:0]const u8,
        constant_count: usize = 0,
        constants: ?[*]const ConstantEntry = null,
    };
    pub const ConstantEntry = struct {
        key: [*:0]const u8 = "",
        value: f64 = 0,
    };
    pub const RenderPipelineDescriptor = struct {
        label: ?[*:0]const u8 = null,
        layout: ?PipelineLayout = null,
        vertex: VertexState,
        primitive: PrimitiveState = .{},
        depth_stencil: ?*const DepthStencilState = null,
        multisample: MultisampleState = .{},
        fragment: ?*const FragmentState = null,
    };
    pub const VertexState = struct {
        module: ShaderModule = undefined,
        entry_point: [*:0]const u8 = "",
        constant_count: usize = 0,
        constants: ?[*]const ConstantEntry = null,
        buffer_count: usize = 0,
        buffers: ?[*]const VertexBufferLayout = null,
    };
    pub const PrimitiveState = struct {
        topology: PrimitiveTopology = .triangle_list,
        strip_index_format: IndexFormat = .undef,
        front_face: FrontFace = .ccw,
        cull_mode: CullMode = .none,
    };
    pub const DepthStencilState = struct {
        format: TextureFormat = .undef,
        depth_write_enabled: bool = false,
        depth_compare: CompareFunction = .always,
        stencil_front: StencilFaceState = .{},
        stencil_back: StencilFaceState = .{},
        stencil_read_mask: u32 = 0xFFFFFFFF,
        stencil_write_mask: u32 = 0xFFFFFFFF,
        depth_bias: i32 = 0,
        depth_bias_slope_scale: f32 = 0,
        depth_bias_clamp: f32 = 0,
    };
    pub const StencilFaceState = struct {
        compare: CompareFunction = .always,
        fail_op: StencilOp = .keep,
        depth_fail_op: StencilOp = .keep,
        pass_op: StencilOp = .keep,
    };
    pub const MultisampleState = struct {
        count: u32 = 1,
        mask: u32 = 0xFFFFFFFF,
        alpha_to_coverage_enabled: bool = false,
    };
    pub const FragmentState = struct {
        module: ShaderModule = undefined,
        entry_point: [*:0]const u8 = "",
        constant_count: usize = 0,
        constants: ?[*]const ConstantEntry = null,
        target_count: usize = 0,
        targets: ?[*]const ColorTargetState = null,
    };
    pub const VertexBufferLayout = struct {
        array_stride: u64 = 0,
        step_mode: VertexStepMode = .vertex,
        attribute_count: usize = 0,
        attributes: ?[*]const VertexAttribute = null,
    };
    pub const VertexAttribute = struct {
        format: VertexFormat = .float32x4,
        offset: u64 = 0,
        shader_location: u32 = 0,
    };
    pub const ColorTargetState = struct {
        format: TextureFormat = .rgba8_unorm,
        blend: ?BlendState = null,
        write_mask: ColorWriteMask = .{ .red = true, .green = true, .blue = true, .alpha = true },
    };
    pub const BlendState = struct {
        color: BlendComponent = .{},
        alpha: BlendComponent = .{},
    };
    pub const BlendComponent = struct {
        operation: BlendOperation = .add,
        src_factor: BlendFactor = .one,
        dst_factor: BlendFactor = .zero,
    };
    pub const RenderPassDescriptor = struct {
        label: ?[*:0]const u8 = null,
        color_attachment_count: usize = 0,
        color_attachments: ?[*]const RenderPassColorAttachment = null,
        depth_stencil_attachment: ?*const RenderPassDepthStencilAttachment = null,
        occlusion_query_set: ?QuerySet = null,
        timestamp_write_count: usize = 0,
        timestamp_writes: ?[*]const RenderPassTimestampWrite = null,
    };
    pub const RenderPassColorAttachment = struct {
        view: ?TextureView = null,
        depth_slice: u32 = std.math.maxInt(u32),
        resolve_target: ?TextureView = null,
        load_op: LoadOp = .clear,
        store_op: StoreOp = .store,
        clear_value: Color = .{ .r = 0, .g = 0, .b = 0, .a = 0 },
    };
    pub const RenderPassDepthStencilAttachment = struct {
        view: TextureView = undefined,
        depth_load_op: LoadOp = .undef,
        depth_store_op: StoreOp = .undef,
        depth_clear_value: f32 = 0.0,
        depth_read_only: bool = false,
        stencil_load_op: LoadOp = .undef,
        stencil_store_op: StoreOp = .undef,
        stencil_clear_value: u32 = 0,
        stencil_read_only: bool = false,
    };
    pub const RenderPassTimestampWrite = struct {
        query_set: QuerySet = undefined,
        beginning_of_pass_write_index: u32 = 0,
        end_of_pass_write_index: u32 = 0,
    };

    pub const ComputePassDescriptor = struct {
        label: ?[*:0]const u8 = null,
        timestamp_write_count: usize = 0,
        timestamp_writes: ?[*]const ComputePassTimestampWrite = null,
    };
    pub const ComputePassTimestampWrite = struct {
        query_set: QuerySet = undefined,
        beginning_of_pass_write_index: u32 = 0,
        end_of_pass_write_index: u32 = 0,
    };

    pub const CommandEncoderDescriptor = struct {
        label: ?[*:0]const u8 = null,
    };
    pub const CommandBufferDescriptor = struct {
        label: ?[*:0]const u8 = null,
    };

    pub const BufferDescriptor = struct {
        label: ?[*:0]const u8 = null,
        usage: BufferUsage = .{},
        size: u64 = 0,
        mapped_at_creation: bool = false,
    };
    pub const BufferUsage = packed struct(u32) {
        map_read: bool = false,
        map_write: bool = false,
        copy_src: bool = false,
        copy_dst: bool = false,
        index: bool = false,
        vertex: bool = false,
        uniform: bool = false,
        storage: bool = false,
        indirect: bool = false,
        query_resolve: bool = false,
        _pad: u22 = 0,
    };
    pub const TextureDescriptor = struct {
        label: ?[*:0]const u8 = null,
        usage: TextureUsage = .{},
        dimension: TextureDimension = .tdim_2d,
        size: Extent3D = .{ .width = 1, .height = 1, .depth_or_array_layers = 1 },
        format: TextureFormat = .rgba8_unorm,
        mip_level_count: u32 = 1,
        sample_count: u32 = 1,
        view_format_count: usize = 0,
        view_formats: ?[*]const TextureFormat = null,
    };
    pub const TextureUsage = packed struct(u32) {
        copy_src: bool = false,
        copy_dst: bool = false,
        texture_binding: bool = false,
        storage_binding: bool = false,
        render_attachment: bool = false,
        _pad: u27 = 0,
    };
    pub const TextureViewDescriptor = struct {
        label: ?[*:0]const u8 = null,
        format: TextureFormat = .undef,
        dimension: TextureViewDimension = .undef,
        base_mip_level: u32 = 0,
        mip_level_count: u32 = 0xffff_ffff,
        base_array_layer: u32 = 0,
        array_layer_count: u32 = 0xffff_ffff,
        aspect: TextureAspect = .all,
    };
    pub const Extent3D = struct {
        width: u32 = 1,
        height: u32 = 1,
        depth_or_array_layers: u32 = 1,
    };

    pub const BindGroupDescriptor = struct {
        label: ?[*:0]const u8 = null,
        layout: BindGroupLayout = undefined,
        entry_count: u32 = 0,
        entries: ?[*]const BindGroupEntry = null,
    };
    pub const BindGroupEntry = struct {
        binding: u32 = 0,
        buffer: ?Buffer = null,
        offset: u64 = 0,
        size: u64 = 0,
        sampler: ?Sampler = null,
        texture_view: ?TextureView = null,
    };
    pub const BindGroupLayoutDescriptor = struct {
        label: ?[*:0]const u8 = null,
        entry_count: u32 = 0,
        entries: ?[*]const BindGroupLayoutEntry = null,
    };
    pub const BindGroupLayoutEntry = struct {
        binding: u32 = 0,
        visibility: ShaderStage = .{},
        buffer: BufferBindingLayout = .{ .binding_type = .undef },
        sampler: SamplerBindingLayout = .{ .binding_type = .undef },
        texture: TextureBindingLayout = .{ .sample_type = .undef },
        storage_texture: StorageTextureBindingLayout = .{ .access = .undef, .format = .undef },
    };
    pub const ShaderStage = packed struct(u32) {
        vertex: bool = false,
        fragment: bool = false,
        compute: bool = false,
        _padding: u29 = 0,
    };
    pub const BufferBindingLayout = struct {
        binding_type: BufferBindingType = .uniform,
        has_dynamic_offset: bool = false,
        min_binding_size: u64 = 0,
    };
    pub const SamplerBindingLayout = struct {
        binding_type: SamplerBindingType = .filtering,
    };
    pub const TextureBindingLayout = struct {
        sample_type: TextureSampleType = .float,
        view_dimension: TextureViewDimension = .tvdim_2d,
        multisampled: bool = false,
    };
    pub const StorageTextureBindingLayout = struct {
        access: StorageTextureAccess = .write_only,
        format: TextureFormat = .undef,
        view_dimension: TextureViewDimension = .tvdim_2d,
    };
    pub const PipelineLayoutDescriptor = struct {
        label: ?[*:0]const u8 = null,
        bind_group_layout_count: u32 = 0,
        bind_group_layouts: ?[*]const BindGroupLayout = null,
    };

    pub const SwapChainDescriptor = struct {
        label: ?[*:0]const u8 = null,
        usage: TextureUsage = .{ .render_attachment = true },
        format: TextureFormat = .bgra8_unorm,
        width: u32 = 1280,
        height: u32 = 720,
        present_mode: PresentMode = .fifo,
    };

    // ---- Enums ----
    pub const FrontFace = enum(u32) { ccw = 0, cw = 1 };
    pub const CullMode = enum(u32) { none = 0, front = 1, back = 2 };
    pub const PrimitiveTopology = enum(u32) { point_list = 0, line_list = 1, line_strip = 2, triangle_list = 3, triangle_strip = 4, _ };
    pub const IndexFormat = enum(u32) { undef = 0, uint16 = 1, uint32 = 2 };
    pub const CompareFunction = enum(u32) { undef = 0, never = 1, less = 2, less_equal = 3, greater = 4, greater_equal = 5, equal = 6, not_equal = 7, always = 8 };
    pub const StencilOp = enum(u32) { keep = 0, zero = 1, replace = 2, invert = 3, increment_clamp = 4, decrement_clamp = 5, increment_wrap = 6, decrement_wrap = 7 };
    pub const VertexFormat = enum(u32) { undef = 0, uint8x2 = 1, uint8x4 = 2, sint8x2 = 3, sint8x4 = 4, unorm8x2 = 5, unorm8x4 = 6, snorm8x2 = 7, snorm8x4 = 8, uint16x2 = 9, uint16x4 = 10, sint16x2 = 11, sint16x4 = 12, unorm16x2 = 13, unorm16x4 = 14, snorm16x2 = 15, snorm16x4 = 16, float16x2 = 17, float16x4 = 18, float32 = 19, float32x2 = 20, float32x3 = 21, float32x4 = 22, uint32 = 23, uint32x2 = 24, uint32x3 = 25, uint32x4 = 26, sint32 = 27, sint32x2 = 28, sint32x3 = 29, sint32x4 = 30, _ };
    pub const VertexStepMode = enum(u32) { vertex = 0, instance = 1 };
    pub const TextureFormat = enum(u32) { undef = 0, r8_unorm = 1, r8_snorm = 2, r8_uint = 3, r8_sint = 4, r16_uint = 5, r16_sint = 6, r16_float = 7, rg8_unorm = 8, rg8_snorm = 9, rg8_uint = 10, rg8_sint = 11, r32_float = 12, r32_uint = 13, r32_sint = 14, rg16_uint = 15, rg16_sint = 16, rg16_float = 17, rgba8_unorm = 18, rgba8_snorm = 19, rgba8_uint = 20, rgba8_sint = 21, bgra8_unorm = 22, bgra8_snorm = 23, rgba16_float = 24, rgba32_float = 25, depth16_unorm = 26, depth24_plus = 27, depth32_float = 28, depth24_plus_stencil8 = 29, depth32_float_stencil8 = 30, stencil8 = 31, bc1_rgba_unorm = 32, bc2_rgba_unorm = 33, bc3_rgba_unorm = 34, bc4_r_unorm = 35, bc5_rg_unorm = 36, bc7_rgba_unorm = 37, etc2_rgb8_unorm = 38, etc2_rgb8a1_unorm = 39, eac_r11_unorm = 40, eac_rg11_unorm = 41, rgba8_srgb = 42, bgra8_srgb = 43, _ };
    pub const LoadOp = enum(u32) { undef = 0, clear = 1, load = 2 };
    pub const StoreOp = enum(u32) { undef = 0, store = 1, discard = 2 };
    pub const BlendOperation = enum(u32) { add = 0, subtract = 1, reverse_subtract = 2, min = 3, max = 4 };
    pub const BlendFactor = enum(u32) { zero = 0, one = 1, src = 2, one_minus_src = 3, src_alpha = 4, one_minus_src_alpha = 5, dst = 6, one_minus_dst = 7, dst_alpha = 8, one_minus_dst_alpha = 9, src_alpha_saturated = 10, constant = 11, one_minus_constant = 12 };
    pub const ColorWriteMask = packed struct(u4) { red: bool = true, green: bool = true, blue: bool = true, alpha: bool = true };
    pub const ColorWriteMaskAll: ColorWriteMask = .{ .red = true, .green = true, .blue = true, .alpha = true };
    pub const TextureDimension = enum(u32) { undef = 0, tdim_1d = 1, tdim_2d = 2, tdim_3d = 3 };
    pub const TextureViewDimension = enum(u32) { undef = 0, tvdim_1d = 1, tvdim_2d = 2, tvdim_2d_array = 3, tvdim_cube = 4, tvdim_cube_array = 5, tvdim_3d = 6 };
    pub const TextureAspect = enum(u32) { all = 0, stencil_only = 1, depth_only = 2 };
    pub const TextureSampleType = enum(u32) { undef = 0, float = 1, unfilterable_float = 2, depth = 3, sint = 4, uint = 5 };
    pub const SamplerBindingType = enum(u32) { undef = 0, filtering = 1, non_filtering = 2, comparison = 3 };
    pub const BufferBindingType = enum(u32) { undef = 0, uniform = 1, storage = 2, read_only_storage = 3 };
    pub const StorageTextureAccess = enum(u32) { undef = 0, write_only = 1, read_only = 2, read_write = 3 };
    pub const PresentMode = enum(u32) { undef = 0, immediate = 1, mailbox = 2, fifo = 3, fifo_relaxed = 4 };

    pub const Color = struct { r: f64, g: f64, b: f64, a: f64 };

    pub const FeatureName = enum(u32) { _ };
    pub const RequiredLimits = extern struct { limits: Limits = .{} };
    pub const Limits = extern struct { max_buffer_size: u64 = 0 };
};

// ---- Handle types (distinct structs, matching zgpu pattern) ----
pub const BufferHandle = struct { index: u32 = 0 };
pub const TextureHandle = struct { index: u32 = 0 };
pub const TextureViewHandle = struct { index: u32 = 0 };
pub const BindGroupLayoutHandle = struct { index: u32 = 0 };
pub const BindGroupHandle = struct { index: u32 = 0 };
pub const PipelineLayoutHandle = struct { index: u32 = 0 };
pub const ComputePipelineHandle = struct { index: u32 = 0 };
pub const RenderPipelineHandle = struct { index: u32 = 0 };
pub const SamplerHandle = struct { index: u32 = 0 };

// ---- BindGroupEntryInfo (matches real zgpu) ----
pub const BindGroupEntryInfo = struct {
    binding: u32 = 0,
    buffer_handle: ?BufferHandle = null,
    offset: u64 = 0,
    size: u64 = 0,
    sampler_handle: ?SamplerHandle = null,
    texture_view_handle: ?TextureViewHandle = null,
};

// ---- Resource info types ----
pub const BufferResourceInfo = struct {
    gpuobj: ?wgpu.Buffer = null,
    size: u64 = 0,
    usage: wgpu.BufferUsage = .{},
};
pub const TextureResourceInfo = struct {
    gpuobj: ?wgpu.Texture = null,
    usage: wgpu.TextureUsage = .{},
    dimension: wgpu.TextureDimension = .tdim_2d,
    size: wgpu.Extent3D = .{},
    format: wgpu.TextureFormat = .undef,
    mip_level_count: u32 = 1,
    sample_count: u32 = 1,
};
pub const TextureViewResourceInfo = struct {
    gpuobj: ?wgpu.TextureView = null,
    format: wgpu.TextureFormat = .undef,
    dimension: wgpu.TextureViewDimension = .tvdim_2d,
    base_mip_level: u32 = 0,
    mip_level_count: u32 = 1,
    base_array_layer: u32 = 0,
    array_layer_count: u32 = 1,
    aspect: wgpu.TextureAspect = .all,
    parent_texture_handle: TextureHandle = 0,
};

// ---- GraphicsContext ----
pub const GraphicsContext = struct {
    pub const PresentResult = enum { normal_execution, swap_chain_resized };
    pub const swapchain_format: wgpu.TextureFormat = .bgra8_unorm;

    device: wgpu.Device = undefined,
    queue: wgpu.Queue = undefined,
    swapchain: wgpu.SwapChain = undefined,
    swapchain_descriptor: wgpu.SwapChainDescriptor = .{},
    swapchain_dimensions: Dimensions = .{ .width = 1280, .height = 720 },

    pub const Dimensions = struct { width: u32, height: u32 };

    pub const WindowProvider = struct {
        window: *anyopaque,
        fn_getTime: *const fn () f64,
        fn_getFramebufferSize: *const fn (window: *const anyopaque) [2]u32,
        fn_getWin32Window: *const fn (window: *const anyopaque) callconv(.C) *anyopaque = undefined,
        fn_getX11Display: *const fn () callconv(.C) *anyopaque = undefined,
        fn_getX11Window: *const fn (window: *const anyopaque) callconv(.C) u32 = undefined,
        fn_getWaylandDisplay: ?*const fn () callconv(.C) *anyopaque = null,
        fn_getWaylandSurface: ?*const fn (window: *const anyopaque) callconv(.C) *anyopaque = null,
        fn_getCocoaWindow: *const fn (window: *const anyopaque) callconv(.C) ?*anyopaque = undefined,
    };

    pub fn create(allocator: std.mem.Allocator, window_provider: WindowProvider, opts: anytype) !*GraphicsContext {
        _ = window_provider;
        _ = opts;
        const gctx = try allocator.create(GraphicsContext);
        gctx.* = .{};
        return gctx;
    }

    pub fn deinit(self: *GraphicsContext, allocator: std.mem.Allocator) void {
        _ = self;
        _ = allocator;
    }

    pub fn present(self: *GraphicsContext) PresentResult {
        _ = self;
        return .normal_execution;
    }

    pub fn destroy(self: *GraphicsContext, allocator: std.mem.Allocator) void {
        self.deinit(allocator);
    }

    pub fn createBindGroupLayout(self: *GraphicsContext, entries: []const wgpu.BindGroupLayoutEntry) BindGroupLayoutHandle {
        _ = self;
        _ = entries;
        return .{};
    }
    pub fn createPipelineLayout(self: *GraphicsContext, bgl_handles: []const BindGroupLayoutHandle) PipelineLayoutHandle {
        _ = self;
        _ = bgl_handles;
        return .{};
    }
    pub fn createComputePipeline(self: *GraphicsContext, pipeline_layout: PipelineLayoutHandle, descriptor: wgpu.ComputePipelineDescriptor) ComputePipelineHandle {
        _ = self;
        _ = pipeline_layout;
        _ = descriptor;
        return .{};
    }
    pub fn createRenderPipeline(self: *GraphicsContext, pipeline_layout: PipelineLayoutHandle, descriptor: wgpu.RenderPipelineDescriptor) RenderPipelineHandle {
        _ = self;
        _ = pipeline_layout;
        _ = descriptor;
        return .{};
    }
    pub fn createBuffer(self: *GraphicsContext, descriptor: wgpu.BufferDescriptor) BufferHandle {
        _ = self;
        _ = descriptor;
        return .{};
    }
    pub fn createBindGroup(self: *GraphicsContext, layout: BindGroupLayoutHandle, entries: []const BindGroupEntryInfo) BindGroupHandle {
        _ = self;
        _ = layout;
        _ = entries;
        return .{};
    }
    pub fn createTexture(self: *GraphicsContext, descriptor: wgpu.TextureDescriptor) TextureHandle {
        _ = self;
        _ = descriptor;
        return .{};
    }
    pub fn createTextureView(self: *GraphicsContext, texture: TextureHandle, descriptor: wgpu.TextureViewDescriptor) TextureViewHandle {
        _ = self;
        _ = texture;
        _ = descriptor;
        return .{};
    }
    pub fn createSampler(self: *GraphicsContext, descriptor: anytype) SamplerHandle {
        _ = self;
        _ = descriptor;
        return .{};
    }

    pub fn lookupResource(self: GraphicsContext, handle: anytype) ?LookupResourceResult(@TypeOf(handle)) {
        _ = self;
        return null;
    }

    pub fn lookupResourceInfo(self: GraphicsContext, handle: anytype) ?LookupResourceInfoResult(@TypeOf(handle)) {
        _ = self;
        return null;
    }

    pub fn submit(self: *GraphicsContext, commands: []const wgpu.CommandBuffer) void {
        _ = self;
        _ = commands;
    }
    pub fn releaseResource(self: *GraphicsContext, handle: anytype) void {
        _ = self;
        _ = handle;
    }
    pub fn destroyResource(self: *GraphicsContext, handle: anytype) void {
        _ = self;
        _ = handle;
    }
    pub fn isResourceValid(self: GraphicsContext, handle: anytype) bool {
        _ = self;
        _ = handle;
        return false;
    }
    pub fn canRender(self: *GraphicsContext) bool {
        _ = self;
        return true;
    }
};

// ---- Lookup helper types ----
pub fn LookupResourceResult(comptime Handle: type) type {
    return switch (Handle) {
        BufferHandle => wgpu.Buffer,
        TextureHandle => wgpu.Texture,
        TextureViewHandle => wgpu.TextureView,
        SamplerHandle => wgpu.Sampler,
        RenderPipelineHandle => wgpu.RenderPipeline,
        ComputePipelineHandle => wgpu.ComputePipeline,
        BindGroupHandle => wgpu.BindGroup,
        BindGroupLayoutHandle => wgpu.BindGroupLayout,
        PipelineLayoutHandle => wgpu.PipelineLayout,
        else => @compileError("[zgpu stub] lookupResource not implemented for " ++ @typeName(Handle)),
    };
}

pub fn LookupResourceInfoResult(comptime Handle: type) type {
    return switch (Handle) {
        BufferHandle => BufferResourceInfo,
        TextureHandle => TextureResourceInfo,
        TextureViewHandle => TextureViewResourceInfo,
        else => @compileError("[zgpu stub] lookupResourceInfo not implemented for " ++ @typeName(Handle)),
    };
}

// ---- Shader module creation ----
pub fn createWgslShaderModule(device: anytype, code: [:0]const u8, label: [:0]const u8) wgpu.ShaderModule {
    _ = device;
    _ = code;
    _ = label;
    return undefined;
}

pub fn addLibraryPathsTo(exe: *std.Build.Step.Compile) void {
    _ = exe;
}

// ---- Binding layout helpers (matching zgpu API) ----
pub const BufferVisibility = wgpu.ShaderStage;
pub const BufferType = enum { storage, uniform };

pub fn bufferEntry(
    binding: u32,
    visibility: wgpu.ShaderStage,
    binding_type: wgpu.BufferBindingType,
    has_dynamic_offset: bool,
    min_binding_size: u64,
) wgpu.BindGroupLayoutEntry {
    return .{
        .binding = binding,
        .visibility = visibility,
        .buffer = .{
            .binding_type = binding_type,
            .has_dynamic_offset = has_dynamic_offset,
            .min_binding_size = min_binding_size,
        },
    };
}

pub fn textureEntry(
    binding: u32,
    visibility: wgpu.ShaderStage,
    sample_type: wgpu.TextureSampleType,
    view_dimension: wgpu.TextureViewDimension,
    multisampled: bool,
) wgpu.BindGroupLayoutEntry {
    return .{
        .binding = binding,
        .visibility = visibility,
        .texture = .{
            .sample_type = sample_type,
            .view_dimension = view_dimension,
            .multisampled = multisampled,
        },
    };
}

pub fn storageTextureEntry(
    binding: u32,
    visibility: wgpu.ShaderStage,
    access: wgpu.StorageTextureAccess,
    format: wgpu.TextureFormat,
    view_dimension: wgpu.TextureViewDimension,
) wgpu.BindGroupLayoutEntry {
    return .{
        .binding = binding,
        .visibility = visibility,
        .storage_texture = .{
            .access = access,
            .format = format,
            .view_dimension = view_dimension,
        },
    };
}

pub fn samplerEntry(
    binding: u32,
    visibility: wgpu.ShaderStage,
    binding_type: wgpu.SamplerBindingType,
) wgpu.BindGroupLayoutEntry {
    return .{
        .binding = binding,
        .visibility = visibility,
        .sampler = .{ .binding_type = binding_type },
    };
}
