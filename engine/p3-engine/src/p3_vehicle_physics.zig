// =============================================================================
// P³ ENGINE — VEHICLE PHYSICS MODULE (Pacejka tire + suspension + transmission)
// =============================================================================
//
// Native P³ implementation of vehicle physics, architecturally inspired by
// Unreal Engine's ChaosVehicles (FWheelStatus, FVehicleTransmissionConfig,
// EVehicleDifferential, ChaosVehicleWheel config) but WITHOUT copying Epic's
// C++ — uses our own P³ projective math and Zig-native idioms.
//
// Why this module exists:
//   Games (GTA V, RDR 2, Genshin Impact, Forza, etc.) need REAL vehicle
//   physics: suspension, tire friction (Pacejka Magic Formula), differential,
//   transmission (gear ratios), engine RPM curves, ABS, traction control.
//   Other engines implement this as a "crutch" on top of R³. We implement it
//   natively in P³ — same algorithms (public domain — math is not copyrightable),
//   no workarounds.
//
// All algorithms here are PUBLIC DOMAIN (textbook formulas):
//   - Pacejka Magic Tire Formula: Hans B. Pacejka, "Tire and Vehicle Dynamics"
//     (the formula itself is public, only specific manufacturer coefficients
//     are proprietary). The standard form with normalized slip ratio is
//     published in countless academic papers.
//   - Hooke's law for suspension springs: F = -k*x - c*v (centuries old)
//   - Rigid body integration (semi-implicit Euler): textbook classical mechanics
//   - Differential types (open/limited-slip/torque-vectoring): automotive engineering
//
// References (publicly available):
//   - UE ChaosVehicleWheel.h, ChaosWheeledVehicleMovementComponent.h: structural
//     patterns (WheelConfig, FWheelStatus, EVehicleDifferential)
//   - Pacejka 2012 book: tire force formulas
//   - Game Physics Engine Development (Ian Millington): rigid body integration
//
// All math uses p3.Vec3 / p3.Mat4x4 (P³ projective types).
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;
const Quaternion = p3.Quaternion;

// ---------------------------------------------------------------------------
// Differential types (matches UE's EVehicleDifferential)
// ---------------------------------------------------------------------------
pub const DifferentialType = enum(u8) {
    undefined = 0,
    all_wheel_drive,    // torque distributed equally front/rear
    front_wheel_drive,  // only front axle
    rear_wheel_drive,   // only rear axle
};

// ---------------------------------------------------------------------------
// Wheel configuration (matches UE's UChaosVehicleWheel config fields)
// ---------------------------------------------------------------------------
pub const WheelConfig = struct {
    /// Position offset relative to vehicle origin (in vehicle-local space)
    offset: Vec3,
    /// Radius of the wheel (meters)
    wheel_radius: f32 = 0.35,
    /// Width of the wheel (meters)
    wheel_width: f32 = 0.20,
    /// Mass of the wheel (kg)
    wheel_mass: f32 = 20.0,
    /// Cornering stiffness (Pacejka parameter BCD) — higher = grippier in corners
    cornering_stiffness: f32 = 25000.0,
    /// Friction force multiplier (per-wheel scaling)
    friction_multiplier: f32 = 2.0,
    /// Lateral skid grip loss (0..1, lower = less grip on skid)
    side_slip_modifier: f32 = 1.0,
    /// Slip ratio threshold above which ABS kicks in (0..1)
    slip_threshold: f32 = 0.10,
    /// Lateral skid threshold above which traction control kicks in (rad)
    skid_threshold: f32 = 0.20,
    /// Steering angle (degrees, +left, -right) — controlled per-frame
    steering_angle_deg: f32 = 0.0,
    /// Engine torque applied to this wheel (Nm) — controlled per-frame
    drive_torque: f32 = 0.0,
    /// Brake torque applied to this wheel (Nm) — controlled per-frame
    brake_torque: f32 = 0.0,
    /// Handbrake torque (Nm, typically only rear axle)
    handbrake_torque: f32 = 0.0,
    /// Is ABS enabled?
    abs_enabled: bool = true,
    /// Is traction control enabled?
    traction_control_enabled: bool = true,
    /// Suspension spring constant (N/m, Hooke's law F = -k*x)
    suspension_spring_k: f32 = 250000.0,
    /// Suspension damping coefficient (N·s/m, F = -c*v)
    suspension_damping_c: f32 = 5000.0,
    /// Suspension max raise (m, positive up from rest)
    suspension_max_raise: f32 = 0.15,
    /// Suspension max drop (m, negative down from rest)
    suspension_max_drop: f32 = -0.15,
    /// Rest length of suspension (m)
    suspension_rest_length: f32 = 0.40,
    /// Which axle? (front/rear)
    axle: enum(u8) { front, rear } = .front,
};

// ---------------------------------------------------------------------------
// Wheel runtime state (matches UE's FWheelStatus)
// ---------------------------------------------------------------------------
pub const WheelState = struct {
    /// Is the wheel currently in contact with the ground?
    in_contact: bool = false,
    /// Contact point in world space (from raycast)
    contact_point: Vec3 = Vec3.zero(),
    /// Hit normal at contact
    contact_normal: Vec3 = Vec3.init(0, 1, 0),
    /// Normalized suspension length (0 = fully compressed, 1 = fully extended)
    normalized_suspension_length: f32 = 1.0,
    /// Spring force currently applied (N)
    spring_force: f32 = 0.0,
    /// Damping force currently applied (N)
    damping_force: f32 = 0.0,
    /// Slip angle (rad) — difference between wheel heading and actual velocity direction
    slip_angle: f32 = 0.0,
    /// Slip ratio (dimensionless, -inf..+inf, typical range -0.3..0.3)
    slip_ratio: f32 = 0.0,
    /// Is the wheel slipping longitudinally (excessive wheelspin)?
    is_slipping: bool = false,
    /// Slip magnitude (|slip_ratio| when slipping)
    slip_magnitude: f32 = 0.0,
    /// Is the wheel skidding laterally (excessive drift)?
    is_skidding: bool = false,
    /// Skid magnitude (|slip_angle| when skidding)
    skid_magnitude: f32 = 0.0,
    /// Current wheel angular velocity (rad/s)
    angular_velocity: f32 = 0.0,
    /// Longitudinal force on the wheel (N) — from Pacejka tire model
    longitudinal_force: f32 = 0.0,
    /// Lateral force on the wheel (N) — from Pacejka tire model
    lateral_force: f32 = 0.0,
    /// Surface friction coefficient (from contacted material, default 1.0)
    surface_friction: f32 = 1.0,
    /// ABS active this frame?
    abs_activated: bool = false,
    /// Traction control active this frame?
    traction_control_activated: bool = false,
};

// ---------------------------------------------------------------------------
// Engine configuration (matches UE's FVehicleEngineConfig)
// ---------------------------------------------------------------------------
pub const EngineConfig = struct {
    /// Max engine torque (Nm) at peak RPM
    max_torque: f32 = 600.0,
    /// Max RPM (redline)
    max_rpm: f32 = 7000.0,
    /// Idle RPM
    idle_rpm: f32 = 800.0,
    /// Engine braking torque (Nm, when no throttle)
    engine_brake_torque: f32 = 80.0,
    /// RPM at which torque peaks (used for torque curve)
    peak_torque_rpm: f32 = 4000.0,
    /// Moment of inertia of engine (kg·m²) — affects how quickly RPM changes
    moi: f32 = 0.5,
};

// ---------------------------------------------------------------------------
// Transmission / gearbox configuration (matches UE's FVehicleTransmissionConfig)
// ---------------------------------------------------------------------------
pub const TransmissionConfig = struct {
    /// Use automatic transmission (vs manual)
    use_automatic_gears: bool = true,
    /// Auto reverse when stationary and throttle applied?
    use_auto_reverse: bool = true,
    /// Final drive ratio (multiplier on all gear ratios)
    final_ratio: f32 = 3.08,
    /// Forward gear ratios (e.g., [2.85, 2.02, 1.35, 1.0] for 4-speed)
    forward_gear_ratios: []const f32 = &[_]f32{ 2.85, 2.02, 1.35, 1.0 },
    /// Reverse gear ratio(s)
    reverse_gear_ratios: []const f32 = &[_]f32{2.86},
    /// RPM above which gear upshifts (auto)
    change_up_rpm: f32 = 4500.0,
    /// RPM below which gear downshifts (auto)
    change_down_rpm: f32 = 2000.0,
    /// Time to shift gear (s, no power applied during shift)
    gear_change_time: f32 = 0.4,
    /// Mechanical efficiency of transmission (0..1)
    transmission_efficiency: f32 = 0.9,
};

// ---------------------------------------------------------------------------
// Vehicle configuration (the top-level config that gets passed around)
// ---------------------------------------------------------------------------
pub const VehicleConfig = struct {
    /// Total vehicle mass (kg)
    mass: f32 = 1500.0,
    /// Center of mass offset from body origin (m)
    center_of_mass_offset: Vec3 = Vec3.init(0, -0.2, 0),
    /// Moment of inertia tensor (kg·m²) — diagonal elements
    moi_diag: Vec3 = Vec3.init(800, 1200, 1400),
    /// Aerodynamic drag coefficient (Cd × frontal area, dimensionless)
    drag_coefficient: f32 = 0.35,
    /// Aerodynamic downforce coefficient (force per (m/s)²)
    downforce_coefficient: f32 = 0.0,
    /// Differential type
    differential: DifferentialType = .all_wheel_drive,
    /// Engine config
    engine: EngineConfig = .{},
    /// Transmission config
    transmission: TransmissionConfig = .{},
    /// Wheel configs (one per wheel)
    wheels: []const WheelConfig = &[_]WheelConfig{},
};

// ---------------------------------------------------------------------------
// Vehicle runtime state
// ---------------------------------------------------------------------------
pub const VehicleState = struct {
    /// Position in world (m)
    position: Vec3 = Vec3.zero(),
    /// Linear velocity (m/s)
    velocity: Vec3 = Vec3.zero(),
    /// Orientation quaternion
    orientation: Quaternion = Quaternion.fromAxisAngle(Vec3.init(0, 1, 0), 0),
    /// Angular velocity (rad/s)
    angular_velocity: Vec3 = Vec3.zero(),
    /// Current engine RPM (rev/min)
    engine_rpm: f32 = 800.0,
    /// Current gear (-1=reverse, 0=neutral, 1..N=forward gears)
    current_gear: i32 = 1,
    /// Time remaining for gear shift (s, 0 = not shifting)
    shift_timer: f32 = 0.0,
    /// Throttle input (0..1)
    throttle: f32 = 0.0,
    /// Brake input (0..1)
    brake: f32 = 0.0,
    /// Steering input (-1..+1, -1=full left, +1=full right)
    steering: f32 = 0.0,
    /// Handbrake input (0..1)
    handbrake: f32 = 0.0,
    /// Per-wheel runtime state
    wheels: []WheelState = &[_]WheelState{},

    pub fn init(allocator: std.mem.Allocator, num_wheels: usize) !VehicleState {
        const wheels = try allocator.alloc(WheelState, num_wheels);
        for (wheels) |*w| w.* = .{};
        return .{ .wheels = wheels };
    }

    pub fn deinit(self: *VehicleState, allocator: std.mem.Allocator) void {
        if (self.wheels.len > 0) allocator.free(self.wheels);
    }
};

// ---------------------------------------------------------------------------
// Pacejka Magic Tire Formula — the public-domain normalized form
// ---------------------------------------------------------------------------
//
//   F_long = D * sin(C * atan(B * slip_ratio - E * (B * slip_ratio - atan(B * slip_ratio))))
//   F_lat  = D * sin(C * atan(B * slip_angle - E * (B * slip_angle - atan(B * slip_angle))))
//
// D = peak force (N), C = shape factor (typically 1.65 for longitudinal, 1.3 for lateral)
// B = stiffness factor, E = curvature factor
//
// Reference: Hans B. Pacejka, "Tire and Vehicle Dynamics" (book, 2012, SAE)
// Formula itself is public domain (it's math). Manufacturer coefficients are not.
//
// We use standard passenger-car defaults from academic literature.
// Returns longitudinal/lateral force in Newtons.

pub const PacejkaParams = struct {
    /// D: peak force (N) — proportional to normal load * surface friction
    peak_factor: f32 = 1.0,
    /// C: shape factor — controls the "peakiness" of the curve
    shape_factor: f32 = 1.65,
    /// B: stiffness factor — controls slope at zero slip
    stiffness_factor: f32 = 10.0,
    /// E: curvature factor — controls asymptotic falloff
    curvature_factor: f32 = 0.97,
};

pub fn pacejkaLongitudinalForce(
    slip_ratio: f32,
    normal_load: f32,
    surface_friction: f32,
    params: PacejkaParams,
) f32 {
    // Longitudinal force vs slip ratio (combo slip)
    const D = params.peak_factor * normal_load * surface_friction;
    const C = params.shape_factor;
    const B = params.stiffness_factor;
    const E = params.curvature_factor;
    const Bsr = B * slip_ratio;
    const inner = Bsr - E * (Bsr - math.atan(Bsr));
    const force = D * math.sin(C * math.atan(inner));
    return force;
}

pub fn pacejkaLateralForce(
    slip_angle: f32,
    normal_load: f32,
    surface_friction: f32,
    params: PacejkaParams,
) f32 {
    // Same formula, slip_angle (rad) instead of slip_ratio
    const D = params.peak_factor * normal_load * surface_friction;
    const C = 1.3; // lateral shape factor is typically 1.3 (vs 1.65 longitudinal)
    const B = params.stiffness_factor * 0.5; // lateral stiffness typically half of longitudinal
    const E = params.curvature_factor;
    const Bsa = B * slip_angle;
    const inner = Bsa - E * (Bsa - math.atan(Bsa));
    const force = D * math.sin(C * math.atan(inner));
    return force;
}

// ---------------------------------------------------------------------------
// Suspension raycast (matches UE's suspension trace)
// ---------------------------------------------------------------------------
//
// In a full engine, this does a physics raycast against the world's
// collision geometry. In our simplified version, we raycast against
// an implicit ground plane (y=0) — useful for testing the suspension
// model. Replace with real raycast against track mesh for production.

pub const SuspensionRaycastResult = struct {
    hit: bool,
    contact_point: Vec3,
    contact_normal: Vec3,
    distance: f32, // distance from ray origin to contact
};

pub fn raycastGroundPlane(
    wheel_origin: Vec3,
    suspension_axis: Vec3, // should point downward (-normal)
    rest_length: f32,
    max_drop: f32,
) SuspensionRaycastResult {
    // Simple ground plane at y=0
    if (suspension_axis.y >= 0) {
        // Axis points up — ray will never hit ground
        return .{ .hit = false, .contact_point = Vec3.zero(), .contact_normal = Vec3.init(0, 1, 0), .distance = 0 };
    }
    const max_drop_safe = if (max_drop < 0) -max_drop else max_drop;
    const total_length = rest_length + max_drop_safe;
    // parametric: origin + t * axis; we want y = 0
    // origin.y + t * axis.y = 0 => t = -origin.y / axis.y
    if (suspension_axis.y == 0) return .{ .hit = false, .contact_point = Vec3.zero(), .contact_normal = Vec3.init(0, 1, 0), .distance = 0 };
    const t = -wheel_origin.y / suspension_axis.y;
    if (t < 0 or t > total_length) {
        return .{ .hit = false, .contact_point = Vec3.zero(), .contact_normal = Vec3.init(0, 1, 0), .distance = 0 };
    }
    const contact = Vec3.init(
        wheel_origin.x + t * suspension_axis.x,
        0,
        wheel_origin.z + t * suspension_axis.z,
    );
    return .{
        .hit = true,
        .contact_point = contact,
        .contact_normal = Vec3.init(0, 1, 0),
        .distance = t,
    };
}

// ---------------------------------------------------------------------------
// Compute wheel state from raycast + vehicle velocity
// ---------------------------------------------------------------------------
pub fn updateWheelState(
    wheel_idx: usize,
    config: WheelConfig,
    state: *VehicleState,
    ray_result: SuspensionRaycastResult,
    dt: f32,
) void {
    var ws = &state.wheels[wheel_idx];
    ws.in_contact = ray_result.hit;
    if (ray_result.hit) {
        ws.contact_point = ray_result.contact_point;
        ws.contact_normal = ray_result.contact_normal;
        // Normalized suspension length: 0 = fully compressed, 1 = fully extended
        const rest = config.suspension_rest_length;
        const total_len = rest + (if (config.suspension_max_drop < 0) -config.suspension_max_drop else config.suspension_max_drop);
        const compressed = total_len - ray_result.distance;
        ws.normalized_suspension_length = 1.0 - (compressed / total_len);
        // Spring force (Hooke): F = -k * (compression - rest)
        const compression = rest - ray_result.distance;
        ws.spring_force = config.suspension_spring_k * compression;
        // Damping force: F = -c * v (relative velocity along suspension axis)
        // Approximation: use vertical velocity of vehicle
        ws.damping_force = -config.suspension_damping_c * state.velocity.y;
        // Surface friction defaults
        ws.surface_friction = 1.0; // would be set by PhysMaterial lookup
    } else {
        ws.spring_force = 0;
        ws.damping_force = 0;
        ws.normalized_suspension_length = 1.0;
    }
    _ = dt;
}

// ---------------------------------------------------------------------------
// Update engine RPM based on throttle, current gear, wheel angular velocity
// ---------------------------------------------------------------------------
pub fn updateEngineRpm(
    state: *VehicleState,
    config: VehicleConfig,
    avg_wheel_angular_velocity: f32,
    dt: f32,
) void {
    // Wheel angular velocity → engine RPM via gear ratio
    const gear_idx: usize = if (state.current_gear > 0) @intCast(state.current_gear - 1) else 0;
    const gear_ratios = if (state.current_gear > 0) config.transmission.forward_gear_ratios else config.transmission.reverse_gear_ratios;
    if (gear_idx >= gear_ratios.len) return;
    const ratio = gear_ratios[gear_idx] * config.transmission.final_ratio;
    const wheel_rpm = avg_wheel_angular_velocity * 60.0 / (2.0 * math.pi);
    const target_rpm = wheel_rpm * ratio;
    // Smooth toward target RPM based on throttle
    const idle = config.engine.idle_rpm;
    const throttle_rpm = idle + state.throttle * (config.engine.max_rpm - idle);
    const desired_rpm = if (state.throttle > 0.01) @max(throttle_rpm, target_rpm) else @max(idle, target_rpm);
    // Simple low-pass
    const lerp: f32 = 0.05; // 5% per dt = ~50ms time constant
    state.engine_rpm += (desired_rpm - state.engine_rpm) * lerp;
    // Clamp
    state.engine_rpm = @max(idle, @min(config.engine.max_rpm, state.engine_rpm));
    _ = dt;
}

// ---------------------------------------------------------------------------
// Automatic transmission: shift up/down based on RPM
// ---------------------------------------------------------------------------
pub fn updateTransmission(
    state: *VehicleState,
    config: VehicleConfig,
    dt: f32,
) void {
    if (!config.transmission.use_automatic_gears) return;
    if (state.shift_timer > 0) {
        state.shift_timer -= dt;
        return;
    }
    const num_forward = config.transmission.forward_gear_ratios.len;
    if (state.engine_rpm > config.transmission.change_up_rpm and state.current_gear > 0 and state.current_gear < @as(i32, @intCast(num_forward))) {
        state.current_gear += 1;
        state.shift_timer = config.transmission.gear_change_time;
    } else if (state.engine_rpm < config.transmission.change_down_rpm and state.current_gear > 1) {
        state.current_gear -= 1;
        state.shift_timer = config.transmission.gear_change_time;
    }
}

// ---------------------------------------------------------------------------
// Apply forces to vehicle body (semi-implicit Euler integration)
// ---------------------------------------------------------------------------
pub fn integrateVehicle(
    state: *VehicleState,
    config: VehicleConfig,
    dt: f32,
) void {
    // Sum forces on vehicle
    var total_force = Vec3.zero();
    var total_torque = Vec3.zero();
    // Gravity
    total_force = total_force.add(Vec3.init(0, -9.81 * config.mass, 0));
    // Aerodynamic drag: F = -0.5 * rho * Cd * A * v^2 * v_hat
    // Simplified: F = -drag_coeff * v * |v|
    const speed = state.velocity.length();
    if (speed > 0.01) {
        const drag_mag = config.drag_coefficient * speed * speed;
        const drag_dir = state.velocity.scale(-1.0 / speed);
        total_force = total_force.add(drag_dir.scale(drag_mag));
    }
    // Per-wheel forces: spring, damping, tire (long + lat)
    var drive_force: f32 = 0;
    var i: usize = 0;
    while (i < state.wheels.len) : (i += 1) {
        const wc = config.wheels[i];
        const ws = state.wheels[i];
        if (!ws.in_contact) continue;
        // Spring + damping along suspension axis (assume -Y for now)
        const suspension_force = ws.spring_force + ws.damping_force;
        total_force = total_force.add(Vec3.init(0, suspension_force, 0));
        // Tire longitudinal force (forward direction)
        const forward_dir = state.orientation.rotateVec(Vec3.init(0, 0, 1));
        total_force = total_force.add(forward_dir.scale(ws.longitudinal_force));
        // Tire lateral force (right direction)
        const right_dir = state.orientation.rotateVec(Vec3.init(1, 0, 0));
        total_force = total_force.add(right_dir.scale(ws.lateral_force));
        // Torque from lateral force on steering wheels (yaw)
        if (wc.steering_angle_deg != 0) {
            // Simplified: lateral force at front creates yaw torque
            const offset_x = wc.offset.x;
            const offset_z = wc.offset.z;
            total_torque = total_torque.add(Vec3.init(0, ws.lateral_force * offset_z, 0));
            _ = offset_x;
        }
        drive_force += ws.longitudinal_force;
    }
    // Integrate linear velocity (semi-implicit Euler)
    const accel = total_force.scale(1.0 / config.mass);
    state.velocity = state.velocity.add(accel.scale(dt));
    state.position = state.position.add(state.velocity.scale(dt));
    // Integrate angular velocity (simplified — assume diagonal MoI)
    const ang_accel = Vec3.init(
        total_torque.x / config.moi_diag.x,
        total_torque.y / config.moi_diag.y,
        total_torque.z / config.moi_diag.z,
    );
    state.angular_velocity = state.angular_velocity.add(ang_accel.scale(dt));
    // Update orientation (small-angle approximation for stability)
    const ang = state.angular_velocity.scale(dt);
    const ang_mag = ang.length();
    if (ang_mag > 0.0001) {
        const axis = ang.scale(1.0 / ang_mag);
        const delta_q = Quaternion.fromAxisAngle(axis, ang_mag);
        state.orientation = delta_q.mul(state.orientation).normalize();
    }
}

// ---------------------------------------------------------------------------
// Compute slip ratio and slip angle from wheel velocity
// ---------------------------------------------------------------------------
//
// slip_ratio = (wheel_angular_velocity * wheel_radius - vehicle_forward_speed) / |vehicle_forward_speed|
// slip_angle = atan2(lateral_velocity, forward_velocity) - steering_angle
//
// These are the inputs to Pacejka. The output is the tire force vector.

pub fn computeWheelSlip(
    wheel_idx: usize,
    config: WheelConfig,
    state: *VehicleState,
    forward_speed: f32,
    lateral_speed: f32,
) struct { slip_ratio: f32, slip_angle: f32 } {
    var ws = &state.wheels[wheel_idx];
    const wheel_radius = config.wheel_radius;
    const wheel_long_speed = ws.angular_velocity * wheel_radius;
    // Slip ratio (avoid division by zero)
    var slip_ratio: f32 = 0;
    if (@abs(forward_speed) > 0.5) {
        slip_ratio = (wheel_long_speed - forward_speed) / @abs(forward_speed);
    } else if (@abs(wheel_long_speed) > 0.5) {
        slip_ratio = wheel_long_speed / 5.0; // arbitrary normalization at low speed
    }
    // Slip angle (rad)
    const steer_rad = config.steering_angle_deg * math.pi / 180.0;
    var slip_angle: f32 = 0;
    if (@abs(forward_speed) > 0.5) {
        slip_angle = math.atan2(lateral_speed, forward_speed) - steer_rad;
    }
    // Update slipping/skidding flags
    ws.is_slipping = @abs(slip_ratio) > config.slip_threshold;
    ws.slip_magnitude = @abs(slip_ratio);
    ws.is_skidding = @abs(slip_angle) > config.skid_threshold;
    ws.skid_magnitude = @abs(slip_angle);
    ws.slip_ratio = slip_ratio;
    ws.slip_angle = slip_angle;
    return .{ .slip_ratio = slip_ratio, .slip_angle = slip_angle };
}

// ---------------------------------------------------------------------------
// Apply tire forces (Pacejka) to wheel based on slip
// ---------------------------------------------------------------------------
pub fn applyTireForces(
    wheel_idx: usize,
    config: WheelConfig,
    state: *VehicleState,
    slip: struct { slip_ratio: f32, slip_angle: f32 },
    normal_load: f32,
) void {
    var ws = &state.wheels[wheel_idx];
    // Longitudinal force from Pacejka (slip_ratio → F_long)
    ws.longitudinal_force = pacejkaLongitudinalForce(
        slip.slip_ratio,
        normal_load,
        ws.surface_friction * config.friction_multiplier,
        .{ .stiffness_factor = config.cornering_stiffness / normal_load },
    );
    // Lateral force from Pacejka (slip_angle → F_lat)
    ws.lateral_force = pacejkaLateralForce(
        slip.slip_angle,
        normal_load,
        ws.surface_friction * config.friction_multiplier * config.side_slip_modifier,
        .{ .stiffness_factor = config.cornering_stiffness / normal_load },
    );
    // ABS: if slip_ratio exceeds threshold, reduce drive torque
    if (config.abs_enabled and @abs(slip.slip_ratio) > config.slip_threshold) {
        ws.abs_activated = true;
        ws.longitudinal_force *= 0.5; // crude ABS: cut force in half
    }
    // Traction control: if slipping longitudinally, reduce torque
    if (config.traction_control_enabled and ws.is_slipping) {
        ws.traction_control_activated = true;
        ws.longitudinal_force *= 0.7;
    }
}

// ---------------------------------------------------------------------------
// Compute drive torque from engine (through transmission + differential)
// ---------------------------------------------------------------------------
pub fn computeDriveTorque(
    state: *VehicleState,
    config: VehicleConfig,
    wheel_idx: usize,
) f32 {
    if (state.shift_timer > 0) return 0; // no power during shift
    const wc = config.wheels[wheel_idx];
    // Engine torque at current RPM (use max torque curve approximation)
    const rpm_norm = state.engine_rpm / config.engine.max_rpm;
    const rpm_factor = 1.0 - 0.3 * @abs(rpm_norm - 0.6); // peak around 60% redline
    const engine_torque = config.engine.max_torque * rpm_factor * state.throttle;
    // Apply brake torque (opposes motion)
    const brake_torque = -config.engine.engine_brake_torque * state.brake;
    // Apply handbrake torque (rear wheels only)
    var handbrake_torque: f32 = 0;
    if (wc.axle == .rear) {
        handbrake_torque = -state.handbrake * 1500; // 1500 Nm max handbrake
    }
    // Gear ratio
    const gear_idx: usize = if (state.current_gear > 0) @intCast(state.current_gear - 1) else 0;
    const gear_ratios = if (state.current_gear > 0) config.transmission.forward_gear_ratios else config.transmission.reverse_gear_ratios;
    if (gear_idx >= gear_ratios.len) return 0;
    const ratio = gear_ratios[gear_idx] * config.transmission.final_ratio * config.transmission.transmission_efficiency;
    // Differential: distribute torque to wheels based on type
    var torque_split: f32 = 0;
    switch (config.differential) {
        .all_wheel_drive => torque_split = 0.25, // equal split for 4 wheels
        .front_wheel_drive => torque_split = if (wc.axle == .front) 0.5 else 0,
        .rear_wheel_drive => torque_split = if (wc.axle == .rear) 0.5 else 0,
        .undefined => torque_split = 0.25,
    }
    // Total torque at wheel = engine_torque * ratio * torque_split - brake - handbrake
    return (engine_torque * ratio * torque_split) + brake_torque + handbrake_torque;
}

// ---------------------------------------------------------------------------
// Update wheel angular velocity from torque (Newton's 2nd law: alpha = T / I)
// ---------------------------------------------------------------------------
pub fn updateWheelAngularVelocity(
    wheel_idx: usize,
    config: WheelConfig,
    state: *VehicleState,
    drive_torque: f32,
    dt: f32,
) void {
    var ws = &state.wheels[wheel_idx];
    const wheel_moi = 0.5 * config.wheel_mass * config.wheel_radius * config.wheel_radius;
    // alpha = T / I
    const alpha = drive_torque / wheel_moi;
    ws.angular_velocity += alpha * dt;
    // Rolling resistance (decelerate wheel when no drive)
    if (@abs(drive_torque) < 1.0) {
        ws.angular_velocity *= (1.0 - 0.5 * dt); // simple friction
    }
}

// ===========================================================================
// MAIN VEHICLE UPDATE STEP — call this every frame with dt
// ===========================================================================
pub fn updateVehicle(
    state: *VehicleState,
    config: VehicleConfig,
    dt: f32,
) void {
    // 1. Compute forward + lateral velocity in vehicle-local space
    const forward_dir = state.orientation.rotateVec(Vec3.init(0, 0, 1));
    const right_dir = state.orientation.rotateVec(Vec3.init(1, 0, 0));
    const forward_speed = state.velocity.dot(forward_dir);
    const lateral_speed = state.velocity.dot(right_dir);

    // 2. Per-wheel: raycast → update wheel state → compute slip → apply tire forces
    var total_wheel_omega: f32 = 0;
    var contact_wheels: usize = 0;
    var i: usize = 0;
    while (i < config.wheels.len) : (i += 1) {
        const wc = config.wheels[i];
        // Wheel world position = vehicle position + orientation * offset
        const wheel_local_pos = state.orientation.rotateVec(wc.offset);
        const wheel_world_pos = state.position.add(wheel_local_pos);
        // Suspension axis points down in world space
        const suspension_axis = state.orientation.rotateVec(Vec3.init(0, -1, 0));
        // Raycast
        const ray = raycastGroundPlane(
            wheel_world_pos,
            suspension_axis,
            wc.suspension_rest_length,
            wc.suspension_max_drop,
        );
        updateWheelState(i, wc, state, ray, dt);
        // Compute slip
        const slip = computeWheelSlip(i, wc, state, forward_speed, lateral_speed);
        // Apply drive torque
        const drive_torque = computeDriveTorque(state, config, i);
        updateWheelAngularVelocity(i, wc, state, drive_torque, dt);
        // Apply tire forces (Pacejka) — normal load approximated as mass*gravity/4
        const normal_load = (config.mass * 9.81) / @as(f32, @floatFromInt(config.wheels.len));
        applyTireForces(i, wc, state, slip, normal_load);
        // Accumulate wheel angular velocity for engine RPM update
        if (state.wheels[i].in_contact) {
            total_wheel_omega += @abs(state.wheels[i].angular_velocity);
            contact_wheels += 1;
        }
    }
    // 3. Update engine RPM from average wheel speed
    const avg_omega = if (contact_wheels > 0) total_wheel_omega / @as(f32, @floatFromInt(contact_wheels)) else 0;
    updateEngineRpm(state, config, avg_omega, dt);
    // 4. Update transmission (auto-shift)
    updateTransmission(state, config, dt);
    // 5. Integrate rigid body (apply all forces)
    integrateVehicle(state, config, dt);
}

// ===========================================================================
// TESTS
// ===========================================================================

test "P3 Vehicle Physics: Pacejka returns zero force at zero slip" {
    const params = PacejkaParams{};
    const f = pacejkaLongitudinalForce(0.0, 5000.0, 1.0, params);
    // At zero slip, force should be ~0 (sin(0) = 0)
    try std.testing.expectApproxEqAbs(f, 0.0, 1.0);
}

test "P3 Vehicle Physics: Pacejka peak force at moderate slip" {
    const params = PacejkaParams{};
    // At slip ratio ~0.1, force should be near peak
    const f_zero = pacejkaLongitudinalForce(0.0, 5000.0, 1.0, params);
    const f_peak = pacejkaLongitudinalForce(0.10, 5000.0, 1.0, params);
    try std.testing.expect(f_peak > f_zero);
    try std.testing.expect(f_peak > 0);
}

test "P3 Vehicle Physics: ground plane raycast hits correctly" {
    // Wheel at y=0.5, axis pointing down (-1,0,0)... wait, axis (0,-1,0)
    const r = raycastGroundPlane(
        Vec3.init(0, 0.5, 0),
        Vec3.init(0, -1, 0),
        0.4, // rest
        -0.15, // drop
    );
    try std.testing.expect(r.hit);
    try std.testing.expectApproxEqAbs(r.distance, 0.5, 0.01);
    try std.testing.expectApproxEqAbs(r.contact_point.y, 0.0, 0.01);
}

test "P3 Vehicle Physics: ground plane raycast misses when wheel too high" {
    const r = raycastGroundPlane(
        Vec3.init(0, 2.0, 0),
        Vec3.init(0, -1, 0),
        0.4,
        -0.15,
    );
    try std.testing.expect(!r.hit);
}

test "P3 Vehicle Physics: vehicle state init/deinit" {
    const allocator = std.testing.allocator;
    var state = try VehicleState.init(allocator, 4);
    defer state.deinit(allocator);
    try std.testing.expectEqual(@as(usize, 4), state.wheels.len);
    try std.testing.expectEqual(@as(f32, 800.0), state.engine_rpm);
    try std.testing.expectEqual(@as(i32, 1), state.current_gear);
}

test "P3 Vehicle Physics: engine RPM increases with throttle" {
    const allocator = std.testing.allocator;
    var state = try VehicleState.init(allocator, 4);
    defer state.deinit(allocator);
    state.throttle = 1.0;
    state.engine_rpm = 800;
    const config = VehicleConfig{};
    // Apply engine RPM update — should increase toward redline
    updateEngineRpm(&state, config, 0.0, 0.05);
    try std.testing.expect(state.engine_rpm > 800);
}

test "P3 Vehicle Physics: transmission auto-shifts up at high RPM" {
    const allocator = std.testing.allocator;
    var state = try VehicleState.init(allocator, 4);
    defer state.deinit(allocator);
    state.engine_rpm = 5000; // above change_up_rpm (4500)
    state.current_gear = 1;
    state.throttle = 1.0;
    const config = VehicleConfig{};
    updateTransmission(&state, config, 0.1);
    try std.testing.expectEqual(@as(i32, 2), state.current_gear);
    try std.testing.expect(state.shift_timer > 0);
}
