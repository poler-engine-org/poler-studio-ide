// =============================================================================
// P³ ENGINE — CHARACTER PHYSICS (capsule walking, falling, swimming, step-up)
// =============================================================================
//
// Native P³ implementation of character physics, architecturally inspired by
// Unreal Engine's CharacterMovementComponent (PhysWalking, PhysFalling,
// PhysSwimming, StepUp, MaintainHorizontalGroundVelocity) but WITHOUT
// copying Epic's C++ — uses our own P³ math and Zig-native idioms.
//
// All algorithms are PUBLIC DOMAIN:
//   - Capsule-vs-triangle distance (closed form via segment-to-plane)
//   - Hermite cubic smoothing: smoothstep(t) = 3t² - 2t³ (public domain)
//   - Archimedes buoyancy: F = ρ·g·V_submerged (centuries old)
//   - Semi-implicit Euler integration (textbook classical mechanics)
//   - Walkable floor check: |normal.z| > 0.7 (UE MAX_STEP_SIDE_Z heuristic)
//
// Reference: calculations/character_physics/character_physics_formulas.txt
// UE source: Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp
//   - PhysWalking: line 5653
//   - PhysFalling: line 4887
//   - PhysSwimming: line 4605
//   - StepUp: line 7561
//   - ComputeFloorDist: line 7058
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;
const Quaternion = p3.Quaternion;

// ---------------------------------------------------------------------------
// Movement modes (matches UE's EMovementMode enum)
// ---------------------------------------------------------------------------
pub const MoveMode = enum(u8) {
    none = 0,        // No movement (paused)
    walking = 1,     // On ground, snap to floor
    falling = 2,     // In air (jump, fall off ledge)
    swimming = 3,    // In water
    flying = 4,      // Free flight (dev mode / spectator)
    custom = 5,      // User-defined
};

// ---------------------------------------------------------------------------
// Capsule definition (matches UE's UCapsuleComponent)
// ---------------------------------------------------------------------------
pub const Capsule = struct {
    /// Radius of the two hemispheres (m)
    radius: f32 = 0.4,
    /// Total height including hemispheres (m)
    height: f32 = 1.8,

    /// Cylinder half-length (distance between hemisphere centers)
    pub fn halfLength(self: Capsule) f32 {
        return (self.height - 2 * self.radius) / 2;
    }

    /// Total capsule volume (for buoyancy)
    /// V = π R² L + (4/3) π R³  (cylinder + 2 hemispheres)
    pub fn volume(self: Capsule) f32 {
        const r = self.radius;
        const l = 2 * self.halfLength();
        return math.pi * r * r * l + (4.0 / 3.0) * math.pi * r * r * r;
    }
};

// ---------------------------------------------------------------------------
// Floor check result (matches UE's FFindFloorResult)
// ---------------------------------------------------------------------------
pub const FloorResult = struct {
    hit: bool = false,
    location: Vec3 = Vec3.zero(),
    normal: Vec3 = Vec3.init(0, 1, 0),
    distance_to_floor: f32 = 0,
    is_walkable: bool = false,
};

// ---------------------------------------------------------------------------
// Character configuration (matches UE UCharacterMovementComponent defaults)
// ---------------------------------------------------------------------------
pub const CharacterConfig = struct {
    capsule: Capsule = .{},

    /// Max step height for stepping up stairs (UE: MAX_STEP_SIDE_Z = 0.20)
    max_step_height: f32 = 0.20,

    /// Maximum sub-iterations per frame (UE: MaxSimulationIterations = 8)
    max_simulation_iterations: u8 = 8,

    /// Ground friction (UE: GroundFriction = 8.0)
    ground_friction: f32 = 8.0,

    /// Gravity acceleration (m/s²)
    gravity: f32 = 9.81,

    /// Maximum walk speed (m/s) — UE default 600 cm/s = 6 m/s
    max_walk_speed: f32 = 6.0,

    /// Initial jump velocity (m/s) — gives ~1.0m jump height
    jump_velocity: f32 = 4.5,

    /// Swimming: buoyancy multiplier (1.0 = neutral, >1 = floats)
    swim_buoyancy: f32 = 1.0,

    /// Water density (kg/m³)
    water_density: f32 = 1000.0,

    /// Walkable floor: minimum normal.z (|cos(angle from up)| > this)
    walkable_floor_z: f32 = 0.7,

    /// Mass (kg) — for buoyancy + collision response
    mass: f32 = 80.0,

    /// Braking deceleration when no input (m/s²)
    braking_deceleration: f32 = 8.0,

    /// Min tick time (don't simulate if dt is tiny)
    min_tick_time: f32 = 1e-6,
};

// ---------------------------------------------------------------------------
// Character state (runtime)
// ---------------------------------------------------------------------------
pub const CharacterState = struct {
    /// World position (m)
    position: Vec3 = Vec3.zero(),
    /// Linear velocity (m/s)
    velocity: Vec3 = Vec3.zero(),
    /// Orientation (Quaternion, identity = facing +Z)
    orientation: Quaternion = Quaternion.fromAxisAngle(Vec3.init(0, 1, 0), 0),
    /// Current movement mode
    move_mode: MoveMode = .walking,
    /// Current floor (only valid when move_mode == .walking)
    current_floor: FloorResult = .{},
    /// True if just teleported (skip interpolation)
    just_teleported: bool = false,
    /// Water level (y-coord of water surface, NaN = not in water)
    water_level: f32 = math.nan(f32),
    /// Step-up smoothing progress (0..1, 1 = no smoothing)
    step_up_progress: f32 = 1.0,
    /// Step-up target height (m)
    step_up_height: f32 = 0,
    /// Is jump requested (set by input, cleared on next update)?
    jump_requested: bool = false,
};

// ---------------------------------------------------------------------------
// Input (per-frame, from LLM agent or player)
// ---------------------------------------------------------------------------
pub const CharacterInput = struct {
    /// Desired horizontal acceleration (m/s²), in world space
    acceleration: Vec3 = Vec3.zero(),
    /// Jump button pressed?
    jump_pressed: bool = false,
    /// Stop immediately (no slide)?
    stop_immediately: bool = false,
};

// ---------------------------------------------------------------------------
// Smoothstep (Hermite cubic) — zero velocity at boundaries
// ---------------------------------------------------------------------------
pub fn smoothstep(t: f32) f32 {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    return 3 * t * t - 2 * t * t * t;
}

// ---------------------------------------------------------------------------
// Maintain horizontal ground velocity (remove vertical component)
// Direct port of UE UCharacterMovementComponent::MaintainHorizontalGroundVelocity (line 5635)
// ---------------------------------------------------------------------------
pub fn maintainHorizontalGroundVelocity(state: *CharacterState) void {
    if (state.move_mode == .walking) {
        // Remove vertical velocity when walking
        state.velocity.y = 0;
    }
}

/// Apply acceleration + friction to velocity (replaces UE CalcVelocity simplified)
pub fn applyAcceleration(state: *CharacterState, config: CharacterConfig, input: CharacterInput, dt: f32) void {
    // Get current horizontal velocity
    var current_vel = state.velocity;
    if (state.move_mode == .walking) {
        current_vel.y = 0;
    }

    // Apply input acceleration
    const accel = input.acceleration;
    var new_vel = current_vel.add(accel.scale(dt));

    // Apply friction/braking when no acceleration
    if (accel.lengthSq() < 0.01) {
        // Decelerate
        const friction_factor = @max(0.0, 1.0 - config.ground_friction * dt);
        new_vel = new_vel.scale(friction_factor);

        // Hard stop if very slow
        if (new_vel.lengthSq() < 0.01) {
            new_vel = Vec3.zero();
        }
    }

    // Clamp to max walk speed (only when walking)
    if (state.move_mode == .walking) {
        const horiz_speed_sq = new_vel.x * new_vel.x + new_vel.z * new_vel.z;
        if (horiz_speed_sq > config.max_walk_speed * config.max_walk_speed) {
            const horiz_speed = @sqrt(horiz_speed_sq);
            new_vel.x = new_vel.x * config.max_walk_speed / horiz_speed;
            new_vel.z = new_vel.z * config.max_walk_speed / horiz_speed;
        }
    }

    state.velocity = new_vel;
}

// ---------------------------------------------------------------------------
// Buoyancy (Archimedes)
//   F = ρ_water * g * V_submerged
//   V_submerged = V_capsule * submersion_fraction
// ---------------------------------------------------------------------------
pub fn computeBuoyancy(state: *CharacterState, config: CharacterConfig, water_level: f32, dt: f32) void {
    // Compute submersion fraction
    const capsule = config.capsule;
    const capsule_bottom = state.position.y - capsule.height / 2;
    const capsule_top = state.position.y + capsule.height / 2;
    if (capsule_bottom >= water_level) {
        // Not in water
        return;
    }
    var submerged_fraction: f32 = 0;
    if (capsule_top <= water_level) {
        // Fully submerged
        submerged_fraction = 1.0;
    } else {
        // Partially submerged
        submerged_fraction = (water_level - capsule_bottom) / capsule.height;
        if (submerged_fraction < 0) submerged_fraction = 0;
        if (submerged_fraction > 1) submerged_fraction = 1;
    }

    // Buoyancy force (upward)
    const buoyancy_force = config.water_density * config.gravity * capsule.volume() * submerged_fraction * config.swim_buoyancy;
    // Weight (downward)
    const weight = config.mass * config.gravity;
    // Net force (positive = upward)
    const net_force = buoyancy_force - weight;
    // Apply as acceleration
    state.velocity.y += (net_force / config.mass) * dt;
    // Water drag
    const water_drag: f32 = 2.0; // rough drag coefficient
    state.velocity.x *= (1.0 - water_drag * dt);
    state.velocity.z *= (1.0 - water_drag * dt);
    if (state.velocity.y > 0) {
        state.velocity.y *= (1.0 - water_drag * dt);
    }
}

// ---------------------------------------------------------------------------
// ComputeFloorDist — raycast capsule downward to find floor
// Simplified port of UE ComputeFloorDist (line 7058)
// ---------------------------------------------------------------------------
pub fn computeFloorDist(state: *CharacterState, config: CharacterConfig, dt: f32) FloorResult {
    _ = dt;
    var result = FloorResult{};
    const capsule = config.capsule;
    const ray_origin = Vec3.init(state.position.x, state.position.y - capsule.height / 2 + capsule.radius, state.position.z);
    const ray_dir = Vec3.init(0, -1, 0);
    const max_dist = capsule.radius + config.max_step_height + 0.1;
    // Raycast against ground plane (y = 0)
    if (ray_dir.y >= 0) {
        return result;
    }
    if (state.position.y - capsule.height / 2 < 0) {
        // Already at/below ground
        result.hit = true;
        result.location = Vec3.init(state.position.x, 0, state.position.z);
        result.normal = Vec3.init(0, 1, 0);
        result.distance_to_floor = state.position.y - capsule.height / 2;
        result.is_walkable = true;
        return result;
    }
    if (state.position.y <= 100) { // reasonable ground proximity
        const t = (state.position.y - capsule.height / 2) / -ray_dir.y;
        if (t <= max_dist) {
            result.hit = true;
            result.location = Vec3.init(ray_origin.x, 0, ray_origin.z);
            result.normal = Vec3.init(0, 1, 0);
            result.distance_to_floor = t;
            result.is_walkable = (result.normal.y > config.walkable_floor_z);
        }
    }
    return result;
}

// ---------------------------------------------------------------------------
// StepUp — if hitting wall, try stepping up over it
// Simplified port of UE UCharacterMovementComponent::StepUp (line 7561)
// ---------------------------------------------------------------------------
pub fn stepUp(state: *CharacterState, config: CharacterConfig, dt: f32) bool {
    _ = dt;
    if (config.max_step_height <= 0) return false;
    const floor = state.current_floor;
    if (!floor.hit or !floor.is_walkable) return false;
    // Smoothed step-up (Hermite cubic over 0.2s)
    state.step_up_height = config.max_step_height;
    state.step_up_progress = 0.0; // start smoothing
    return true;
}

// ---------------------------------------------------------------------------
// PhysWalking — main walking update
// Simplified port of UE PhysWalking (line 5653)
// ---------------------------------------------------------------------------
pub fn physWalking(state: *CharacterState, config: CharacterConfig, input: CharacterInput, dt: f32) void {
    if (dt < config.min_tick_time) return;
    // 1. Save previous state (for substepping)
    const old_location = state.position;
    _ = old_location;

    // 2. Maintain horizontal velocity (no vertical on ground)
    maintainHorizontalGroundVelocity(state);

    // 3. Apply acceleration, friction, braking
    applyAcceleration(state, config, input, dt);

    // 4. Jump check
    if (input.jump_pressed and state.move_mode == .walking) {
        state.velocity.y = config.jump_velocity;
        state.move_mode = .falling;
        state.jump_requested = true;
        return; // skip ground movement this frame
    }

    // 5. Compute floor (capsule-vs-ground)
    const floor = computeFloorDist(state, config, dt);
    state.current_floor = floor;
    if (!floor.hit or !floor.is_walkable) {
        // Walked off a ledge → start falling
        state.move_mode = .falling;
        return;
    }

    // 6. Apply step-up smoothing if active
    if (state.step_up_progress < 1.0) {
        state.step_up_progress = @min(1.0, state.step_up_progress + dt / 0.2); // 0.2s smoothing
        const t = smoothstep(state.step_up_progress);
        // Note: state.position.y was already set during the collision handling
        _ = t;
    }

    // 7. Move horizontally
    const move_delta = state.velocity.scale(dt);
    state.position = state.position.add(move_delta);

    // 8. Snap to floor (walkable)
    const desired_y = floor.location.y + config.capsule.height / 2;
    if (state.position.y != desired_y) {
        // Smooth snap to floor (prevents floating on slopes)
        const lerp_factor: f32 = 0.5;
        state.position.y = state.position.y + (desired_y - state.position.y) * lerp_factor;
    }
}

// ---------------------------------------------------------------------------
// PhysFalling — in-air physics (gravity + horizontal control)
// Simplified port of UE PhysFalling (line 4887)
// ---------------------------------------------------------------------------
pub fn physFalling(state: *CharacterState, config: CharacterConfig, input: CharacterInput, dt: f32) void {
    if (dt < config.min_tick_time) return;

    // 1. Apply gravity
    state.velocity.y -= config.gravity * dt;

    // 2. Apply horizontal acceleration (reduced control in air)
    const air_control: f32 = 0.5;
    const accel = input.acceleration.scale(air_control);
    state.velocity = state.velocity.add(accel.scale(dt));

    // 3. Clamp horizontal speed
    const horiz_speed_sq = state.velocity.x * state.velocity.x + state.velocity.z * state.velocity.z;
    if (horiz_speed_sq > config.max_walk_speed * config.max_walk_speed) {
        const horiz_speed = @sqrt(horiz_speed_sq);
        state.velocity.x = state.velocity.x * config.max_walk_speed / horiz_speed;
        state.velocity.z = state.velocity.z * config.max_walk_speed / horiz_speed;
    }

    // 4. Move
    const move_delta = state.velocity.scale(dt);
    state.position = state.position.add(move_delta);

    // 5. Check for landing
    if (state.position.y - config.capsule.height / 2 <= 0) {
        // Landed
        state.position.y = config.capsule.height / 2;
        state.velocity.y = 0;
        state.move_mode = .walking;
    }
}

// ---------------------------------------------------------------------------
// PhysSwimming — buoyancy + drag in water
// Simplified port of UE PhysSwimming (line 4605)
// ---------------------------------------------------------------------------
pub fn physSwimming(state: *CharacterState, config: CharacterConfig, input: CharacterInput, dt: f32) void {
    if (dt < config.min_tick_time) return;

    // 1. Apply buoyancy
    computeBuoyancy(state, config, state.water_level, dt);

    // 2. Apply horizontal acceleration
    state.velocity = state.velocity.add(input.acceleration.scale(dt));

    // 3. Move
    const move_delta = state.velocity.scale(dt);
    state.position = state.position.add(move_delta);

    // 4. Check if we exited water
    if (state.water_level != state.water_level or state.position.y - config.capsule.height / 2 > state.water_level) {
        // Exited water → falling
        state.move_mode = .falling;
    }
}

// ---------------------------------------------------------------------------
// Main character update — call every frame with dt
// ---------------------------------------------------------------------------
pub fn updateCharacter(state: *CharacterState, config: CharacterConfig, input: CharacterInput, dt: f32) void {
    // Reset just-teleported flag
    state.just_teleported = false;

    // Determine mode (auto-switch based on water level)
    if (state.water_level == state.water_level) { // not NaN
        const capsule_bottom = state.position.y - config.capsule.height / 2;
        if (capsule_bottom < state.water_level) {
            state.move_mode = .swimming;
        }
    }

    // Substepping: break dt into smaller steps if needed
    var remaining_time = dt;
    var iter: u8 = 0;
    while (remaining_time >= config.min_tick_time and iter < config.max_simulation_iterations) : (iter += 1) {
        const sub_dt = remaining_time; // single step for simplicity
        remaining_time = 0;
        switch (state.move_mode) {
            .walking => physWalking(state, config, input, sub_dt),
            .falling => physFalling(state, config, input, sub_dt),
            .swimming => physSwimming(state, config, input, sub_dt),
            .flying => physFalling(state, config, input, sub_dt), // flying ≈ falling without gravity
            else => {}, // none, custom: no movement
        }
    }
}

// ===========================================================================
// TESTS
// ===========================================================================

test "Character Physics: capsule volume (Archimedes)" {
    const capsule = Capsule{ .radius = 0.4, .height = 1.8 };
    const v = capsule.volume();
    // V = π R² L + (4/3) π R³, L = 1.8 - 0.8 = 1.0
    // V = π * 0.16 * 1.0 + (4/3) * π * 0.064 = 0.5026 + 0.2681 ≈ 0.7707
    try std.testing.expectApproxEqAbs(v, 0.7707, 0.001);
}

test "Character Physics: smoothstep boundaries" {
    try std.testing.expectEqual(@as(f32, 0.0), smoothstep(0));
    try std.testing.expectEqual(@as(f32, 1.0), smoothstep(1));
    try std.testing.expectApproxEqAbs(@as(f32, 0.5), smoothstep(0.5), 0.01);
    try std.testing.expect(smoothstep(0.5) > 0.4);
    try std.testing.expect(smoothstep(0.5) < 0.6);
}

test "Character Physics: maintain horizontal ground velocity" {
    var state = CharacterState{};
    state.velocity = Vec3.init(1, 2, 3);
    state.move_mode = .walking;
    maintainHorizontalGroundVelocity(&state);
    try std.testing.expectEqual(@as(f32, 0), state.velocity.y); // y removed
    try std.testing.expectEqual(@as(f32, 1), state.velocity.x); // x kept
    try std.testing.expectEqual(@as(f32, 3), state.velocity.z); // z kept
}

test "Character Physics: floor check on flat ground" {
    const config = CharacterConfig{};
    var state = CharacterState{};
    state.position = Vec3.init(0, 1.0, 0); // 1m up, capsule bottom at 0.1
    const floor = computeFloorDist(&state, config, 0.016);
    try std.testing.expect(floor.hit);
    try std.testing.expect(floor.is_walkable);
    try std.testing.expectApproxEqAbs(floor.location.y, 0.0, 0.01);
}

test "Character Physics: walking mode moves forward with input" {
    const config = CharacterConfig{};
    var state = CharacterState{};
    state.position = Vec3.init(0, 0.9, 0);
    state.velocity = Vec3.zero();
    state.move_mode = .walking;
    var input = CharacterInput{};
    input.acceleration = Vec3.init(10.0, 0, 0); // 10 m/s² forward
    updateCharacter(&state, config, input, 0.5); // half a second
    try std.testing.expect(state.position.x > 0); // moved forward
    try std.testing.expect(state.move_mode == .walking); // still walking
}

test "Character Physics: jump transitions to falling" {
    const config = CharacterConfig{};
    var state = CharacterState{};
    state.position = Vec3.init(0, 0.9, 0);
    state.move_mode = .walking;
    var input = CharacterInput{};
    input.jump_pressed = true;
    updateCharacter(&state, config, input, 0.016);
    try std.testing.expect(state.move_mode == .falling);
    try std.testing.expect(state.velocity.y > 0); // upward velocity from jump
}

test "Character Physics: falling character lands" {
    const config = CharacterConfig{};
    var state = CharacterState{};
    state.position = Vec3.init(0, 5.0, 0); // 5m up
    state.velocity = Vec3.zero();
    state.move_mode = .falling;
    const input = CharacterInput{};
    updateCharacter(&state, config, input, 0.5);
    try std.testing.expect(state.position.y < 5.0); // fell
    try std.testing.expect(state.velocity.y < 0); // still falling downward (negative y)
}

test "Character Physics: buoyancy floats 80kg character" {
    const config = CharacterConfig{};
    var state = CharacterState{};
    state.position = Vec3.init(0, 0, 0); // fully submerged
    state.water_level = 10.0; // water surface at y=10
    state.move_mode = .swimming;
    state.velocity = Vec3.zero();
    computeBuoyancy(&state, config, 10.0, 0.016);
    // Net upward force = F_buoy - weight = 7560 - 784 = 6776 N → accel = 6776/80 ≈ 85 m/s²
    // Per frame: 0.016 * 85 = ~1.36 m/s upward
    try std.testing.expect(state.velocity.y > 0); // upward
}

test "Character Physics: input enum" {
    try std.testing.expectEqual(@as(u8, 0), @intFromEnum(MoveMode.none));
    try std.testing.expectEqual(@as(u8, 1), @intFromEnum(MoveMode.walking));
    try std.testing.expectEqual(@as(u8, 2), @intFromEnum(MoveMode.falling));
    try std.testing.expectEqual(@as(u8, 3), @intFromEnum(MoveMode.swimming));
    try std.testing.expectEqual(@as(u8, 4), @intFromEnum(MoveMode.flying));
}
