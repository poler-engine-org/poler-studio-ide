// =============================================================================
// P³ ENGINE — O3DE X11/XCB & QT CURSOR RESTORATION INTERCEPTOR (ZIG)
// =============================================================================
// Перехватывает вызовы X11, XFixes, XCB и Qt5Gui, принудительно запрещая
// скрытие системного указателя мыши.
// =============================================================================

const std = @import("std");

pub const xcb_void_cookie_t = extern struct {
    sequence: c_uint = 0,
};

// --- X11 / XFixes Interceptions ---
export fn XFixesHideCursor(dpy: ?*anyopaque, win: c_ulong) callconv(.C) void {
    _ = dpy;
    _ = win;
}

export fn XFixesShowCursor(dpy: ?*anyopaque, win: c_ulong) callconv(.C) void {
    _ = dpy;
    _ = win;
}

export fn XDefineCursor(dpy: ?*anyopaque, win: c_ulong, cursor: c_ulong) callconv(.C) c_int {
    _ = dpy;
    _ = win;
    _ = cursor;
    return 0;
}

export fn XUndefineCursor(dpy: ?*anyopaque, win: c_ulong) callconv(.C) c_int {
    _ = dpy;
    _ = win;
    return 0;
}

// --- XCB Interceptions ---
export fn xcb_xfixes_hide_cursor(c: ?*anyopaque, window: u32) callconv(.C) xcb_void_cookie_t {
    _ = c;
    _ = window;
    return .{ .sequence = 0 };
}

export fn xcb_xfixes_hide_cursor_checked(c: ?*anyopaque, window: u32) callconv(.C) xcb_void_cookie_t {
    _ = c;
    _ = window;
    return .{ .sequence = 0 };
}

export fn xcb_change_window_attributes(c: ?*anyopaque, window: u32, value_mask: u32, value_list: ?*const anyopaque) callconv(.C) xcb_void_cookie_t {
    _ = c;
    _ = window;
    _ = value_mask;
    _ = value_list;
    return .{ .sequence = 0 };
}

// --- Qt5 QGuiApplication override cursor guard ---
export fn _ZN15QGuiApplication17setOverrideCursorERK7QCursor(cursor: ?*const anyopaque) callconv(.C) void {
    _ = cursor;
}

export fn _ZN15QPlatformCursor17setOverrideCursorERK7QCursor(this: ?*anyopaque, cursor: ?*const anyopaque) callconv(.C) void {
    _ = this;
    _ = cursor;
}

export fn P3_CursorFix_IsActive() bool {
    return true;
}
