# POLER-OS Capability-Based Security: Результаты архитектурного аудита v0.9.0

> Дата: 2026-07-18  
> Версия кодовой базы: v0.9.0 (commit 24da718)  
> Объём: ~22 000 строк Zig 0.13.0  

---

## 1. Архитектурная оценка

### 1.1 Соответствие принципам capability-based безопасности

Предложенная модель **Capability Token + Semantic Gateway + AI-capsule** частично следует классическим принципам capability-based безопасности, но содержит ряд существенных расхождений.

**Где модель совпадает с принципами:**

- **Правило подмножества при деривации.** Функция `deriveCapability()` в `capability.zig` (строка 39) гарантирует `requested_child_caps & ~parent_caps != 0 → null`. Это классическое свойство *no-escalation*, аналогичное seL4 CNode derivation: дочерняя capability никогда не расширяет права родителя.
- **Объектные права доступа.** `HandleEntry.access_mask: u32` в `object_manager.zig` реализует per-handle ACL — аналог Zircon handles, где каждый handle имеет граничные права. Функция `checkObjectAccess()` в `capability.zig` проверяет `(access_mask & required_access) == required_access`.
- **Изоляция AI-капсул.** `AI_DEFAULT_CAPS` в `ai_capsule.zig` исключает `CAP_ADMIN`, `CAP_RAW_IO`, `CAP_PRIVILEGE`, `CAP_DEVICE` — аналогично Capsicum capability mode, где процесс ограничен в доступе к глобальному пространству имён.

**Где модель расходится с принципами:**

- **Ambient authority не устранена.** В `subsystem.zig:dispatch()` решение принимается через `polerAuthenticate(action)`, который читает `pcb.acl_capabilities` — глобальную маску процесса. Процесс НЕ должен явно предоставлять capability при каждом системном вызове; вместо этого ядро неявно проверяет права. Это нарушение принципа *no ambient authority* из Capsicum/CHERI — процесс может вызвать `open()` без предъявления file capability.
- **Отсутствие разграничения объектов.** `u64 acl_capabilities` — это bitmask уровня процесса. Когда процесс имеет `CAP_FILE_READ`, он может читать ВСЕ файлы, а не только те, на которые у него есть capability. Это прямо расходится с seL4 и Zircon.
- **Отсутствие conveyance restriction.** В IPC определён `MSG_TYPE_CAP_TRANSFER` и `carries_handle`, но **реализация переноса handle между процессами отсутствует**. `channelSend()` просто копирует сообщение, не изымая handle из таблицы отправителя. Это позволяет дублирование capabilities.

### 1.2 Обоснование отказа от HMAC-SHA256 в capability tokens

Решение отказаться от HMAC-SHA256 в `poler_token[32]` в пользу kernel object references — **архитектурно верное**. Обоснование:

```
┌─────────────────────────────────────────────────────────────────┐
│         HMAC-SHA256 в capability tokens                         │
│  ┌──────────┐     ┌─────────────────┐     ┌──────────────────┐ │
│  │ Userspace │────→│ Token = HMAC(   │────→│ Kernel verifies  │ │
│  │ presents  │     │   key, caps,    │     │ HMAC signature   │ │
│  │ token     │     │   expiry, pid)  │     │                  │ │
│  └──────────┘     └─────────────────┘     └──────────────────┘ │
│                                                                   │
│  Проблемы:                                                        │
│  1. Ключ HMAC где хранить? В kernel memory → уже нужен           │
│     protection kernel memory → circular dependency               │
│  2. revocation = invalidate token → нужен replay cache            │
│  3. 32-byte token на каждый syscall → cache pressure             │
│  4. Timing side-channel на HMAC verification                     │
│  5. Отсутствует в seL4, Zircon, CHERI — нигде не нужно          │
└─────────────────────────────────────────────────────────────────┘
```

**Аргумент 1: Тавтология защиты.** Если ядро уже защищает `ProcessControlBlock` (kernel-only памяти, Ring 0), то добавление HMAC-SHA256 создаёт **второй слой защиты того же самого состояния**.

**Аргумент 2: Отсутствие распределённой составляющей.** Ни одна из production-систем (seL4, Zircon, CHERI) не использует криптографию для runtime capability enforcement. Криптография нужна только для *network capabilities*, но POLER-OS — single-machine OS.

**Аргумент 3: Проблема revocation.** HMAC-token нельзя отозвать без replay cache. `revokeCaps()` в `capability.zig` просто очищает биты — мгновенный эффект.

**Аргумент 4: Производительность.** HMAC-SHA256 на 32 байта ≈ 500 тактов. Текущая проверка bitmask — 1-2 такта. Разница в ~250x.

### 1.3 Трёхслойная архитектура

```
┌──────────────────────────────────────────────┐
│           Layer 3: AI-capsule                │
│   ai_capsule.zig — lifecycle management      │
│   (create/start/update/rollback/stop)        │
│   AI_DEFAULT_CAPS, memory_quota, TTL         │
├──────────────────────────────────────────────┤
│           Layer 2: Policy Engine             │
│   policy_engine.zig — rule evaluation        │
│   (Allow/Deny/Audit/RateLimit)               │
│   64 rules, whitelist-by-default             │
├──────────────────────────────────────────────┤
│           Layer 1: Capability Kernel         │
│   kernel_integrate.zig — ACL check           │
│   capability.zig — derive/check/revoke       │
│   PCB.acl_capabilities (u64 bitmask)         │
└──────────────────────────────────────────────┘
```

Архитектура **звукова по замыслу**, но имеет дефекты реализации:

**Дефект 1: Layer 2 не используется.** `policy_engine.zig` существует, но **НЕ вызывается** из `polerAuthenticate()`. Policy Engine — мёртвый код.

**Дефект 2: Нет разделения enforcement и policy.** `polerAuthenticate()` одновременно проверяет capabilities И определяет sensitivity level.

**Дефект 3: Layer 3 не изолирован аппаратно.** AI-capsule — просто процесс с ограниченными `acl_capabilities`, нет Ring 3 enforcement.

### 1.4 Эволюция u64 bitmask ACL → полная capability модель

| Аспект | Текущее (u64 bitmask) | Целевое (capability model) | Break нужен? |
|--------|----------------------|---------------------------|-------------|
| Гранулярность | Процесс-уровень | Объект-уровень (per-handle) | Нет — `access_mask` уже есть |
| Деривация | `deriveCapability()` | CNode-подобная иерархия | Да — нужна CSpace таблица |
| Передача | IPC `carries_handle` (не реализовано) | Handle transfer (move semantics) | Да — нужен handle relocation |
| Revocation | Мгновенная (`revokeCaps`) | Мгновенная (kernel memory) | Нет |
| TTL | `cap_expire_tick` в PCB | Per-capability TTL | Да — нужен capability descriptor |

`u64 bitmask` может сосуществовать с объектными capabilities как **coarse-grained gate**. Каждый syscall сначала проверяет bitmask (быстро), затем при необходимости — object-level `access_mask`.

### 1.5 Отсутствующие механизмы

1. **Capability spacing / CNode** — нет иерархической организации capabilities
2. **Revocation propagation** — отзыв у родителя не отзывает у потомков
3. **Audit log persistence** — кольцевой буфер 256 записей, теряется при reboot
4. **Rate limiting** — `PolicyDecision.RateLimit` определён, но не реализован
5. **Capability separation для IPC** — каналы не проверяют capabilities отправителя/получателя
6. **Mandatory Access Control** — нет MAC enforcement
7. **Secure boot / measured boot** — нет TPM, нет измерения целостности ядра
8. **Kernel address space isolation** — identity-mapped физическая память

---

## 2. Анализ модели угроз

### Сводная таблица угроз

| # | Угроза | Вероятность | Критичность | Статус защиты |
|---|--------|------------|-------------|---------------|
| 2.1 | Компрометация AI-агента | H | Высокая | Частичная |
| 2.2 | RCE в AI-capsule | H | Высокая | Минимальная |
| 2.3 | Privilege escalation | M | Критическая | Частичная |
| 2.4 | IPC-атаки | H | Высокая | Минимальная |
| 2.5 | TOCTOU | M | Высокая | Отсутствует |
| 2.6 | Confused Deputy | M | Высокая | Отсутствует |
| 2.7 | Replay-атаки | L | Средняя | Частичная |
| 2.8 | Подделка capabilities | L | Высокая | Частичная |
| 2.9 | Утечки памяти | H | Критическая | Частичная |
| 2.10 | Side-channel | M | Средняя | Частичная |
| 2.11 | DMA-атаки | H | Критическая | Минимальная |
| 2.12 | Bootloader | H | Критическая | Отсутствует |
| 2.13 | Драйверы | H | Критическая | Минимальная |
| 2.14 | Kernel update | M | Средняя | Отсутствует |
| 2.15 | AI update | M | Высокая | Частичная |
| 2.16 | Физический доступ | H | Критическая | Отсутствует |
| 2.17 | Chain of trust | H | **Критическая** | **Отсутствует** |

### Детальный анализ ключевых угроз

**2.1 Компрометация AI-агента** (H / Высокая)
AI-код работает в userspace с реальными capabilities; Python interpreter — огромная атакная поверхность. Компрометированный AI-capsule имеет `CAP_FILE_READ|WRITE|EXECUTE`, `CAP_PROCESS_CREATE`, может читать/писать файлы и создавать процессы. Защита: `AI_DEFAULT_CAPS` исключает опасные caps, `memory_quota` и `cap_expire_tick` ограничивают ресурсы. Необходимо: per-object capabilities, network egress filtering, syscall rate limiting, seccomp-like whitelist.

**2.4 IPC-атаки** (H / Высокая)
`channelSend()` не проверяет что `sender_handle` принадлежит отправителю — любой процесс может отправить от чужого handle. Нет authentication на сообщениях. `CHANNEL_QUEUE_SIZE=16` — можно заблокировать канал. `MSG_TYPE_CAP_TRANSFER` не реализован — при реализации без move-semantics будет handle duplication. Необходимо: привязка handle к PID, move-semantics для handle transfer, per-process channel limit.

**2.5 TOCTOU** (M / Высокая)
На SMP другой CPU может изменить `acl_capabilities` через `processMgrSetCaps()` между проверкой и использованием. `ProcessControlBlock` не защищён spinlock'ом. Необходимо: Per-PCB spinlock, атомарное чтение через `@atomicLoad()`, sequence lock.

**2.11 DMA-атаки** (H / Критическая)
VirtIO-BLK DMA buffers identity-mapped (phys==virt). Компрометированное устройство может писать в произвольные физические адреса. Необходимо: IOMMU, DMA buffer isolation, userspace драйверы.

**2.17 Цепочка доверия** (H / Критическая)
POLER token MAC вычисляется, но **НИГДЕ не верифицируется**. MAC computation в `polerAuthenticate()` — dead code. Token non-zero check тривиально обходится. Необходимо: реализовать MAC verification, либо удалить dead code, заменить `generateProcessToken()` PRF на SipHash/HMAC с kernel secret key.

---

## 3. Capability-модель

### 3.1 Формат объекта

| Уровень | Поле | Тип | Место хранения |
|---------|------|-----|----------------|
| Процесс | `acl_capabilities` | `u64` | `ProcessControlBlock` |
| Дескриптор | `access_mask` | `u32` | `HandleEntry` |
| TTL процесса | `cap_expire_tick` | `u64` | `ProcessControlBlock` |
| Версия | `acl_version` | `u32` | `ProcessControlBlock` |

**Критический дефект:** `acl_version` **никогда не проверяется** при авторизации. `access_mask` живёт в параллельной вселенной от `acl_capabilities` — нет связи между `CAP_FILE_READ` и `ACCESS_READ`.

**Стратегия миграции:**
1. Фаза 1: Добавить `CapDescriptor` как обёртку над u64 с дополнительными полями, сохранить `acl_capabilities` как «плоскую проекцию»
2. Фаза 2: Заменить прямые обращения на accessor-функции
3. Фаза 3: Перевести `CapDescriptor` на индекс в CapTable (аналог CNode)

### 3.2 Делегирование и передача

**Проблемы:**
- fork() — слепое копирование capabilities без вызова `deriveCapability()`
- IPC `carries_handle` определён но не обрабатывается
- `grantCaps()` позволяет расширить capability-множество — не делегирование, а повышение привилегий

**Рекомендация:** Гибридный подход — u64 bitmask как «паспорт процесса» (coarse-grain), per-handle `access_mask` как «виза на конкретный объект» (fine-grain). При fork() вызывать `deriveCapability()` с маской политики.

### 3.3 Отзыв (revocation)

**Критические проблемы:**
1. Нет каскадного отзыва — отзыв у родителя не влияет на потомков (fork копирует значение, не ссылку)
2. `rollbackCapsule()` может **вернуть** отозванные capability из снапшота
3. Длинные операции не прерываются при отзыве в середине

**Рекомендация:** Ввести `revoke_caps_cascade(pid, caps)` — обход потомков через `ppid`.

### 3.4 Ограничение области действия (scope)

**Наиболее критический архитектурный дефект:** `CAP_FILE_READ` — глобальное разрешение на ВСЕ файлы. `checkHandleAccess()` существует, но **не вызывается** из `polerAuthenticate()`.

**Рекомендация — стратегия C (двойная проверка):** Наименее инвазивная. В `polerAuthenticate()` добавить вызов `objmgr.checkHandleAccess(handle, required_access)` после проверки `acl_capabilities`.

### 3.5 Срок действия (TTL)

TTL измеряется в `scheduler_ticks`, но точная частота **не документирована**. `processMgrCheckCapExpiry()` — пассивная проверка, вызывается вручную. TTL привязан к процессу в целом, а не к отдельным capability-битам.

**Рекомендация:** Ввести `CAP_MINIMAL_SURVIVAL` вместо захардкоженного набора. Добавить периодическую проверку TTL в `schedule()`.

### 3.6 Поколения (generation)

`acl_version` и `acl_global_version` **никогда не проверяются**. Per-handle `cap_generation` проверяется только косвенно через `cap_revoked` (boolean).

**Рекомендация:** При кэшировании capability сохранять `acl_version`; при syscall сверять с текущим.

### 3.7 Сериализация и хранение

`carries_handle` в `IpcMessage` не обрабатывается в `channelSend()`/`channelReceive()`. CapSnapshot — volatile-only (теряется при reboot). `exec()` не сбрасывает `cap_expire_tick`.

**Рекомендация:** Реализовать обработку `carries_handle` с move-semantics; при exec() сбрасывать TTL; добавить persistent storage для CapSnapshot.

### Сводная таблица дефектов

| Подсекция | Критичность | Дефект | Файл:строка |
|-----------|-------------|--------|-------------|
| 3.1 | Средняя | `acl_version` никогда не проверяется | kernel_integrate.zig:271 |
| 3.2 | Высокая | fork() не вызывает `deriveCapability()` | kernel_integrate.zig:477 |
| 3.2 | Высокая | IPC `carries_handle` не обрабатывается | ipc.zig:40 |
| 3.3 | Высокая | Нет каскадного отзыва | — |
| 3.4 | Критическая | `checkHandleAccess()` не вызывается при авторизации | object_manager.zig:182 |
| 3.5 | Средняя | Нет автоматической проверки TTL в scheduler | scheduler.zig |
| 3.7 | Средняя | `exec()` не сбрасывает `cap_expire_tick` | kernel_integrate.zig:499 |

---

## 4. Криптографические подписи в capabilities

| Подход | Плюсы | Минусы | Применимость |
|--------|-------|--------|-------------|
| HMAC-SHA256 (отклонён) | Уже реализован в rsa_oaep.zig; constant-time сравнение | Ключ — единая точка компрометации; ~500 тактов на syscall; replay vulnerability | ❌ Отклонён обоснованно |
| RSA-OAEP (есть в poler_core) | Асимметричная; публичный ключ безопасно распространять | RSA подпись = тысячи умножений; OAEP — шифрование, не подпись | ⚠️ Только для boot-time verification |
| Kernel object references (ВЫБРАН) | Нулевая криптография; O(1) проверка; полная изоляция | Защита заканчивается на границе ядра; нет non-repudiation | ✅ Оптимальный выбор |
| Zircon handles | Проверенная архитектура; совместимость с POSIX-like API | Утечка handle = несанкционированный доступ | ✅ Ближайший к текущему дизайну |
| CHERI | Аппаратная защита; zero-cost проверка | Требует CHERI-совместимый процессор; x86_64 не поддерживает | ❌ Неприменим сейчас |
| i386 сегменты | Аппаратная проверка привилегий | Устаревшая модель; x86_64 депрекировал сегментацию | ❌ Категорически неприемлем |

### Когда криптография оправдана

**Оправдана:** capabilities пересекают границы доверия (сетевой IPC), долгосрочное хранение на диск, non-repudiation, проверка без доверенного пути.

**Не нужна:** все capabilities внутри одного ядра, latency-critical paths, нет ключей = нет утечки ключей.

### Завершение POLER token verification

**Да, но с правильной архитектурой.** «Токен» = kernel object reference (handle). Проверка означает:
1. Handle validation (O(1) bounds check) — **должно быть реализовано**
2. Rights check (битовая операция AND) — **должно быть реализовано**
3. Revocation check — **должно быть реализовано**
4. Криптографическая верификация — **не должна реализовываться** для локальных capabilities

---

## 5. Ядро

### 5.1 Планировщик (scheduler.zig)

**Добавить:** Per-CPU run queue для SMP; проверка capabilities в schedule() (пропускать задачи с истёкшими caps); приоритеты для AI-капсул; расширить MAX_TASKS до 256 для Phase 2.

### 5.2 Диспетчер syscall (syscall_integration.zig)

**Изменить:** Перенести capability check в `zig_syscall_handler()` ДО dispatch — сейчас legacy syscalls (1–6) обходят проверку. Добавить SubsystemId.AI routing (0x3000+).

### 5.3 Диспетчер capabilities (capability.zig)

**Критический дефект:** `polerAuthenticate()` НЕ вызывает capability.zig — два независимых механизма проверки capabilities. Необходим рефакторинг: polerAuthenticate() должен делегировать в capability.zig.

**Добавить:** Cascading revocation; capability inheritance policy при fork().

### 5.4 Менеджер памяти (pmm64.zig + vmm64.zig)

**Добавить:** Per-process memory quota enforcement — `memory_quota` в PCB определён, но НЕ проверяется в vmm. COW protection от capability misuse — проверять `CAP_MEMORY_MMAP` перед handleCowPageFault().

### 5.5 IPC (ipc.zig)

**Критические проблемы:**
- Спинлок `Channel.lock` объявлен, но **НИКОГДА не используется** — data race при SMP
- `findChannelByHandle()` — линейный поиск O(32)
- `channelReceive()` не блокирует — нет интеграции с планировщиком
- Handle transfer — нет атомарности

### 5.6 Процессы и потоки (kernel_integrate.zig)

**Добавить:** `pid: u32` в Task для O(1) PCB lookup; AI capsule автоматически получает ограниченный capability set при `subsystem_id = .AI`.

### 5.7 Пространства имён

**Добавить:** Per-process handle table (сейчас глобальная); handles КАК capabilities (Zircon model); namespace isolation для AI (`/capsule/` sub-tree).

### 5.8 Драйверы

**Добавить:** User-space driver model для AI safety; драйвер в Ring 3 общается через Channel IPC; AI-капсула НЕ получает `CAP_DEVICE`.

### 5.9 VFS

**Изменить:** `vfsOpen()` и `vfsWrite()` не проверяют capabilities — необходимо проверять `CAP_FILE_READ`/`CAP_FILE_WRITE`.

**Добавить:** Per-file required_caps; VFS mount options с capability gating.

### 5.10 Служба политик (policy_engine.zig)

**Критический дефект:** Policy Engine **НЕ вызывается** из syscall path — мёртвый код. `PolicySubsystem` дублирует `SubsystemId`. Две независимые версии: `acl_global_version` и `policy_version`.

**Рекомендация:** Интегрировать: `polerAuthenticate() → cap.checkCapabilitiesWithTtl() → policy_engine.evaluate() → execute`.

---

## 6. AI-подсистема

### 6.1 Жизненный цикл

**Оценка:** Модуль `ai_capsule.zig` (304 строки) реализует полный цикл: create/start/update/rollback/stop.

**Проблемы:**
- При updateCapsule() процесс **удерживает** capabilities пока загружается новый ELF — если он вредоносный, немедленно получает все capabilities старого
- rollbackCapsule() восстанавливает capabilities, но НЕ ELF-бинарник — вредоносный код может быть уже внедрён
- createCapsule() не проверяет CAP_AI_MANAGE у вызывающего

**Рекомендации:** Strip capabilities перед update, restore после верификации; ELF signature verification; two-phase commit (suspend → load → verify → resume).

### 6.2 Sandbox

**Аппаратная изоляция:** Ring 3 + separate PML4 — базовая, но неполная:
- Нет SMEP/SMAP — установить биты CR4
- Нет IOMMU — DMA isolation полностью отсутствует
- Нет ограничения RDTSC для AI-капсул

**Syscall surface:** `AI_DEFAULT_CAPS` должен исключать `CAP_NETWORK` и ограничивать `CAP_MEMORY_MMAP` (принудительный PTE_NO_EXECUTE, запрет mprotect с PROT_EXEC).

**Сравнение с seL4:** В seL4 поток не может открыть файл без capability на конкретный объект. В POLER-OS `CAP_FILE_READ` — глобальное разрешение. Необходим whitelist объектов для AI.

### 6.3 Управление ресурсами

- **CPU:** Чистый Round-Robin без квот. Необходим иерархический планировщик с budget.
- **RAM:** `memory_quota` в PCB не проверяется VMM. Необходимо: проверка при каждом allocPage().
- **FS:** Нет chroot/namespace isolation. Необходим `root_inode` per-PCB.
- **Сеть:** AI_DEFAULT_CAPS не должен включать CAP_NETWORK. Обновление моделей — через IPC к fetcher-процессу.

### 6.4 IPC для AI

- **AI ↔ Kernel:** SYSCALL — основной путь (быстрый, synchronous, auditable). IPC-канал — опционально для асинхронных уведомлений.
- **AI ↔ Другие процессы:** Нужен для inference-запросов и результатов. MAC на IPC-сообщениях.
- **Формат:** 64 байта (кэш-линия) + shared memory для payload > 56 байт.

### 6.5 Журналирование

256 записей — **категорически недостаточно** для AI. Необходимо: минимум 8192; отдельный `AiAuditEntry` с capsule_id, config_version, code_hash; persistent log на FAT32.

---

## 7. API

### 7.1 Общие принципы

AI использует SubsystemId.AI (0x3000+). Capability token передаётся неявно через ProcessControlBlock.

### 7.2 Новые AI syscall

| Системный вызов | Номер | Требуемые capabilities |
|-----------------|-------|----------------------|
| `AI_SYSCALL_VFS_QUERY` | `0x3000` | `CAP_AI_RUNTIME \| CAP_FILE_READ` |
| `AI_SYSCALL_SPAWN_CHILD` | `0x3001` | `CAP_AI_RUNTIME \| CAP_PROCESS_CREATE` |
| `AI_SYSCALL_MMAP_QUOTA` | `0x3002` | `CAP_AI_RUNTIME \| CAP_MEMORY_MMAP` |
| `AI_SYSCALL_CHANNEL_CREATE` | `0x3010` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_CHANNEL_SEND` | `0x3011` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_CHANNEL_RECV` | `0x3012` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_CHANNEL_DESTROY` | `0x3013` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_NET_REQUEST` | `0x3020` | `CAP_AI_RUNTIME \| CAP_NETWORK` |
| `AI_SYSCALL_EVENT_WAIT` | `0x3030` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_AUDIT_READ` | `0x3040` | `CAP_AI_RUNTIME \| CAP_AUDIT_READ` |
| `AI_SYSCALL_CAP_QUERY` | `0x3050` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_CAP_DERIVE` | `0x3051` | `CAP_AI_RUNTIME` |
| `AI_SYSCALL_CREATE_CAPSULE` | `0x3060` | `CAP_AI_MANAGE \| CAP_POLER_AUTH` |
| `AI_SYSCALL_START_CAPSULE` | `0x3061` | `CAP_AI_MANAGE` |
| `AI_SYSCALL_UPDATE_CAPSULE` | `0x3062` | `CAP_AI_MANAGE` |
| `AI_SYSCALL_ROLLBACK_CAPSULE` | `0x3063` | `CAP_AI_MANAGE` |
| `AI_SYSCALL_STOP_CAPSULE` | `0x3064` | `CAP_AI_MANAGE` |
| `AI_SYSCALL_POLICY_ADD` | `0x3070` | `CAP_POLICY_SET \| CAP_ADMIN \| CAP_POLER_AUTH` |
| `AI_SYSCALL_POLICY_REMOVE` | `0x3071` | `CAP_POLICY_SET \| CAP_ADMIN` |
| `AI_SYSCALL_POLICY_EVAL` | `0x3072` | `CAP_AI_RUNTIME \| CAP_POLER_AUTH` |

**Критическая находка:** В `policy_engine.zig` правило для AI-капсул указывает `syscall_min=0x2010`, что попадает в POLER_NATIVE, а не AI (0x3000+). Правило никогда не сработает.

---

## 8. Zig — структура проекта

### Новые файлы

| Файл | Назначение | Приоритет |
|------|-----------|-----------|
| `subsystem/ai/ai_api.zig` | Обработчик AI-syscall (0x3000+) | **Критический** |
| `cap_defs.zig` | CAP_* константы (вынести из kernel_integrate.zig) | Высокий |
| `error_types.zig` | Общие типы ошибок | Высокий |
| `resource_quota.zig` | Учёт квот: memory, CPU | Средний |
| `sandbox.zig` | Sandboxing для AI-капсул | Средний |

### Циклические зависимости

**`kernel_integrate.zig` ↔ `subsystem.zig`** — самый опасный цикл. Решение: вынести общие типы в `cap_defs.zig` и `error_types.zig`; использовать callback-паттерн (уже применяется для scheduler/dynlinker).

### Декомпозиция kernel_integrate.zig

Разделить ~1240 строк на:
- `vfs.zig` — VFS↔FAT32
- `process_mgr.zig` — ProcessManager + COW fork
- `memory_mgr.zig` — MemoryManager
- `poler_auth.zig` — POLER Auth + ACL
- `kernel_init.zig` — Init + TCB wiring

---

## 9. Roadmap реализации

| Фаза | Цель | Статус | Критерий готовности |
|------|------|--------|-------------------|
| 9.1 | Capability module | ✅ DONE | deriveCapability + checkWithTtl + revokeCaps |
| 9.2 | Policy Engine | ✅ DONE | 8 default rules, evaluate() |
| 9.3 | AI Capsule lifecycle | ✅ DONE | create/start/update/rollback/stop |
| 9.4 | Channel IPC | ✅ DONE | createChannel/send/receive/destroy |
| 9.5 | Per-object capabilities | TODO | checkHandleAccess() вызывается из polerAuthenticate() |
| 9.6 | Memory quota enforcement | TODO | VMM проверяет pcb.memory_quota при mmap |
| 9.7 | AI capsule ELF loading | TODO | Python interpreter ELF загружается в AI capsule |
| 9.8 | Persistent audit log | TODO | AclAuditEntry записывается на FAT32 |
| 9.9 | **Userspace enforcement (Ring 3)** | TODO | Shell в Ring 3, все процессы через syscall gates |
| 9.10 | IOMMU для DMA isolation | TODO | VirtIO-BLK DMA через IOMMU domains |
| 9.11 | Secure boot chain | TODO | SHA-256 kernel hash + UEFI Secure Boot |
| 9.12 | Network stack для AI | TODO | Проксированный доступ через fetcher-процесс |

**Оценка:** ~35 недель соло, ~20 недель с 2 разработчиками. Фаза 9.9 (Ring 3) — **единственная наиболее критичная**.

---

## 10. Риски

### 5 критических находок

1. **Ring 0 only** — все процессы работают в kernel mode; capability system advisory, не enforced
2. **Global ObjectManager** — единственная таблица handles для всех процессов; переход к per-process будет экспоненциально дороже с каждым новым feature
3. **Нет тестов** — 22K строк, ноль meaningful unit tests
4. **Predictable POLER tokens** — `generateProcessToken()` использует PID + subsystem + константу; нет entropy source
5. **Static Task kernel stacks** — MAX_TASKS=64 × 8KB = 512KB BSS; нужно dynamic allocation перед Ring 3

### Что менять сейчас (пока проект ранний)

- Перенести Shell в Ring 3
- Завершить POLER token verification
- Включить IOMMU
- Добавить базовый тестовый фреймворк

### Решения, ведущие к тупику

- Отложенный переход на per-process handle tables
- Игнорирование Zig 0.13.0 breaking changes
- Добавление новых subsystem без унификации авторизации

### Что сохранить любой ценой

- SipHash heap integrity
- 64-bit ACL bitmask (эволюционирует, не заменяется)
- COW с refcounting + TLB shootdown

---

## 11. Практические рекомендации

### P0 — Обязательно реализовать (critical for security)

1. **Перенос Shell из Ring 0 в userspace** — без этого все защиты обходятся через Shell
2. **Завершение POLER token verification** — capability-модель номинальна без этого
3. **IOMMU для DMA isolation** — устройства могут обходить всю защиту памяти
4. **Userspace enforcement (Ring 3)** — страничная защита для разделения kernel/user
5. **Изоляция AI-капсул на уровне памяти** — каждая капсула в собственном address space
6. **Унификация авторизации NT+POSIX** — единый механизм, обе подсистемы обращаются к нему
7. **Kernel object references вместо неаудитируемых указателей**

### P1 — Желательно реализовать (important for robustness)

1. Revocation для capability-токенов с атомарным инвалидированием
2. Rate limiting на Semantic Gateway
3. Лёгкая криптографическая защита токенов (SipHash с kernel secret)
4. Система аудит-логов для всех операций с capabilities
5. Тестовый фреймворк для capability-модели (fuzzing)
6. Версионирование протокола Channel IPC
7. Структурированная обработка отказов AI-капсул
8. Секьюрити-ревью Zig 0.13.0 runtime

### P2 — Можно отложить (nice to have)

1. Формальная верификация capability-модуля
2. Hardware-enforced CFI (Intel CET)
3. Миграция на микроядерную архитектуру
4. Многомерные квоты для AI-капсул
5. Визуальный дашборд для мониторинга капсул
6. Интерактивный отладчик capability-политик
7. Интеграция с TPM

### P3 — Можно исключить (unnecessary or harmful)

1. HMAC-SHA256 для capability-токенов — уже обоснованно отклонён
2. Полная POSIX-совместимость как цель первого приоритета
3. Userspace-драйверы для всех устройств (достаточно IOMMU)
4. Сложная RBAC-модель поверх capabilities
5. Поддержка 32-битных платформ

### Заменить более надёжными

1. Identity-mapped DMA → IOMMU + bounce buffers
2. Shell в Ring 0 → supervisor process в Ring 3
3. Неаудитируемые указатели → целочисленные дескрипторы с kernel-side таблицей
4. Неформальная проверка токенов → SipHash-2-4 с per-boot ключом
5. Монолитный policy engine → декомпозированный набор политик

---

## 12. Итоговая оценка

### Числовые оценки

| Критерий | Оценка (1–10) |
|---|---|
| Предложенная архитектура (Capability Token + Semantic Gateway + AI-capsule) | **8.5** |
| Текущая архитектура POLER-OS v0.9.0 | **5.0** |
| Реалистичность эволюционного перехода | **6.5** |

### Сильные стороны текущей архитектуры

1. Двойная совместимость NT+POSIX — уникальное позиционирование
2. Реализация на Zig 0.13.0 — контролируемое UB, comptime, нулевые скрытые аллокации
3. Уже реализованный capability-модуль — фундамент заложен
4. AI-капсулы с TTL, квотами и rollback — глубже, чем в большинстве ОС
5. Channel IPC — элегантная альтернатива сырым pipes
6. Выбор kernel object references вместо HMAC — архитектурно верное решение
7. Version tracking для AI-капсул — критичен для безопасной эксплуатации

### Слабые стороны текущей архитектуры

1. Shell в Ring 0 — катастрофическая уязвимость
2. Незавершённая верификация POLER-токенов — модель номинальна
3. Identity-mapped DMA — устройства обходят защиту памяти
4. Отсутствие userspace enforcement — все процессы привилегированы
5. Двойной стек NT+POSIX без унификации авторизации
6. Нет криптографической защиты токенов
7. 22K строк без тестового покрытия
8. Zig 0.13.0 — нестабильный toolchain

### Сильные стороны предложенной модели

1. Capability Token как единый механизм авторизации — устраняет ambiguity
2. Semantic Gateway — осмысленная фильтрация IPC, уникальная для AI-aware ОС
3. Двойная модель capabilities (u64/u32) — элегантный компромисс
4. AI-capsule lifecycle — полный управляемый цикл
5. Композиционность — capabilities передаются через Channel IPC
6. Kernel object references — простая, верифицируемая модель
7. Отказ от HMAC-SHA256 — зрелое решение
8. Эволюционная совместимость — не требует переписывания

### Слабые стороны предложенной модели

1. Semantic Gateway — потенциальное узкое место и единая точка отказа
2. Сложность reasoning о комбинированных u64/u32 caps
3. Отсутствие формальной модели для policy engine
4. Риск over-engineering для ~22K строк
5. Неясная история с revocation в асинхронной среде
6. Зависимость от единого kernel secret
7. Нет ответа на hardware-level атаки (Spectre, Rowhammer)
8. Zig runtime не предоставляет memory safety

### Наиболее реалистичный путь эволюции

**Этап 1 (0–4 месяца): Замыкание perimeter.** Устранение трёх катастрофических уязвимостей: Shell → Ring 3, POLER token verification + SipHash, IOMMU для DMA. Настройка страничной защиты. Менее 5% кодовой базы, но оценка безопасности 5.0 → ~7.0.

**Этап 2 (4–10 месяцев): Закрепление модели.** Capability-модель интегрируется во все подсистемы. Semantic Gateway с rate limiting. Audit log persistence. AI memory isolation. Fuzzing-фреймворк. Оценка 7.0 → 8.0.

**Этап 3 (10–18 месяцев): Зрелость и формализация.** Policy engine декомпозиция. Формальная верификация критических путей. Микроядерная миграция критичных подсистем. Hardware-enforced механизмы (CFI, TPM). Оценка 8.0 → 8.5+.

**Ключевой принцип:** никогда не ломать ABI — каждый этап добавляет ограничения, но не удаляет интерфейсы.
