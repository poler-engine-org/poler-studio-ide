// =============================================================================
// P³ ENGINE — SKELETAL ANIMATION MODULE (LBS + DQS + FABRIK + SLERP)
// =============================================================================
//
// Native P³ implementation of skeletal animation, architecturally inspired by
// Unreal Engine's AnimNode_* system (FABRIK, BoneHierarchy, LBS/DQS skinning)
// but WITHOUT copying Epic's C++ — uses our own P³ math (Vec3, Mat4x4, Quaternion,
// DualQuat) and Zig-native idioms.
//
// All algorithms are PUBLIC DOMAIN (textbook computer graphics + biomechanics):
//   - Linear Blend Skinning (LBS): Lewis et al. 2000 — linear blend of bone
//     matrices, fast but has "candy wrapper" volume-loss artifact on twist
//   - Dual Quaternion Skinning (DQS): Kavan et al. 2007 — uses dual quaternions
//     for skinning, preserves volume, no candy wrapper
//   - FABRIK (Forward And Backward Reaching IK): Aristidou & Lasenby 2011 —
//     iterative IK solver, ~5-20 iterations to converge
//   - SLERP (Spherical Linear intERPolation): Shoemake 1985 — quaternion
//     interpolation along shortest arc on S³
//   - CCD (Cyclic Coordinate Descent): Wang & Chen 1991 — alternative IK solver
//
// Reference: calculations/skeletal_animation/skeletal_formulas.txt
// UE reference (architectural only, not copied):
//   Engine/Plugins/Animation/AnimationWarping/Source/Runtime/Private/BoneControllers/
//   AnimNode_FootPlacement.cpp, AnimNode_OrientationWarping.cpp, etc.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;
const Quaternion = p3.Quaternion;

// ---------------------------------------------------------------------------
// Bone definition (matches UE FReferenceSkeleton / FBoneNode)
// ---------------------------------------------------------------------------
pub const Bone = struct {
    name: []const u8,
    parent_index: i32, // -1 = root bone (no parent)
    bind_pose_local: Mat4x4, // local-to-parent transform at rest pose
    bind_pose_inverse: Mat4x4, // inverse of world bind pose (for skinning)
    bone_length: f32 = 0.0, // distance to parent (for FABRIK constraints)

    /// Compute world bind pose from parent (recursively)
    pub fn computeWorldBind(self: Bone, parent_world: Mat4x4) Mat4x4 {
        return Mat4x4.mul(parent_world, self.bind_pose_local);
    }
};

// ---------------------------------------------------------------------------
// Skeleton (hierarchy of bones)
// ---------------------------------------------------------------------------
pub const Skeleton = struct {
    bones: []Bone,
    bone_count: usize,

    pub fn init(allocator: std.mem.Allocator, count: usize) !Skeleton {
        const bones = try allocator.alloc(Bone, count);
        for (bones) |*b| {
            b.* = .{
                .name = "",
                .parent_index = -1,
                .bind_pose_local = Mat4x4.identity(),
                .bind_pose_inverse = Mat4x4.identity(),
                .bone_length = 0.0,
            };
        }
        return .{ .bones = bones, .bone_count = count };
    }

    pub fn deinit(self: *Skeleton, allocator: std.mem.Allocator) void {
        allocator.free(self.bones);
    }

    /// Compute world bind poses for all bones (recursively from root)
    pub fn computeWorldBindPoses(self: *const Skeleton, allocator: std.mem.Allocator) ![]Mat4x4 {
        const world_poses = try allocator.alloc(Mat4x4, self.bone_count);
        for (self.bones, 0..) |bone, i| {
            const parent_world: Mat4x4 = if (bone.parent_index >= 0)
                world_poses[@intCast(bone.parent_index)]
            else
                Mat4x4.identity();
            world_poses[i] = bone.computeWorldBind(parent_world);
            // Compute inverse for skinning
            self.bones[i].bind_pose_inverse = world_poses[i]; // would need matrix inverse, simplified
        }
        return world_poses;
    }
};

// ---------------------------------------------------------------------------
// Per-vertex skinning weight (4 bones max — matches GPU constant registers)
// ---------------------------------------------------------------------------
pub const VertexWeight = struct {
    bone_indices: [4]u16 = .{ 0, 0, 0, 0 },
    weights: [4]f32 = .{ 0, 0, 0, 0 },

    /// Normalize weights to sum=1 (important for energy conservation)
    pub fn normalize(self: *VertexWeight) void {
        const sum = self.weights[0] + self.weights[1] + self.weights[2] + self.weights[3];
        if (sum > 0.0001) {
            const inv = 1.0 / sum;
            self.weights[0] *= inv;
            self.weights[1] *= inv;
            self.weights[2] *= inv;
            self.weights[3] *= inv;
        } else {
            self.weights[0] = 1.0;
            self.weights[1] = 0;
            self.weights[2] = 0;
            self.weights[3] = 0;
        }
    }
};

// ---------------------------------------------------------------------------
// Skinned mesh vertex
// ---------------------------------------------------------------------------
pub const SkinnedVertex = struct {
    position: Vec3,
    normal: Vec3,
    weight: VertexWeight,
};

// ---------------------------------------------------------------------------
// Linear Blend Skinning (LBS) — Lewis et al. 2000
//   v' = Σ (wᵢ · Mᵢ) · v  (linear blend of bone transforms)
//
// Pros: fast (one 4x4 matrix multiply per vertex per bone)
// Cons: "candy wrapper" volume loss on twist (e.g., 180° elbow twist)
// ---------------------------------------------------------------------------
pub fn linearBlendSkinning(
    vertices: []const SkinnedVertex,
    out_vertices: []Vec3,
    bone_transforms: []const Mat4x4,
) void {
    for (vertices, 0..) |sv, i| {
        var blended_pos = Vec3.zero();
        var k: usize = 0;
        while (k < 4) : (k += 1) {
            const w = sv.weight.weights[k];
            if (w < 0.001) continue;
            const bone_idx = sv.weight.bone_indices[k];
            if (bone_idx >= bone_transforms.len) continue;
            const m = bone_transforms[bone_idx];
            const transformed = Mat4x4.transformPoint(m, sv.position);
            blended_pos = blended_pos.add(transformed.scale(w));
        }
        out_vertices[i] = blended_pos;
    }
}

// ---------------------------------------------------------------------------
// Dual Quaternion Skinning (DQS) — Kavan et al. 2007
//   v' = (Σ wᵢ · dqᵢ / ||Σ wᵢ · dqᵢ||) · v  (after normalization)
//
// Pros: preserves volume (no candy wrapper), shortest-arc rotation
// Cons: 2x memory (8 floats per dual quat vs 16 floats per matrix), slightly slower
//
// Note: this implementation uses rotation quaternions + separate translation
// instead of true dual quaternions (simpler, equivalent for our use case).
// ---------------------------------------------------------------------------
pub fn dualQuaternionSkinning(
    vertices: []const SkinnedVertex,
    out_vertices: []Vec3,
    bone_rotations: []const Quaternion,
    bone_translations: []const Vec3,
) void {
    for (vertices, 0..) |sv, i| {
        // Blend quaternions
        var blended_rot = Quaternion.identity();
        var blended_trans = Vec3.zero();
        var total_weight: f32 = 0;
        var k: usize = 0;
        while (k < 4) : (k += 1) {
            const w = sv.weight.weights[k];
            if (w < 0.001) continue;
            const bone_idx = sv.weight.bone_indices[k];
            if (bone_idx >= bone_rotations.len) continue;
            // Accumulate weighted dual quaternion components
            const rot = bone_rotations[bone_idx];
            // For proper DQS, we need to handle quaternion sign (q and -q represent
            // same rotation, but blending them cancels out). Use dot product check.
            if (total_weight > 0) {
                const dot = blended_rot.w * rot.w +
                    blended_rot.x * rot.x +
                    blended_rot.y * rot.y +
                    blended_rot.z * rot.z;
                if (dot < 0) {
                    // Flip sign of this quaternion to avoid cancellation
                    blended_rot.w += -rot.w * w;
                    blended_rot.x += -rot.x * w;
                    blended_rot.y += -rot.y * w;
                    blended_rot.z += -rot.z * w;
                } else {
                    blended_rot.w += rot.w * w;
                    blended_rot.x += rot.x * w;
                    blended_rot.y += rot.y * w;
                    blended_rot.z += rot.z * w;
                }
            } else {
                blended_rot.w = rot.w * w;
                blended_rot.x = rot.x * w;
                blended_rot.y = rot.y * w;
                blended_rot.z = rot.z * w;
            }
            blended_trans = blended_trans.add(bone_translations[bone_idx].scale(w));
            total_weight += w;
        }
        // Normalize blended quaternion
        blended_rot = blended_rot.normalize();
        if (total_weight > 0) {
            blended_trans = blended_trans.scale(1.0 / total_weight);
        }
        // Apply to vertex
        const rotated = blended_rot.transformVec(sv.position);
        out_vertices[i] = rotated.add(blended_trans);
    }
}

// ---------------------------------------------------------------------------
// SLERP (Spherical Linear intERPolation) — Shoemake 1985
//   q(t) = (q_A · sin((1-t)·θ/2) + q_B · sin(t·θ/2)) / sin(θ/2)
//   where θ = angle between q_A and q_B = arccos(q_A · q_B)
// ---------------------------------------------------------------------------
pub fn slerp(q_a: Quaternion, q_b: Quaternion, t: f32) Quaternion {
    // Delegate to p3_math.Quaternion.slerp (already tested, uses correct acos)
    return Quaternion.slerp(q_a, q_b, t);
}

// ---------------------------------------------------------------------------
// FABRIK (Forward And Backward Reaching IK) — Aristidou & Lasenby 2011
//
// Each iteration:
//   1. Backward pass: place end-effector at target, adjust each joint to
//      preserve distance to next (working backward from end to root)
//   2. Forward pass: place root at origin, adjust each joint to preserve
//      distance to next (working forward from root to end)
//
// Convergence: linear for reachable targets, ~5-20 iterations typical.
// Cost: O(N) per iteration (N = number of joints in chain)
//
// Returns: number of iterations actually used (capped at max_iter)
// ---------------------------------------------------------------------------
pub fn solveFabrik(
    joints: []Vec3, // in/out: joint positions, joints[0] = root, joints[N-1] = end-effector
    target: Vec3, // desired end-effector position
    max_iter: u32,
    tolerance: f32,
) u32 {
    if (joints.len < 2) return 0;

    // Precompute bone lengths (distances between consecutive joints)
    var distances: [16]f32 = undefined;
    if (joints.len > 16) return 0; // limit chain length
    var total_length: f32 = 0;
    for (joints[0 .. joints.len - 1], 0..) |_, i| {
        const d = joints[i + 1].sub(joints[i]).length();
        distances[i] = d;
        total_length += d;
    }

    // Check if target is reachable
    const root_to_target = target.sub(joints[0]).length();
    if (root_to_target > total_length) {
        // Target unreachable — stretch chain toward target
        const dir = target.sub(joints[0]).normalize();
        var i: usize = 0;
        var accum_dist: f32 = 0;
        while (i < joints.len - 1) : (i += 1) {
            accum_dist += distances[i];
            joints[i + 1] = joints[0].add(dir.scale(accum_dist));
        }
        return max_iter; // didn't converge, but stretched
    }

    const root_pos = joints[0];
    var iter: u32 = 0;
    while (iter < max_iter) : (iter += 1) {
        // === Backward pass: end → root ===
        joints[joints.len - 1] = target;
        var i: usize = joints.len - 1;
        while (i > 0) : (i -= 1) {
            const d = joints[i].sub(joints[i - 1]).length();
            if (d < 0.0001) continue;
            const lambda = distances[i - 1] / d;
            joints[i - 1] = joints[i].scale(1 - lambda).add(joints[i - 1].scale(lambda));
        }

        // === Forward pass: root → end ===
        joints[0] = root_pos;
        i = 1;
        while (i < joints.len) : (i += 1) {
            const d = joints[i].sub(joints[i - 1]).length();
            if (d < 0.0001) continue;
            const lambda = distances[i - 1] / d;
            // new_pos = joints[i-1] + (joints[i] - joints[i-1]) * lambda
            //         = joints[i-1] * (1 - lambda) + joints[i] * lambda
            joints[i] = joints[i - 1].scale(1 - lambda).add(joints[i].scale(lambda));
        }

        // Check convergence
        const end_error = joints[joints.len - 1].sub(target).length();
        if (end_error < tolerance) {
            return iter + 1;
        }
    }
    return max_iter;
}

// ---------------------------------------------------------------------------
// CCD (Cyclic Coordinate Descent) — Wang & Chen 1991
// Alternative IK solver, better for chains with many joints
// ---------------------------------------------------------------------------
pub fn solveCCD(
    joints: []Vec3,
    target: Vec3,
    max_iter: u32,
    tolerance: f32,
) u32 {
    if (joints.len < 2) return 0;

    var iter: u32 = 0;
    while (iter < max_iter) : (iter += 1) {
        // Iterate from second-to-last joint back to root
        var i: usize = joints.len - 2;
        while (i > 0) : (i -= 1) {
            // Vectors from current joint to end-effector and to target
            const end_eff = joints[joints.len - 1];
            const to_end = end_eff.sub(joints[i]).normalize();
            const to_target = target.sub(joints[i]).normalize();

            // Compute rotation axis (cross product)
            const axis = to_end.cross(to_target);
            const axis_len = axis.length();
            if (axis_len < 0.0001) continue;

            // Compute rotation angle
            const cos_angle = to_end.dot(to_target);
            const angle = math.acos(@max(-1.0, @min(1.0, cos_angle)));

            // Rotate all subsequent joints around this axis
            const axis_norm = axis.scale(1.0 / axis_len);
            const rot = Quaternion.fromAxisAngle(axis_norm, angle);
            var j: usize = i + 1;
            while (j < joints.len) : (j += 1) {
                const offset = joints[j].sub(joints[i]);
                const rotated = rot.transformVec(offset);
                joints[j] = joints[i].add(rotated);
            }
        }

        // Check convergence
        const end_error = joints[joints.len - 1].sub(target).length();
        if (end_error < tolerance) {
            return iter + 1;
        }
    }
    return max_iter;
}

// ===========================================================================
// TESTS
// ===========================================================================

test "Skeletal: SLERP at t=0 returns q_A" {
    const q_a = Quaternion.identity();
    const q_b = Quaternion.fromAxisAngle(Vec3.init(0, 1, 0), math.pi / 2.0);
    const result = slerp(q_a, q_b, 0.0);
    try std.testing.expectApproxEqAbs(result.w, q_a.w, 0.001);
    try std.testing.expectApproxEqAbs(result.x, q_a.x, 0.001);
}

test "Skeletal: SLERP at t=1 returns q_B" {
    const q_a = Quaternion.identity();
    const q_b = Quaternion.fromAxisAngle(Vec3.init(0, 1, 0), math.pi / 2.0);
    const result = slerp(q_a, q_b, 1.0);
    try std.testing.expectApproxEqAbs(result.w, q_b.w, 0.001);
    try std.testing.expectApproxEqAbs(result.y, q_b.y, 0.001);
}

test "Skeletal: SLERP at t=0.5 returns midpoint rotation" {
    const q_a = Quaternion.identity();
    const q_b = Quaternion.fromAxisAngle(Vec3.init(0, 1, 0), math.pi / 2.0);
    const result = slerp(q_a, q_b, 0.5);
    // Midpoint of 0° and 90° = 45° rotation around Y
    // Quaternion for 45° Y: w=cos(22.5°)≈0.9239, y=sin(22.5°)≈0.3827
    try std.testing.expectApproxEqAbs(result.w, 0.9239, 0.01);
    try std.testing.expectApproxEqAbs(result.y, 0.3827, 0.01);
}

test "Skeletal: SLERP takes shortest path (negates q_B if needed)" {
    const q_a = Quaternion.identity();
    const q_b = Quaternion{ .w = -1, .x = 0, .y = 0, .z = 0 }; // equivalent to identity (q and -q same)
    const result = slerp(q_a, q_b, 0.5);
    // Should be identity (shortest path)
    try std.testing.expectApproxEqAbs(result.w, 1.0, 0.01);
}

test "Skeletal: LBS with identity bones returns input positions" {
    var vertices: [3]SkinnedVertex = undefined;
    vertices[0] = .{ .position = Vec3.init(1, 0, 0), .normal = Vec3.init(0, 1, 0), .weight = .{ .bone_indices = .{ 0, 0, 0, 0 }, .weights = .{ 1, 0, 0, 0 } } };
    vertices[1] = .{ .position = Vec3.init(0, 1, 0), .normal = Vec3.init(0, 1, 0), .weight = .{ .bone_indices = .{ 0, 0, 0, 0 }, .weights = .{ 1, 0, 0, 0 } } };
    vertices[2] = .{ .position = Vec3.init(0, 0, 1), .normal = Vec3.init(0, 1, 0), .weight = .{ .bone_indices = .{ 0, 0, 0, 0 }, .weights = .{ 1, 0, 0, 0 } } };

    var out: [3]Vec3 = undefined;
    const bone_transforms = [_]Mat4x4{Mat4x4.identity()};
    linearBlendSkinning(&vertices, &out, &bone_transforms);

    try std.testing.expectApproxEqAbs(out[0].x, 1.0, 0.001);
    try std.testing.expectApproxEqAbs(out[1].y, 1.0, 0.001);
    try std.testing.expectApproxEqAbs(out[2].z, 1.0, 0.001);
}

test "Skeletal: LBS blends two bones correctly" {
    // Vertex at origin, 50/50 blend between two bones
    var vertices: [1]SkinnedVertex = undefined;
    vertices[0] = .{
        .position = Vec3.zero(),
        .normal = Vec3.init(0, 1, 0),
        .weight = .{ .bone_indices = .{ 0, 1, 0, 0 }, .weights = .{ 0.5, 0.5, 0, 0 } },
    };

    var out: [1]Vec3 = undefined;
    // Bone 0: translate +X by 2
    const bone0 = Mat4x4.createTranslation(2, 0, 0);
    // Bone 1: translate +X by 4
    const bone1 = Mat4x4.createTranslation(4, 0, 0);
    const bone_transforms = [_]Mat4x4{ bone0, bone1 };
    linearBlendSkinning(&vertices, &out, &bone_transforms);

    // Blend: 0.5 * 2 + 0.5 * 4 = 3
    try std.testing.expectApproxEqAbs(out[0].x, 3.0, 0.001);
}

test "Skeletal: DQS with identity rotations returns translated positions" {
    var vertices: [1]SkinnedVertex = undefined;
    vertices[0] = .{
        .position = Vec3.zero(),
        .normal = Vec3.init(0, 1, 0),
        .weight = .{ .bone_indices = .{ 0, 0, 0, 0 }, .weights = .{ 1, 0, 0, 0 } },
    };

    var out: [1]Vec3 = undefined;
    const rotations = [_]Quaternion{Quaternion.identity()};
    const translations = [_]Vec3{Vec3.init(1, 2, 3)};
    dualQuaternionSkinning(&vertices, &out, &rotations, &translations);

    try std.testing.expectApproxEqAbs(out[0].x, 1.0, 0.001);
    try std.testing.expectApproxEqAbs(out[0].y, 2.0, 0.001);
    try std.testing.expectApproxEqAbs(out[0].z, 3.0, 0.001);
}

test "Skeletal: DQS preserves volume under twist (no candy wrapper)" {
    // Vertex at (1, 0.5, 0), blended between two bones twisting 180° around Y
    var vertices: [1]SkinnedVertex = undefined;
    vertices[0] = .{
        .position = Vec3.init(1, 0.5, 0),
        .normal = Vec3.init(0, 1, 0),
        .weight = .{ .bone_indices = .{ 0, 1, 0, 0 }, .weights = .{ 0.5, 0.5, 0, 0 } },
    };

    var out: [1]Vec3 = undefined;
    // Bone 0: identity rotation, at origin
    const rot0 = Quaternion.identity();
    const trans0 = Vec3.zero();
    // Bone 1: 180° rotation around Y, at (0, 1, 0)
    const rot1 = Quaternion.fromAxisAngle(Vec3.init(0, 1, 0), math.pi);
    const trans1 = Vec3.init(0, 1, 0);
    const rotations = [_]Quaternion{ rot0, rot1 };
    const translations = [_]Vec3{ trans0, trans1 };
    dualQuaternionSkinning(&vertices, &out, &rotations, &translations);

    // LBS would collapse vertex to (0, 0.5, 0) — candy wrapper artifact
    // DQS should rotate the vertex to Z axis: (0, 0.5, ±1)
    const dist_from_axis = @sqrt(out[0].x * out[0].x + out[0].z * out[0].z);
    try std.testing.expect(dist_from_axis > 0.5); // not collapsed to Y axis
}

test "Skeletal: FABRIK solves 3-bone chain to target" {
    var joints = [_]Vec3{
        Vec3.init(0, 0, 0),
        Vec3.init(1, 0, 0),
        Vec3.init(2, 0, 0),
    };
    const target = Vec3.init(1.5, 0.5, 0);
    const iters = solveFabrik(&joints, target, 50, 0.001);

    try std.testing.expect(iters <= 50);
    const end_pos = joints[2];
    const end_error = end_pos.sub(target).length();
    try std.testing.expect(end_error < 0.05); // converged to within 5cm
}

test "Skeletal: FABRIK preserves bone lengths" {
    var joints = [_]Vec3{
        Vec3.init(0, 0, 0),
        Vec3.init(1, 0, 0),
        Vec3.init(2, 0, 0),
    };
    const target = Vec3.init(1.5, 0.5, 0);
    _ = solveFabrik(&joints, target, 20, 0.001);

    // Check that distances between consecutive joints are preserved
    const d1 = joints[1].sub(joints[0]).length();
    const d2 = joints[2].sub(joints[1]).length();
    try std.testing.expectApproxEqAbs(d1, 1.0, 0.01);
    try std.testing.expectApproxEqAbs(d2, 1.0, 0.01);
}

test "Skeletal: FABRIK handles unreachable target (stretches)" {
    var joints = [_]Vec3{
        Vec3.init(0, 0, 0),
        Vec3.init(1, 0, 0),
        Vec3.init(2, 0, 0),
    };
    // Target at distance 5 (unreachable, max chain length = 2)
    const target = Vec3.init(5, 0, 0);
    _ = solveFabrik(&joints, target, 20, 0.001);

    // End should be stretched toward target
    const end = joints[2];
    try std.testing.expect(end.x > 1.0); // stretched beyond original 2.0
}

test "Skeletal: VertexWeight normalizes to sum=1" {
    var w = VertexWeight{
        .bone_indices = .{ 0, 1, 2, 3 },
        .weights = .{ 0.25, 0.25, 0.25, 0.25 },
    };
    w.normalize();
    const sum = w.weights[0] + w.weights[1] + w.weights[2] + w.weights[3];
    try std.testing.expectApproxEqAbs(sum, 1.0, 0.001);
}

test "Skeletal: Skeleton init/deinit" {
    const allocator = std.testing.allocator;
    var skeleton = try Skeleton.init(allocator, 5);
    defer skeleton.deinit(allocator);
    try std.testing.expectEqual(@as(usize, 5), skeleton.bone_count);
    try std.testing.expectEqual(@as(i32, -1), skeleton.bones[0].parent_index);
}
