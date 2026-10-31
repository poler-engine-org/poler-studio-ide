---
name: anticheat-researcher
description: >-
  Анализ и изолированная интеграция драйверов античитов (BattlEye BEDaisy.sys, Easy Anti-Cheat, Vanguard).
  Используй для реверса Ring 0 NT-структур, перехвата колбэков создания процессов (PsSetCreateProcessNotifyRoutine), защиты дескрипторов (ObRegisterCallbacks) и изоляции KUSER_SHARED_DATA.
---

# 🛡️ Anti-Cheat Researcher & Sandbox Integration

Навык **anticheat-researcher** вооружает агента архитектурными паттернами и методиками реверс-инжиниринга драйверов защиты от читов для их легитимного запуска внутри песочницы ядра POLER-OS.

---

## 🎯 Ключевые цели анализа
1. **NT Kernel Callbacks & Hooks:**
   - Моделирование `PsSetCreateProcessNotifyRoutineEx`, `PsSetCreateThreadNotifyRoutine`, `PsSetLoadImageNotifyRoutine`.
   - Имитация `ObRegisterCallbacks` для защиты дескрипторов процесса игры без утечки процессов хоста.
2. **KUSER_SHARED_DATA & Timers:**
   - Предоставление защищенного маппинга страницы `0x7FFE0000` (InterruptTime, SystemTime, ProcessorFeatures).
3. **Memory Integrity Checks:**
   - Обеспечение честной проверки CRC/хешей игровых модулей без права сканирования посторонних областей памяти.
