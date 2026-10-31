---
name: binary-analyzer
description: >-
  Глубокий статический и динамический анализ бинарных форматов (PE64, ELF64, COFF, DWARF, Dynamic Linker).
  Используй для анализа библиотек CachyOS, трансляции Win32/NT-драйверов и инспекции таблиц экспорта/импорта/релокаций.
---

# 🔬 Binary Analyzer — PE64 & ELF64 Deep Inspection

Навык **binary-analyzer** вооружает агента знаниями и инструментами для инспекции бинарных файлов на уровне байтов, заголовков и структур динамического связывания.

---

## 🛠️ Возможности
1. **ELF64 & Dynamic Linking:** парсинг `PT_INTERP`, `PT_LOAD`, `DT_NEEDED`, проверка выравнивания сегментов и TLS-блоков (`PT_TLS`).
2. **PE64 & NT Structures:** разбор DOS/NT Headers, Data Directories (IAT/EAT, `.reloc`, `.pdata`), подготовка к загрузке Windows-драйверов и античитов.
3. **Disassembly & ABI Verification:** верификация правильности сохранения регистров по System V AMD64 и Microsoft x64 Calling Convention.
