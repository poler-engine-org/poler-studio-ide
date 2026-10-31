// =============================================================================
// P³ ENGINE (PROJECTIVE 3D ENGINE) — UNIFIED PUBLIC API
// =============================================================================
//
// Root module exporting all mathematical, physical, astronomical, optical,
// GPU, ECS, and UI subsystems of the P³ Engine.
// =============================================================================

const std = @import("std");

// Subsystem Modules
pub const math = @import("p3_math.zig");
pub const kernel = @import("p3_kernel.zig");
pub const scale = @import("p3_scale.zig");
pub const causal = @import("p3_causal.zig");
pub const poler = @import("p3_poler.zig");
pub const optics = @import("p3_null_fluid.zig");
pub const pga = @import("p3_pga.zig");
pub const dual_quat = @import("p3_dual_quat.zig");
pub const ecs = @import("p3_ecs.zig");
pub const scene = @import("p3_scene.zig");
pub const physics = @import("p3_physics.zig");
pub const gpu = @import("p3_gpu.zig");
pub const gpu_rt = @import("p3_gpu_rt.zig");
pub const rhi = @import("p3_rhi.zig");
pub const renderer = @import("p3_renderer.zig");
pub const input = @import("p3_input.zig");
pub const safety = @import("p3_safety.zig");

// UI Subsystems (O3DE LyShine Port)
pub const ui = struct {
    pub const canvas = @import("p3_ui_canvas.zig");
    pub const transform = @import("p3_ui_transform.zig");
    pub const layout = @import("p3_ui_layout.zig");
    pub const draw = @import("p3_ui_draw.zig");
    pub const text = @import("p3_ui_text.zig");
    pub const image = @import("p3_ui_image.zig");
    pub const button = @import("p3_ui_button.zig");
    pub const scroll = @import("p3_ui_scroll.zig");
    pub const animation = @import("p3_ui_animation.zig");
    pub const interactable = @import("p3_ui_interactable.zig");
    pub const dragdrop = @import("p3_ui_dragdrop.zig");
    pub const navigation = @import("p3_ui_navigation.zig");
    pub const render = @import("p3_ui_render.zig");
    pub const raylib = @import("p3_ui_raylib.zig");
};

// Top-Level Unified `p3` Namespace
pub const p3 = struct {
    pub const math = @import("p3_math.zig");
    pub const kernel = @import("p3_kernel.zig");
    pub const scale = @import("p3_scale.zig");
    pub const causal = @import("p3_causal.zig");
    pub const poler = @import("p3_poler.zig");
    pub const optics = @import("p3_null_fluid.zig");
    pub const pga = @import("p3_pga.zig");
    pub const dual_quat = @import("p3_dual_quat.zig");
    pub const ecs = @import("p3_ecs.zig");
    pub const scene = @import("p3_scene.zig");
    pub const physics = @import("p3_physics.zig");
    pub const gpu = @import("p3_gpu.zig");
    pub const gpu_rt = @import("p3_gpu_rt.zig");
    pub const rhi = @import("p3_rhi.zig");
    pub const renderer = @import("p3_renderer.zig");
    pub const ui = @import("root.zig").ui;
    pub const procedural_mesh = @import("p3_procedural_mesh.zig");
    pub const vision = @import("p3_vision.zig");
};

pub const generateSpaceshipMesh = @import("p3_procedural_mesh.zig").generateSpaceshipMesh;
pub const generateHoverTankMesh = @import("p3_procedural_mesh.zig").generateHoverTankMesh;
pub const generateCyberBolideMesh = @import("p3_procedural_mesh.zig").generateCyberBolideMesh;
pub const TankConfig = @import("p3_procedural_mesh.zig").TankConfig;
pub const BolideConfig = @import("p3_procedural_mesh.zig").BolideConfig;
pub const ProceduralMesh = @import("p3_procedural_mesh.zig").ProceduralMesh;
pub const ShipHullConfig = @import("p3_procedural_mesh.zig").ShipHullConfig;
pub const VisualFrameBuffer = @import("p3_vision.zig").VisualFrameBuffer;
pub const ProjectiveRasterizer = @import("p3_vision.zig").ProjectiveRasterizer;
pub const VisualObservation = @import("p3_vision.zig").VisualObservation;
pub const ObservationPublisher = @import("p3_vision.zig").ObservationPublisher;
pub const AgentAction = @import("p3_vision.zig").AgentAction;
pub const ActionBus = @import("p3_vision.zig").ActionBus;

// Core Geometric & Mathematical Primitives (Direct Export)
pub const Vec2 = math.Vec2;
pub const Vec3 = math.Vec3;
pub const Vec4 = math.Vec4;
pub const HomVec4 = kernel.HomVec4;
pub const Mat3x3 = math.Mat3x3;
pub const Mat4x4 = math.Mat4x4;
pub const PGL4 = kernel.PGL4;
pub const Quaternion = math.Quaternion;
pub const Rotator = math.Rotator;
pub const DualQuaternion = dual_quat.DualQuat;
pub const Transform = math.Transform;
pub const Ray = math.Ray;
pub const Plane = math.Plane;
pub const Aabb = math.Aabb;
pub const Frustum = math.Frustum;
pub const Projection = math.Projection;
pub const InterpCurve = math.InterpCurve;
pub const Color = math.Color;
pub const Uuid = math.Uuid;

// Astrophysics & Scale Primitives
pub const PlanetScale = scale.PlanetScale;
pub const AstronomyTools = scale.AstronomyTools;

// Physical & Dispersive Optics
pub const DrudeReflectorOptics = optics.DrudeReflectorOptics;
pub const PhaseBoundaryConfig = optics.PhaseBoundaryConfig;
pub const ProjectiveOptics = optics.ProjectiveOptics;
pub const MediumZone = optics.MediumZone;

// =============================================================================
// ROOT TESTS & VERIFICATION
// =============================================================================

test "Root: API surface availability" {
    const v = Vec3.init(1, 2, 3);
    const p = HomVec4.init(v.x, v.y, v.z, 1.0);
    try std.testing.expectApproxEqAbs(p.x, 1.0, 1e-4);

    const rot = Rotator.init(10, 20, 30);
    const q = rot.toQuaternion();
    try std.testing.expectApproxEqAbs(q.length(), 1.0, 1e-4);

    const earth_scale = PlanetScale.earth();
    try std.testing.expectApproxEqAbs(earth_scale.gravity, 9.80665, 0.01);
}
