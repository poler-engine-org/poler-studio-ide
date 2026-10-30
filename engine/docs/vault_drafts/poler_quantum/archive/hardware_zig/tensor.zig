// tensor.zig — Программная f64-модель тензоров POLER на Zig
const std = @import("std");

pub const Tensor = struct {
    data: []f64,
    rows: usize,
    cols: usize,

    pub fn init(allocator: std.mem.Allocator, rows: usize, cols: usize) !Tensor {
        const data = try allocator.alloc(f64, rows * cols);
        @memset(data, 0.0);
        return Tensor{ .data = data, .rows = rows, .cols = cols };
    }

    pub fn deinit(self: *Tensor, allocator: std.mem.Allocator) void {
        allocator.free(self.data);
    }
};
