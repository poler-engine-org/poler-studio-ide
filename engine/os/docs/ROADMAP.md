# POLER-OS Development Roadmap

## Completed: v0.6.0

- [x] Multiboot2 boot into 64-bit long mode
- [x] Identity paging (4GB via 2MB huge pages)
- [x] GDT + TSS setup
- [x] IDT with ISR stubs (vectors 0–48)
- [x] PIC remap (IRQ0–15 → INT 32–47)
- [x] Local APIC timer (vector 48, PIT-calibrated)
- [x] IO-APIC keyboard routing
- [x] ACPI parsing (RSDP/RSDT/MADT/HPET)
- [x] PMM — bitmap physical frame allocator
- [x] VMM — 4-level paging with map/unmap
- [x] Kernel heap — free-list with split/coalesce
- [x] Round-robin scheduler
- [x] Syscall/sysretq mechanism
- [x] CPIO initrd parser
- [x] Framebuffer graphics
- [x] POLER Core ⊗_ε tensor algebra

## Next: v0.6.1 — POLER Core Freestanding

Remove `@import("std")` from poler_core.zig:
- Wrap std-dependent code with `comptime @import("builtin").is_test`
- Keep crypto functions as pure u32 arithmetic (no allocations)
- Real self-test in kernel: output test results via serial

## Next: v0.7.0 — Filesystem + Userspace

- FAT32 driver (reference: NovumOS fat.zig)
- ELF loader
- User mode (Ring 3) with syscall interface
- Process isolation (per-process page tables)

## Next: v0.8.0 — SMP + Networking

- Multi-core boot via SIPI
- Work-stealing scheduler (reference: NovumOS smp.zig)
- RTL8139/e1000 NIC driver
- TCP/IP stack (lwIP port or minimal custom)

## Completed: v0.8.0 — Dual-Personality Kernel

- [x] COW for fork() — clonePML4_COW() with PTE_COW bit
- [x] unmapPageInPML4() for munmap — proper page table cleanup
- [x] ELF loading into per-process PML4 — ET_EXEC + ET_DYN/PIE support
- [x] Real ACL policy for POLER Auth — RSA-OAEP, 10 CSP
- [x] Dual NT+POSIX syscall dispatch — NtXxx/ZwXxx + POSIX simultaneously
- [x] ApiSetMap v6 — 891 virtual API sets, 6 overrides
- [x] VFS ↔ FAT32 integration, ProcessMgr, mmap ↔ VMM

## Completed: v0.9.0 — COW Refcounting + SMP TLB + Dynamic Linker

- [x] Reference counting for COW pages — refPage/unrefPage/freePage with atomics
- [x] SMP TLB shootdown via IPI — cross-core TLB invalidation with wait-for-completion
- [x] ELF64 dynamic linker — .so parsing, RELA/JMPREL relocations, SVR4/GNU hash lookup
- [x] DT_NEEDED enumeration, PLT/GOT setup, init/fini functions

## Completed: v1.0.0 — Production Dynamic Linker

- [x] PLT lock for thread-safe lazy binding — per-library spinlock, double-check on GOT
- [x] TLS (Thread-Local Storage) — PT_TLS parsing, TCB allocation, DTV, __tls_get_addr
- [x] Weak symbols — STB_WEAK resolves to 0 if absent (not error)
- [x] Symbol versioning — DT_VERSYM/DT_VERNEED/DT_VERDEF with version matching
- [x] DT_NEEDED auto-loading from /lib/ via VFS (FAT32 stub, ready for integration)
- [x] BIND_NOW / DF_1_NOW support (eager vs lazy binding)
- [x] FS/GS Base MSR access in HAL — readFsBase/writeFsBase for TLS
- [x] STV_HIDDEN visibility — hidden symbols not exported across libraries
- [x] New relocation types: R_X86_64_TPOFF64, R_X86_64_DTPMOD64, R_X86_64_DTPOFF64

## Next: v1.1.0 — Intent Layer + VFS Integration

- Intent struct + dispatcher
- FS/NET/PROC adapters
- POLER Firewall integration (intent verification via ⊗_ε)
- Object table (capability-based access)
- Full VFS → dynlinker integration (wire FAT32 open/read/close to loadFromVfs)
- Thread creation with TCB allocation and FS_BASE switching

## Next: v2.0.0 — Shell + Self-Hosting

- Interactive shell with pipe/redirection
- Text editor
- Self-hosting Zig compiler (long-term)
