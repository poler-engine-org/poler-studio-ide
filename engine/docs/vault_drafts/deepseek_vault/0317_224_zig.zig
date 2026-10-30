pub fn init() void { ... }

Correctness: The init order is: Serial, GDT, IDT, PIC, TSS, APIC, IOAPIC, keyboard, sti(). This is a reasonable sequence. PIC is initialised before APIC, which is fine because PIC is still used for keyboard (via IOAPIC). The keyboard init happens after IOAPIC and enables interrupts, but sti() is called after all init.

Potential bug: The TSS is loaded before APIC init; that's fine. The APIC.init() calls PIT.calibrateApicTicks which uses APIC and PIT; this may take a while and may cause interrupts (PIT interrupts are masked, so it's okay). However, the PIC is already initialised, and APIC is not yet fully set up (timer not configured). The calibration uses PIT channel 2, which doesn't generate interrupts, so it's safe.

Security: None.

2.15 Global Callbacks & Spinlock
zig
