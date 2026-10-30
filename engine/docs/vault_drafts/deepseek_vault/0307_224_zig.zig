pub export fn isr_common_handler(frame: *InterruptFrame) callconv(.C) *InterruptFrame { ... }

Correctness:

The function returns a potentially modified frame (used for task switching). This is typical.

For IRQs (vector >= 32), it sends EOI to APIC (if active) and PIC (for legacy IRQs). This is correct.

The timer tick (vector 48) increments tick_count and calls timerTickCallback if set. The callback returns a new frame pointer.

The keyboard IRQ (vector 33) reads the scancode from port 0x60 and processes it via kbd_push.

The serial IRQ (vector 36) is a stub.

Potential bugs:

timerTickCallback is of type *const fn (u64) callconv(.C) u64. It takes a u64 argument, but the call passes @intFromPtr(frame). This is suspicious: the function expects a u64, probably the frame pointer, but the scheduler might expect a pointer to the frame. This could be a mismatch. The callback should probably take *InterruptFrame or u64 – it's unclear. The return value is cast to *InterruptFrame via @ptrFromInt. This is dangerous if the callback returns a different numeric value. The design relies on the callback to return a valid frame pointer. This works, but it's fragile.

The keyboard handler uses inb(0x60) and processes scancodes. However, the keyboard interrupt is only enabled if the PS/2 controller is configured correctly; the init code tries to set translation mode, but there is a potential race if an interrupt occurs during initialization (the code disables the keyboard port during init, so it's safe).

The exception handler (handleException) differentiates user vs kernel faults via frame.cs & 0x3. If user‑mode, it calls exitCallback (if set) and then redirects the frame to idle_after_fault. However, after killing the process, the scheduler will not run until the next timer tick. During that time, the CPU will execute idle_after_fault in ring 0. That is okay, but the exitCallback must mark the current task as dead and not return. The code then sets frame.rsp = 0x10b000 – a hardcoded kernel stack address. This is unsafe; the kernel stack might be in use or corrupted. There should be a dedicated kernel idle stack or the current kernel stack should be preserved.

In kernel‑mode exception, the code calls cli(); hlt(); in an infinite loop. This is acceptable as a panic mechanism.

Security: The user/kernel fault distinction is a critical security boundary. The code does the right thing by killing the user process on user‑mode exceptions. However, the hardcoded rsp = 0x10b000 might be a security issue if the idle stack is in user‑accessible memory or if the stack address is not valid in all contexts. The kernel should maintain a known safe stack for such recovery.

Performance: The keyboard handler uses serial output for every scancode – that's a lot of I/O, but it's only for debugging. In production, this should be removed or made conditional.

2.5 PIC (8259)
zig
