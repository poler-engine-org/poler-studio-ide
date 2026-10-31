# BUILDING — сборка POLER Engine (канонический протокол)

> Цель: собрать движок на любой машине — от рабочей станции до Core i5
> с 4 ГБ RAM. Протокол ниже — **канонический**: золотое правило `-j1`,
> debug-профиль для проверок, release — только для бинарника, без
> каких-либо переопределений профиля. Быстрая карта: [README.md](README.md) ·
> вклад: [CONTRIBUTING.md](CONTRIBUTING.md) (коммиты/PR — §3.5) ·
> тесты: [docs/TESTING.md](docs/TESTING.md).

---

## 1. Что понадобится

| Требование | Минимум | Примечание |
|---|---|---|
| ОС | Linux x86-64 (glibc) | Ubuntu / ядро 5.x–6.x |
| Rust | ≥ 1.80 (`rust-version` в Cargo.toml) | проверено на 1.99.0 stable |
| RAM | **4 ГБ** при `-j1` | 8+ ГБ → можно `-j2` (как CI) |
| Свободное место | ~5 ГБ | `target/` release-артефакты |
| Сеть | только первый `cargo build` | далее `--offline` работает |

Установка Rust (если нет):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"          # или новый терминал
rustc --version                    # должно быть >= 1.80
```

## 2. Получение исходников

```bash
git clone https://github.com/poler-engine-org/poler-engine.git
cd poler-engine
git log --oneline -1               # убедись, что на свежем main
```

Проект — гибрид package+workspace: корневой пакет `poler-engine` плюс крейты
`crates/{pqc,pqw,reader,poler-ffi}`. Свежий clone + cargo build работает
из коробки, ничего дополнительно настраивать не надо.

---

## 3. Канонический протокол сборки

### Шаг 1 — быстрая проверка и тесты (debug-профиль, LTO не нужен)

Проверка корректности кода и математика собираются быстро и легко по памяти —
релизный профиль для этого не требуется:

```bash
cargo check --workspace
cargo test -p poler-engine --lib calc::matrix::tests::eigen_sturm   # Штурм-спектр
cargo test -p pqc                                                   # квантовое ядро
```

### Шаг 2 — релизный бинарник

```bash
cargo build --release -p poler-engine -j1
```

> **Золотое правило памяти.** В манифесте включена максимальная оптимизация:
> LTO + `codegen-units = 1` + `opt-level = 3`. Если свободно меньше 8–16 ГБ
> RAM, параллельная сборка всеми ядрами может словить OOM Killer на этапе
> линковки. Правило: **всегда `-j1`** (или `-j2` на 8+ ГБ). Профиль не
> переопределяется — никаких `CARGO_PROFILE_RELEASE_*` переменных:
> канонический бинарник один, и он full-LTO.

### Шаг 3 — установка и обязательный чек-лист

```bash
cp -f target/release/poler-engine ~/.local/bin/poler-engine
```

Чек-лист после сборки (все три пункта обязательны):

1. `poler-engine --version` → актуальная версия; `poler-engine --schema`
   возвращает чистый JSON.
2. `poler-engine --exec 'calc eigen_sturm(tridiag(2,-1,256))' --json` —
   спектр 256×256 за ≤ 60 мс.
3. `poler-engine --triune-speak 'привет' --triune-tokens 12` — отклик
   Триединства без сбоев.

---

## 4. Все 37 MCP-инструментов (pnd-ffi, Zig-ядро)

Если нужен прямой мост к крипто-ядру PND и системный пул `poler_exec*`
(32 базовых инструмента → 37):

```bash
cargo build --release -p poler-engine --features pnd-ffi -j1
```

Требует Zig-тулчейн (0.14+ / 0.16, `os/core/`). Опционально: для базовой
сборки Zig **не нужен**.

## 5. AI-шлюз poler-api (совсем без Cargo)

`tools/ai-gateway/poler-api.rs` — автономный шлюз над установленным
бинарником (schema-манифест + подкоманды + MCP-прокси 34 инструмента),
чистый std, собирается голым `rustc` за секунды на любом железе:

```bash
rustc -O tools/ai-gateway/poler-api.rs -o ~/.local/bin/poler-api
poler-api selftest   # 15/15 PASS = готово
```

## 6. Архиватор `.poler` и poler-box (изоляция ядра, Zero-Disk)

Собранный по §3 бинарник — это ещё и **архиватор** и **микро-контейнер**:
никаких отдельных утилит ставить не надо, всё уже внутри `poler-engine`.

**Зачем архиватор `.poler`.** Суверенное правило владельца: если для
инструментов не хватает диска — они сжимаются архиватором и вызываются
изнутри архива. Контейнер стримит данные (в т.ч. >100 GiB из сети) прямо
в чанкованный формат **без промежуточной несжатой высадки на диск**:
FastCDC content-defined чанкинг + Zstd + BLAKE3-дедупликация + sha256 на
каждую запись. Поиск (`--grep --archives`), чтение и CoW-патчинг записей
идут без распаковки (в логе разработки: 1.5 ГБ упакованы на 41.8 MB/s при
11 МБ RAM; патч Blink внутри `chromium_full.poler` 1.49 ГБ — 27 с).

**Зачем poler-box.** «Замена Docker без ОС» — циклическая обёртка
исполнения поверх `.poler`: payload запускается **напрямую из архива**
(zero-disk), нативное железо и память хоста, но изнутри коробка
непробиваема — namespaces + pivot_root + seccomp, а губернатор обрывает
пожирателей RSS/CPU без вреда хосту.

### 6.1 Упаковка в `.poler`

```bash
# один файл/стрим (статический бинарник, дамп, датасет):
poler-engine --stream-file ./payload --output-archive payload.poler

# файловая таблица (rootfs с библиотеками) — tar.gz разворачивается
# в таблицу записей автоматически (tar_mode):
tar czf rootfs.tar.gz rootfs/
poler-engine --stream-file rootfs.tar.gz --output-archive box.poler

# инспекция и контроль целостности:
poler-engine --poler-list box.poler          # таблица записей + sha256
poler-engine --poler-verify box.poler        # all_ok: true
poler-engine --poler-extract box.poler --extract-dir out/   # с sha256-контролем
poler-engine --poler-cat box.poler --cat-name rootfs/bin/hello | head -1
```

Прочие операции: `--poler-patch <POLER> --manifest <JSON>` (in-place
CoW-патч: replace/add/delete), `--poler-rollback <POLER>` (откат по
`.polerbak`), `--poler-remux <POLER> --output-archive <OUT>` (tar.gz-блоб →
файловая таблица), `--grep <PAT> --archives` (поиск внутри архивов
без распаковки).

### 6.2 Запуск в коробке

```bash
# А) статический payload — стримится в память и исполняется сразу:
poler-engine --poler-box payload.poler --box-entry payload \
  --box-rss-mb 64 --box-cpu-s 5 --box-tmpfs-mb 32

# Б) динамический payload — нужен rootfs (ld-linux + libc + зависимости):
#    записи с префиксом rootfs/ стримятся в tmpfs-корень коробки
poler-engine --poler-box box.poler --box-entry rootfs/bin/hello \
  --box-rss-mb 64 --box-cpu-s 5 --box-tmpfs-mb 32

# В) движок внутри коробки (циклическая обёртка) считает спектр:
poler-engine --poler-box engine_box.poler --box-entry rootfs/bin/poler-engine \
  --box-arg=--exec --box-arg='calc eigen_sturm(tridiag(2,-1,256))' \
  --box-arg=--json --box-rss-mb 512 --box-cpu-s 30 --box-tmpfs-mb 64
```

Правила интерфейса:

* `--box-entry` — имя записи из `--poler-list`; аргументы payload — через
  `--box-arg=...` (с `=`, иначе флаги payload съест clap);
* маппинг по умолчанию `rootfs/` → `/`; свой — через
  `--box-map PREFIX:DIR` (пустой префикс `":/dir"` = весь архив);
* лимиты: `--box-rss-mb`, `--box-cpu-s`, `--box-tmpfs-mb`;
  `--box-no-isolate` — запуск без namespaces (только для отладки);
* после завершения печатается JSON-отчёт: exit_code, kill_reason,
  peak_tree_rss_kb, isolation (namespaces/pivot_root/seccomp).

Сборка `engine_box.poler` для случая (В): rootfs с бинарником движка и его
библиотеками (`ldd target/release/poler-engine`): `rootfs/bin/poler-engine`,
`rootfs/lib64/ld-linux-x86-64.so.2`, `rootfs/lib/x86_64-linux-gnu/{libc,libm,libgcc_s}.so*`
— затем tar.gz → `--stream-file`, как в §6.1.

### 6.3 Приёмка после настройки (проверено на v0.62.0)

1. Статический payload в коробке: `exit_code: 0`, в отчёте —
   `userns_via_helper: true`, `pivot_root: true`, seccomp whitelist.
2. Губернатор: payload, выделяющий 384 МБ при `--box-rss-mb 64`, убит
   на пике ~123 МБ — `kill_reason: "rss_limit"`, exit 137.
3. Циклическая обёртка: движок из архива в коробке возвращает
   `eigen_sturm(tridiag(2,-1,256))` `ok:true` за ~32 мс — λ 0.000149…3.999850,
   как и на хосте.

Под капотом: трёхпроцессная цепочка `poler-engine` (губернатор RSS/CPU
дерева, опрос 50 мс, SIGKILL + JSON-отчёт) → `unshare -Ur` (userns+map,
обход политики ядра) → stage2 (mnt/pid/net/ipc/uts-ns, tmpfs-rootfs
**из архива**, pivot_root) → payload как PID 1: `execveat(memfd_create)`
— на диск не пишется ни байта; seccomp — белый список системных вызовов.

## 7. Timings (реальный слабый хост: 2 vCPU, 4 ГБ RAM, `-j1`)

| Этап | Время |
|---|---|
| `cargo check --workspace` | ~2 м 16 с |
| `cargo test -p poler-engine --lib eigen_sturm` | 2 passed / 0 failed (0.30 с на прогоны) |
| Release-сборка `-p poler-engine -j1` (тёплый `target/`) | **8 м 48 с** |
| `rustc -O poler-api.rs` (шлюз) | ~10 с |
| `--stream-file`: 19 МБ бинарник → `.poler` | 997 мс (ratio 0.44, 78 чанков, peak RSS 27 МБ) |
| `--stream-file`: rootfs движка (5 файлов, 22 МБ) → `.poler` | 1.35 с (ratio 0.44) |
| poler-box: статический payload (hello) | wall 32 мс, exit 0 |
| poler-box: циклическая обёртка — движок из архива, `eigen_sturm(256)` | 32 мс расчёт, wall 141 мс |
| poler-box: губернатор (hog 384 МБ при лимите 64 МБ) | убит на 123 МБ, `rss_limit` |

Чек-лист на этом же хосте: `eigen_sturm(tridiag(2,-1,256))` — **36.5 мс**
(лимит канона 60 мс), det(T₂₅₆) = 257.0, trace = 512.0, `--schema` — чистый
JSON, `--triune-speak` — без сбоев.

## 8. Траблшутинг

**OOM Killer на линковке** (`dmesg | grep -i 'killed process'`):
1. Убедись, что сборка идёт с `-j1` (§3, шаг 2) — это канон и 90% случаев.
2. Добавьте swap (самый сильный приём для слабых машин):

```bash
sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
# отключить после сборки: sudo swapoff /swapfile && sudo rm /swapfile
```

**CI-требование `--locked`**: если меняли `Cargo.toml` (версия/зависимости),
локально запустите любой `cargo build` — он обновит `Cargo.lock`, и **оба
файла коммитьте вместе** (иначе CI упадёт на `--locked`).

**Место кончилось**: `cargo clean` — `target/` пересоберётся с нуля.

**Сеть недоступна для crates.io**: `cargo build --offline` работает, если
зависимости уже закешированы в `~/.cargo` хотя бы раз.

---

> Документ добавлен в v0.62.0 (AI-API реформа), §6 переписан под
> испытанный интерфейс архиватора и poler-box (v0.62.0). Канон протокола
> сборки — владельческий: `-j1`, профиль не трогать, чек-лист из трёх
> пунктов обязателен после каждой пересборки.
