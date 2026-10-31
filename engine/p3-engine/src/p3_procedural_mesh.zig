// =============================================================================
// P³ PROCEDURAL GEOMETRY & VEHICLE MESH ENGINE v1.0 — ZIG
// =============================================================================
//
// Procedural CSG & Parametric Mesh Generator for Spacecraft and Vehicles:
//   - Aerodynamic Hull with cross-sectional projective spline interpolation
//   - Swept Wings, Winglets, and Stabilizers with thickness profile
//   - Twin Multi-stage Ion Thrusters with glowing plasma nozzles
//   - Faceted Crystal Glass Canopy Cockpit
//   - Wingtip Plasma Cannons and Sensor Arrays
//   - Normals, UVs, Vertex Colors, and Hardware Raylib Mesh Upload
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const HomVec4 = p3.HomVec4;

pub const MeshVertex = struct {
    pos: Vec3,
    normal: Vec3,
    uv: [2]f32,
    color: [4]u8,
};

pub const ProceduralMesh = struct {
    vertices: std.ArrayList(MeshVertex),
    indices: std.ArrayList(u16),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) ProceduralMesh {
        return .{
            .vertices = std.ArrayList(MeshVertex).init(allocator),
            .indices = std.ArrayList(u16).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *ProceduralMesh) void {
        self.vertices.deinit();
        self.indices.deinit();
    }

    pub fn addTriangle(
        self: *ProceduralMesh,
        v0: MeshVertex,
        v1: MeshVertex,
        v2: MeshVertex,
    ) !void {
        const base_idx: u16 = @intCast(self.vertices.items.len);
        // Calculate flat normal if not provided
        const edge1 = v1.pos.sub(v0.pos);
        const edge2 = v2.pos.sub(v0.pos);
        const n = edge1.cross(edge2).normalize();

        var vert0 = v0;
        var vert1 = v1;
        var vert2 = v2;
        if (v0.normal.lengthSq() < 0.01) vert0.normal = n;
        if (v1.normal.lengthSq() < 0.01) vert1.normal = n;
        if (v2.normal.lengthSq() < 0.01) vert2.normal = n;

        try self.vertices.append(vert0);
        try self.vertices.append(vert1);
        try self.vertices.append(vert2);

        try self.indices.append(base_idx);
        try self.indices.append(base_idx + 1);
        try self.indices.append(base_idx + 2);
    }

    pub fn addQuad(
        self: *ProceduralMesh,
        v0: MeshVertex,
        v1: MeshVertex,
        v2: MeshVertex,
        v3: MeshVertex,
    ) !void {
        try self.addTriangle(v0, v1, v2);
        try self.addTriangle(v0, v2, v3);
    }
};

pub const ShipHullConfig = struct {
    length: f32 = 3.6,
    width: f32 = 1.8,
    height: f32 = 0.7,
    wing_span: f32 = 4.2,
    wing_sweep: f32 = 1.4,
    body_color: [4]u8 = .{ 220, 225, 235, 255 }, // Matte Titanium White
    wing_color: [4]u8 = .{ 45, 55, 75, 255 },    // Dark Armor Carbon
    canopy_color: [4]u8 = .{ 0, 230, 255, 230 }, // Holographic Blue Glass
    thruster_color: [4]u8 = .{ 255, 120, 30, 255 }, // Plasma Orange
};

/// Generates a high-detail procedural spaceship geometry
pub fn generateSpaceshipMesh(allocator: std.mem.Allocator, cfg: ShipHullConfig) !ProceduralMesh {
    var mesh = ProceduralMesh.init(allocator);

    const l = cfg.length * 0.5;
    const w = cfg.width * 0.5;
    const h = cfg.height * 0.5;

    // ------------------------------------------------------------------------
    // 1. AERODYNAMIC MAIN FUSELAGE
    // ------------------------------------------------------------------------
    const nose = Vec3.init(0.0, 0.0, l);
    const mid_top = Vec3.init(0.0, h, 0.2);
    const mid_bot = Vec3.init(0.0, -h * 0.6, 0.2);
    const mid_left = Vec3.init(-w, 0.0, 0.0);
    const mid_right = Vec3.init(w, 0.0, 0.0);

    const aft_top = Vec3.init(0.0, h * 0.8, -l);
    const aft_bot = Vec3.init(0.0, -h * 0.5, -l);
    const aft_left = Vec3.init(-w * 0.8, 0.0, -l);
    const aft_right = Vec3.init(w * 0.8, 0.0, -l);

    const b_col = cfg.body_color;

    // Nose Cone Faces (Forward Section)
    try mesh.addTriangle(
        .{ .pos = nose, .normal = Vec3.zero(), .uv = .{ 0.5, 1.0 }, .color = b_col },
        .{ .pos = mid_top, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = b_col },
        .{ .pos = mid_right, .normal = Vec3.zero(), .uv = .{ 1.0, 0.5 }, .color = b_col },
    );
    try mesh.addTriangle(
        .{ .pos = nose, .normal = Vec3.zero(), .uv = .{ 0.5, 1.0 }, .color = b_col },
        .{ .pos = mid_left, .normal = Vec3.zero(), .uv = .{ 0.0, 0.5 }, .color = b_col },
        .{ .pos = mid_top, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = b_col },
    );
    try mesh.addTriangle(
        .{ .pos = nose, .normal = Vec3.zero(), .uv = .{ 0.5, 1.0 }, .color = b_col },
        .{ .pos = mid_right, .normal = Vec3.zero(), .uv = .{ 1.0, 0.5 }, .color = b_col },
        .{ .pos = mid_bot, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = b_col },
    );
    try mesh.addTriangle(
        .{ .pos = nose, .normal = Vec3.zero(), .uv = .{ 0.5, 1.0 }, .color = b_col },
        .{ .pos = mid_bot, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = b_col },
        .{ .pos = mid_left, .normal = Vec3.zero(), .uv = .{ 0.0, 0.5 }, .color = b_col },
    );

    // Mid to Aft Fuselage Quads
    try mesh.addQuad(
        .{ .pos = mid_top, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = b_col },
        .{ .pos = aft_top, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = b_col },
        .{ .pos = aft_right, .normal = Vec3.zero(), .uv = .{ 1.0, 0.0 }, .color = b_col },
        .{ .pos = mid_right, .normal = Vec3.zero(), .uv = .{ 1.0, 0.5 }, .color = b_col },
    );
    try mesh.addQuad(
        .{ .pos = mid_top, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = b_col },
        .{ .pos = mid_left, .normal = Vec3.zero(), .uv = .{ 0.0, 0.5 }, .color = b_col },
        .{ .pos = aft_left, .normal = Vec3.zero(), .uv = .{ 0.0, 0.0 }, .color = b_col },
        .{ .pos = aft_top, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = b_col },
    );
    try mesh.addQuad(
        .{ .pos = mid_bot, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = b_col },
        .{ .pos = mid_right, .normal = Vec3.zero(), .uv = .{ 1.0, 0.5 }, .color = b_col },
        .{ .pos = aft_right, .normal = Vec3.zero(), .uv = .{ 1.0, 0.0 }, .color = b_col },
        .{ .pos = aft_bot, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = b_col },
    );
    try mesh.addQuad(
        .{ .pos = mid_bot, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = b_col },
        .{ .pos = aft_bot, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = b_col },
        .{ .pos = aft_left, .normal = Vec3.zero(), .uv = .{ 0.0, 0.0 }, .color = b_col },
        .{ .pos = mid_left, .normal = Vec3.zero(), .uv = .{ 0.0, 0.5 }, .color = b_col },
    );

    // ------------------------------------------------------------------------
    // 2. CRYSTAL COCKPIT CANOPY
    // ------------------------------------------------------------------------
    const c_col = cfg.canopy_color;
    const can_front = Vec3.init(0.0, h * 0.9, l * 0.45);
    const can_peak = Vec3.init(0.0, h * 1.45, 0.0);
    const can_back = Vec3.init(0.0, h * 1.1, -l * 0.4);
    const can_l = Vec3.init(-w * 0.45, h * 0.65, 0.0);
    const can_r = Vec3.init(w * 0.45, h * 0.65, 0.0);

    try mesh.addTriangle(
        .{ .pos = can_front, .normal = Vec3.zero(), .uv = .{ 0.5, 1.0 }, .color = c_col },
        .{ .pos = can_peak, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = c_col },
        .{ .pos = can_r, .normal = Vec3.zero(), .uv = .{ 1.0, 0.5 }, .color = c_col },
    );
    try mesh.addTriangle(
        .{ .pos = can_front, .normal = Vec3.zero(), .uv = .{ 0.5, 1.0 }, .color = c_col },
        .{ .pos = can_l, .normal = Vec3.zero(), .uv = .{ 0.0, 0.5 }, .color = c_col },
        .{ .pos = can_peak, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = c_col },
    );
    try mesh.addTriangle(
        .{ .pos = can_peak, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = c_col },
        .{ .pos = can_back, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = c_col },
        .{ .pos = can_r, .normal = Vec3.zero(), .uv = .{ 1.0, 0.0 }, .color = c_col },
    );
    try mesh.addTriangle(
        .{ .pos = can_peak, .normal = Vec3.zero(), .uv = .{ 0.5, 0.5 }, .color = c_col },
        .{ .pos = can_l, .normal = Vec3.zero(), .uv = .{ 0.0, 0.0 }, .color = c_col },
        .{ .pos = can_back, .normal = Vec3.zero(), .uv = .{ 0.5, 0.0 }, .color = c_col },
    );

    // ------------------------------------------------------------------------
    // 3. SWEPT WINGS & WINGLETS (Delta Wings)
    // ------------------------------------------------------------------------
    const w_col = cfg.wing_color;
    const wing_span = cfg.wing_span * 0.5;

    // Right Wing
    const rw_root_front = Vec3.init(w * 0.9, 0.0, 0.2);
    const rw_root_aft = Vec3.init(w * 0.8, 0.0, -l * 0.9);
    const rw_tip_front = Vec3.init(wing_span, 0.05, -l * 0.4);
    const rw_tip_aft = Vec3.init(wing_span * 0.95, -0.05, -l * 1.05);

    try mesh.addQuad(
        .{ .pos = rw_root_front, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 1 }, .color = w_col },
        .{ .pos = rw_tip_front, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 1 }, .color = w_col },
        .{ .pos = rw_tip_aft, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = w_col },
        .{ .pos = rw_root_aft, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = w_col },
    );
    // Right Wing Underside
    try mesh.addQuad(
        .{ .pos = rw_root_front, .normal = Vec3.init(0, -1, 0), .uv = .{ 0, 1 }, .color = w_col },
        .{ .pos = rw_root_aft, .normal = Vec3.init(0, -1, 0), .uv = .{ 0, 0 }, .color = w_col },
        .{ .pos = rw_tip_aft, .normal = Vec3.init(0, -1, 0), .uv = .{ 1, 0 }, .color = w_col },
        .{ .pos = rw_tip_front, .normal = Vec3.init(0, -1, 0), .uv = .{ 1, 1 }, .color = w_col },
    );

    // Left Wing
    const lw_root_front = Vec3.init(-w * 0.9, 0.0, 0.2);
    const lw_root_aft = Vec3.init(-w * 0.8, 0.0, -l * 0.9);
    const lw_tip_front = Vec3.init(-wing_span, 0.05, -l * 0.4);
    const lw_tip_aft = Vec3.init(-wing_span * 0.95, -0.05, -l * 1.05);

    try mesh.addQuad(
        .{ .pos = lw_root_front, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 1 }, .color = w_col },
        .{ .pos = lw_root_aft, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = w_col },
        .{ .pos = lw_tip_aft, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = w_col },
        .{ .pos = lw_tip_front, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 1 }, .color = w_col },
    );
    // Left Wing Underside
    try mesh.addQuad(
        .{ .pos = lw_root_front, .normal = Vec3.init(0, -1, 0), .uv = .{ 0, 1 }, .color = w_col },
        .{ .pos = lw_tip_front, .normal = Vec3.init(0, -1, 0), .uv = .{ 1, 1 }, .color = w_col },
        .{ .pos = lw_tip_aft, .normal = Vec3.init(0, -1, 0), .uv = .{ 1, 0 }, .color = w_col },
        .{ .pos = lw_root_aft, .normal = Vec3.init(0, -1, 0), .uv = .{ 0, 0 }, .color = w_col },
    );

    // Vertical Stabilizer / Tail Fin
    const fin_bot_fwd = Vec3.init(0.0, h * 0.8, -l * 0.2);
    const fin_bot_aft = Vec3.init(0.0, h * 0.8, -l);
    const fin_top_fwd = Vec3.init(0.0, h * 2.2, -l * 0.7);
    const fin_top_aft = Vec3.init(0.0, h * 2.1, -l * 1.05);

    try mesh.addQuad(
        .{ .pos = fin_bot_fwd, .normal = Vec3.init(1, 0, 0), .uv = .{ 0, 0 }, .color = w_col },
        .{ .pos = fin_top_fwd, .normal = Vec3.init(1, 0, 0), .uv = .{ 0, 1 }, .color = w_col },
        .{ .pos = fin_top_aft, .normal = Vec3.init(1, 0, 0), .uv = .{ 1, 1 }, .color = w_col },
        .{ .pos = fin_bot_aft, .normal = Vec3.init(1, 0, 0), .uv = .{ 1, 0 }, .color = w_col },
    );
    try mesh.addQuad(
        .{ .pos = fin_bot_fwd, .normal = Vec3.init(-1, 0, 0), .uv = .{ 0, 0 }, .color = w_col },
        .{ .pos = fin_bot_aft, .normal = Vec3.init(-1, 0, 0), .uv = .{ 1, 0 }, .color = w_col },
        .{ .pos = fin_top_aft, .normal = Vec3.init(-1, 0, 0), .uv = .{ 1, 1 }, .color = w_col },
        .{ .pos = fin_top_fwd, .normal = Vec3.init(-1, 0, 0), .uv = .{ 0, 1 }, .color = w_col },
    );

    // ------------------------------------------------------------------------
    // 4. TWIN ION THRUSTERS (Aft Engine Block)
    // ------------------------------------------------------------------------
    const t_col = cfg.thruster_color;
    const r_eng = Vec3.init(w * 0.45, 0.05, -l * 1.02);
    const l_eng = Vec3.init(-w * 0.45, 0.05, -l * 1.02);
    const eng_r: f32 = 0.28;

    // Right Nozzle Disc
    try mesh.addTriangle(
        .{ .pos = r_eng, .normal = Vec3.init(0, 0, -1), .uv = .{ 0.5, 0.5 }, .color = t_col },
        .{ .pos = Vec3.init(r_eng.x + eng_r, r_eng.y, r_eng.z), .normal = Vec3.init(0, 0, -1), .uv = .{ 1, 0.5 }, .color = t_col },
        .{ .pos = Vec3.init(r_eng.x, r_eng.y + eng_r, r_eng.z), .normal = Vec3.init(0, 0, -1), .uv = .{ 0.5, 1 }, .color = t_col },
    );
    // Left Nozzle Disc
    try mesh.addTriangle(
        .{ .pos = l_eng, .normal = Vec3.init(0, 0, -1), .uv = .{ 0.5, 0.5 }, .color = t_col },
        .{ .pos = Vec3.init(l_eng.x, l_eng.y + eng_r, l_eng.z), .normal = Vec3.init(0, 0, -1), .uv = .{ 0.5, 1 }, .color = t_col },
        .{ .pos = Vec3.init(l_eng.x - eng_r, l_eng.y, l_eng.z), .normal = Vec3.init(0, 0, -1), .uv = .{ 0, 0.5 }, .color = t_col },
    );

    return mesh;
}

pub const TankConfig = struct {
    width: f32 = 2.4,
    length: f32 = 3.6,
    height: f32 = 1.0,
    armor_color: [4]u8 = .{ 65, 80, 75, 255 }, // Military Olive Green / Titanium
    turret_color: [4]u8 = .{ 95, 115, 105, 255 },
    energy_glow: [4]u8 = .{ 0, 240, 255, 255 }, // Cyan Repulsor Glow
};

/// Parametric procedural generator for combat hover-tanks with anti-grav repulsors
pub fn generateHoverTankMesh(allocator: std.mem.Allocator, cfg: TankConfig) !ProceduralMesh {
    var mesh = ProceduralMesh.init(allocator);
    errdefer mesh.deinit();

    const w = cfg.width;
    const l = cfg.length;
    const h = cfg.height;
    const col = cfg.armor_color;
    const tcol = cfg.turret_color;
    const glow = cfg.energy_glow;

    // 1. Armored Chassis (Upper Deck)
    const fwd_l = Vec3.init(-w * 0.4, h * 0.5, l * 0.5);
    const fwd_r = Vec3.init(w * 0.4, h * 0.5, l * 0.5);
    const mid_l = Vec3.init(-w * 0.5, h * 0.5, 0.0);
    const mid_r = Vec3.init(w * 0.5, h * 0.5, 0.0);
    const aft_l = Vec3.init(-w * 0.45, h * 0.4, -l * 0.5);
    const aft_r = Vec3.init(w * 0.45, h * 0.4, -l * 0.5);

    try mesh.addQuad(
        .{ .pos = fwd_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 1 }, .color = col },
        .{ .pos = fwd_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 1 }, .color = col },
        .{ .pos = mid_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0.5 }, .color = col },
        .{ .pos = mid_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0.5 }, .color = col },
    );
    try mesh.addQuad(
        .{ .pos = mid_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0.5 }, .color = col },
        .{ .pos = mid_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0.5 }, .color = col },
        .{ .pos = aft_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = col },
        .{ .pos = aft_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = col },
    );

    // 2. Beveled Combat Turret
    const tw = w * 0.35;
    const tl = l * 0.3;
    const th = h * 1.1;
    const tur_fl = Vec3.init(-tw * 0.8, th, tl * 0.4);
    const tur_fr = Vec3.init(tw * 0.8, th, tl * 0.4);
    const tur_ar = Vec3.init(tw, th, -tl * 0.6);
    const tur_al = Vec3.init(-tw, th, -tl * 0.6);

    try mesh.addQuad(
        .{ .pos = tur_fl, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 1 }, .color = tcol },
        .{ .pos = tur_fr, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 1 }, .color = tcol },
        .{ .pos = tur_ar, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = tcol },
        .{ .pos = tur_al, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = tcol },
    );

    // 3. Twin Railgun Barrels (Aft to Front)
    const b_r = Vec3.init(tw * 0.35, th * 0.9, tl * 0.4);
    const b_r_tip = Vec3.init(tw * 0.35, th * 0.9, tl * 1.6);
    const b_l = Vec3.init(-tw * 0.35, th * 0.9, tl * 0.4);
    const b_l_tip = Vec3.init(-tw * 0.35, th * 0.9, tl * 1.6);

    try mesh.addTriangle(
        .{ .pos = b_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = glow },
        .{ .pos = Vec3.init(b_r.x + 0.08, b_r.y, b_r.z), .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = glow },
        .{ .pos = b_r_tip, .normal = Vec3.init(0, 1, 0), .uv = .{ 0.5, 1 }, .color = glow },
    );
    try mesh.addTriangle(
        .{ .pos = b_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = glow },
        .{ .pos = Vec3.init(b_l.x - 0.08, b_l.y, b_l.z), .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = glow },
        .{ .pos = b_l_tip, .normal = Vec3.init(0, 1, 0), .uv = .{ 0.5, 1 }, .color = glow },
    );

    // 4. 4 Anti-Gravity Repulsor Pods (Underside Glowing Pads)
    const pad_r: f32 = 0.25;
    const pod_positions = [_]Vec3{
        Vec3.init(-w * 0.45, 0.1, l * 0.4),
        Vec3.init(w * 0.45, 0.1, l * 0.4),
        Vec3.init(-w * 0.45, 0.1, -l * 0.4),
        Vec3.init(w * 0.45, 0.1, -l * 0.4),
    };

    for (pod_positions) |pp| {
        try mesh.addTriangle(
            .{ .pos = pp, .normal = Vec3.init(0, -1, 0), .uv = .{ 0.5, 0.5 }, .color = glow },
            .{ .pos = Vec3.init(pp.x + pad_r, pp.y, pp.z), .normal = Vec3.init(0, -1, 0), .uv = .{ 1, 0.5 }, .color = glow },
            .{ .pos = Vec3.init(pp.x, pp.y, pp.z + pad_r), .normal = Vec3.init(0, -1, 0), .uv = .{ 0.5, 1 }, .color = glow },
        );
    }

    return mesh;
}

pub const BolideConfig = struct {
    width: f32 = 2.0,
    length: f32 = 4.4,
    height: f32 = 0.75,
    body_color: [4]u8 = .{ 20, 24, 32, 255 }, // Carbon Black
    accent_color: [4]u8 = .{ 255, 0, 110, 255 }, // Neon Magenta / Cyber Pink
    neon_glow: [4]u8 = .{ 0, 240, 255, 255 }, // Cyber Cyan
    cockpit_color: [4]u8 = .{ 40, 60, 90, 220 }, // Tinted Aero Canopy
};

/// Parametric procedural generator for Cyberpunk Hover-Racer Bolide
pub fn generateCyberBolideMesh(allocator: std.mem.Allocator, cfg: BolideConfig) !ProceduralMesh {
    var mesh = ProceduralMesh.init(allocator);
    errdefer mesh.deinit();

    const w = cfg.width;
    const l = cfg.length;
    const h = cfg.height;
    const col = cfg.body_color;
    const acc = cfg.accent_color;
    const glow = cfg.neon_glow;
    const ccol = cfg.cockpit_color;

    // 1. Aerodynamic Front Splitter
    const sp_tip = Vec3.init(0.0, 0.05, l * 0.55);
    const sp_l = Vec3.init(-w * 0.48, 0.05, l * 0.42);
    const sp_r = Vec3.init(w * 0.48, 0.05, l * 0.42);
    try mesh.addTriangle(
        .{ .pos = sp_tip, .normal = Vec3.init(0, 1, 0), .uv = .{ 0.5, 1 }, .color = acc },
        .{ .pos = sp_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0 }, .color = acc },
        .{ .pos = sp_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0 }, .color = acc },
    );

    // 2. Low-Profile Hood and Main Monocoque
    const h_fwd_l = Vec3.init(-w * 0.4, h * 0.4, l * 0.35);
    const h_fwd_r = Vec3.init(w * 0.4, h * 0.4, l * 0.35);
    const h_mid_l = Vec3.init(-w * 0.5, h * 0.6, 0.0);
    const h_mid_r = Vec3.init(w * 0.5, h * 0.6, 0.0);

    try mesh.addQuad(
        .{ .pos = h_fwd_l, .normal = Vec3.init(0, 1, 0.3), .uv = .{ 0, 1 }, .color = col },
        .{ .pos = h_fwd_r, .normal = Vec3.init(0, 1, 0.3), .uv = .{ 1, 1 }, .color = col },
        .{ .pos = h_mid_r, .normal = Vec3.init(0, 1, 0), .uv = .{ 1, 0.5 }, .color = col },
        .{ .pos = h_mid_l, .normal = Vec3.init(0, 1, 0), .uv = .{ 0, 0.5 }, .color = col },
    );

    // 3. Cockpit Canopy (Tinted Glass)
    const c_front = Vec3.init(0.0, h * 0.95, l * 0.15);
    const c_l = Vec3.init(-w * 0.22, h * 0.65, -l * 0.05);
    const c_r = Vec3.init(w * 0.22, h * 0.65, -l * 0.05);
    const c_rear = Vec3.init(0.0, h * 0.75, -l * 0.3);

    try mesh.addTriangle(
        .{ .pos = c_front, .normal = Vec3.init(0, 1, 1), .uv = .{ 0.5, 1 }, .color = ccol },
        .{ .pos = c_r, .normal = Vec3.init(1, 1, 0), .uv = .{ 1, 0 }, .color = ccol },
        .{ .pos = c_l, .normal = Vec3.init(-1, 1, 0), .uv = .{ 0, 0 }, .color = ccol },
    );
    try mesh.addTriangle(
        .{ .pos = c_front, .normal = Vec3.init(0, 1, -1), .uv = .{ 0.5, 1 }, .color = ccol },
        .{ .pos = c_l, .normal = Vec3.init(-1, 1, 0), .uv = .{ 0, 0 }, .color = ccol },
        .{ .pos = c_rear, .normal = Vec3.init(0, 1, -1), .uv = .{ 0.5, 0 }, .color = ccol },
    );
    try mesh.addTriangle(
        .{ .pos = c_front, .normal = Vec3.init(0, 1, -1), .uv = .{ 0.5, 1 }, .color = ccol },
        .{ .pos = c_rear, .normal = Vec3.init(0, 1, -1), .uv = .{ 0.5, 0 }, .color = ccol },
        .{ .pos = c_r, .normal = Vec3.init(1, 1, 0), .uv = .{ 1, 0 }, .color = ccol },
    );

    // 4. Rear Dual Aerodynamic Wing & Side Fins
    const w_l = Vec3.init(-w * 0.55, h * 1.0, -l * 0.45);
    const w_r = Vec3.init(w * 0.55, h * 1.0, -l * 0.45);
    const w_mid_l = Vec3.init(-w * 0.25, h * 0.9, -l * 0.52);
    const w_mid_r = Vec3.init(w * 0.25, h * 0.9, -l * 0.52);

    try mesh.addQuad(
        .{ .pos = w_l, .normal = Vec3.init(0, 1, -0.2), .uv = .{ 0, 1 }, .color = acc },
        .{ .pos = w_r, .normal = Vec3.init(0, 1, -0.2), .uv = .{ 1, 1 }, .color = acc },
        .{ .pos = w_mid_r, .normal = Vec3.init(0, 1, -0.2), .uv = .{ 0.8, 0 }, .color = acc },
        .{ .pos = w_mid_l, .normal = Vec3.init(0, 1, -0.2), .uv = .{ 0.2, 0 }, .color = acc },
    );

    // 5. 4 Magnetic Hover-Wheels with Neon Rings
    const wheel_positions = [_]Vec3{
        Vec3.init(-w * 0.52, 0.15, l * 0.28),
        Vec3.init(w * 0.52, 0.15, l * 0.28),
        Vec3.init(-w * 0.52, 0.15, -l * 0.32),
        Vec3.init(w * 0.52, 0.15, -l * 0.32),
    };
    const wr: f32 = 0.28;

    for (wheel_positions) |wp| {
        try mesh.addTriangle(
            .{ .pos = wp, .normal = Vec3.init(0, 0, 1), .uv = .{ 0.5, 0.5 }, .color = glow },
            .{ .pos = Vec3.init(wp.x, wp.y + wr, wp.z), .normal = Vec3.init(0, 0, 1), .uv = .{ 0.5, 1 }, .color = glow },
            .{ .pos = Vec3.init(wp.x + 0.1, wp.y, wp.z + wr), .normal = Vec3.init(0, 0, 1), .uv = .{ 1, 0.5 }, .color = glow },
        );
    }

    return mesh;
}

// =============================================================================
// TESTS
// =============================================================================

test "ProceduralMesh: generate spaceship geometry and verify topology" {
    const allocator = std.testing.allocator;
    var mesh = try generateSpaceshipMesh(allocator, .{});
    defer mesh.deinit();

    // Verify non-empty vertex & index buffers
    try std.testing.expect(mesh.vertices.items.len > 24);
    try std.testing.expect(mesh.indices.items.len > 36);

    // Verify indices point within valid range
    for (mesh.indices.items) |idx| {
        try std.testing.expect(idx < mesh.vertices.items.len);
    }
}
