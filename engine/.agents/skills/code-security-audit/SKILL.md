---
name: code-security-audit
description: >-
  Автоматизированный статический и семантический аудит безопасности кода (CodeQL / AI Security Analyzer / SAST).
  Используй для выявления уязвимостей CWE/OWASP: переполнений буферов, Race Conditions, Use-After-Free, Uninitialized Memory, Register Clobber и криптографических аномалий.
---

# 🛡️ Code Security Audit & SAST Analyzer (CodeQL / AI Security Standard)

Навык **code-security-audit** объединяет правила индустриального статического анализа (CodeQL, Semgrep) и специализированного AI-анализа уязвимостей системного уровня (C, Zig, Rust, ASM).

---

## 🎯 Ключевые паттерны проверок (Audit Matrix)

1. **Kernel ABI & Low-Level Invariants:**
   - **Register Clobber & ABI Breach:** проверка сохранения callee-saved и аргументных регистров (`RAX`, `R10`, `R12..R15`, `FS_BASE`).
   - **Stack/Frame Alignments:** гарантия выравнивания стека 16 байт (`RSP % 16 == 0`) перед вызовами C-ABI / Zig обработчиков.
   - **Uninitialized State Leaks:** поиск неустановленных полей в структурах возврата в Ring 3 (`0xAAAA`-poisoning, `stale`-байты).

2. **Memory Safety & Lifecycle (CWE-119, CWE-416, CWE-476):**
   - **Out-of-Bounds & Buffer Overflows:** границы буферов в `memcpy`, `read`, `pread64`, парсерах заголовков ELF64/PE64.
   - **Null Pointer Dereference:** разыменование непроверенных указателей в деревьях/списках (`_Rb_tree`, дескрипторы).
   - **Demand-Zero & Lazy VMA Overlaps:** проверка коллизий диапазонов при `mmap(MAP_FIXED)`, `mprotect`, `munmap`.

3. **Concurrency & Race Conditions (CWE-362 / TOCTOU):**
   - **Context Hijacking:** изоляция глобальных структур каскада выхода системных вызовов между родительскими и дочерними потоками (`clone`).
   - **Atomic State & Futex Synchronization:** корректность очередей `FUTEX_WAIT` / `FUTEX_WAKE` и `wake_yield`.

4. **Cryptographic & Logic Integrity:**
   - **Constant-Time Execution:** отсутствие ветвлений по секретным ключам Ed25519/RSA.
   - **Zeroization of Sensitive Data:** надежное обнуление временных криптографических буферов.
