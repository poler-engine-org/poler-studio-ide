pub const PIC = struct { ... };

Correctness: The PIC remapping is standard: master offset 32, slave 40. The mask sets all interrupts except IRQ1 (keyboard) – correct because the APIC timer is used for timer interrupts (vector 48). However, IRQ0 (PIT) is masked, which is fine because APIC timer replaces it.

Potential bug: The comment says "Mask all PIC interrupts — we use APIC timer (vector 32) and IO-APIC for keyboard." But vector 48 is used for APIC timer, not 32. The mask 0xFD (11111101) leaves IRQ1 unmasked (bit 1 is 0). That's correct.

Security/Performance: None.

2.6 APIC (Local APIC)
zig
