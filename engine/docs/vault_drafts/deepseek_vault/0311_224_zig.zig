var kbd_shift: bool = false; ...

Correctness:

The keyboard driver uses the controller's translation mode (bit 6 of command byte) to convert Set 2 scancodes to Set 1. This is the recommended approach to avoid double translation. The code explicitly enables translation and does not send 0xF0 0x01 to set scancode set 1. This is correct.

The scancode table (scan_to_ascii) is for Set 1 scancodes.

The handler handles extended keys (0xE0 prefix), modifier keys (Shift, Ctrl, Alt), and key releases (bit 7 set).

The keyboard buffer is a circular buffer of 256 bytes.

Potential bugs:

The init sequence: it disables the keyboard port, reads the command byte, sets bit 6 (translation) and bit 0 (IRQ enable), clears bit 4 (enable port), then writes it back. However, it also resets the keyboard with 0xFF. The reset may clear some settings (like typematic rate), but it's okay. The code waits for BAT (0xAA) but consumes ACK and continues if timeout. This is robust.

There is a possible race condition: after enabling the keyboard port and IRQ, an interrupt could occur before the driver is ready (e.g., before kbd_head/tail are initialized). However, the code does kbd_head = 0; kbd_tail = 0; after the reset and draining, so it's safe.

The keyboard handler uses global state (kbd_shift, etc.) without any locking. If an interrupt occurs while the handler is running, it could be reentrant? Interrupts are disabled during the ISR (since it's an interrupt gate), so it's not reentrant. However, if multiple CPUs (SMP) are active, the state is not per‑CPU and may be corrupted. This is a known issue for SMP – keyboard interrupts are usually handled on the BSP only. The code doesn't mask keyboard IRQ on other CPUs, so an interrupt could be delivered to an AP if the IOAPIC destination is not set to BSP only. The IOAPIC init sets destination APIC ID 0, which is the BSP, so it's fine.

Security: None.

Performance: The keyboard handler uses Serial.puts for every scancode (debug output). This will slow down input significantly and should be disabled or guarded by a debug flag.

2.9 Serial Port
zig
