---
name: kernel-fuzzer
description: >-
  Стресс-тестирование системных вызовов, фаззинг структур данных ядра и поиск граничных состояний (Kernel Fuzzing & Stress Testing).
  Используй для проверки ABI, обнаружения race conditions, переполнений дескрипторов и некорректных errno.
---

# ⚡ Kernel Fuzzer — Syscall Stress & State Testing

Навык **kernel-fuzzer** предназначен для непрерывного стресс-тестирования системных вызовов, валидации дескрипторов и проверки стабильности планировщика задач ядра.

---

## 🎯 Ключевые паттерны анализа
1. **Syscall Argument Mutation:** проверка реакции ядра на невалидные указатели, нулевые буферы, отрицательные смещения и переполнения `size_t`.
2. **Descriptor Boundary Testing:** проверка лимитов дескрипторов файлов (`F_DUPFD`, `pipe2`, `epoll`, `socketpair`).
3. **Concurrency & Race Conditions:** фаззинг многопоточных вызовов (`sys_clone`, `futex`, `madvise`).
