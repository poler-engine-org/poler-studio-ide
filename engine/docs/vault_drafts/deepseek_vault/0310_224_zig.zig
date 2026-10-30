pub const IOAPIC = struct { ... };

Correctness: The IOAPIC base is hardcoded to 0xFEC00000. The redirection entry for IRQ1 (keyboard) is set to vector 33, destination APIC ID 0. The low 32 bits are written with 33, which sets the vector, delivery mode = Fixed, destination = physical, etc. The high 32 bits are 0, meaning APIC ID 0. This is correct.

Potential bug: The code does not check if the IOAPIC is present (but it's standard on most x86 systems). Also, the redirection entry registers for IRQ1 are at offsets 0x12 and 0x13 (since each entry is 64 bits, IRQ1 uses register index 0x10 + 2*1 = 0x12). This is correct.

Security: None.

2.8 PS/2 Keyboard Driver
zig
