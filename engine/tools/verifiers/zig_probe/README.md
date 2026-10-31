# zig_probe — регенерация golden-векторов PND v8.2 из РЕАЛЬНОГО Zig-ядра

MVR-v3, фаза побитовой сверки (цикл A-финал; обновлено в M4 под ядро
v8.2 с P0-фиксами аудита Шнайера). Инструмент воспроизводим: среда
эфемерна, скрипты — в git.

## Что здесь

- `golden_dump.zig` — харнесс: импортирует ядро монорепозитория
  (`os/core/poler_core.zig`) как именованный модуль и печатает
  golden-векторы (phi / pndmix / sbox / mds / lhca / fround / fhalf /
  cipher / drbg / modinv / attractor / gfmul) в текстовый формат.

## Процедура регенерации (нужны: клон монорепо + Zig 0.14.0)

После M4 probe-копия больше не нужна: ядро живёт в `os/core/` и
экспортирует нужные функции как `pub` (mixColumnsPnd, polerFeistelF,
polerFeistelFHalf, ctGf256Mul, invMixColumnsPnd).

```bash
cd /path/to/poler-engine                  # корень монорепозитория
export ZIG_BIN=/path/to/zig               # Zig 0.14.0

# 1. собственные тесты ядра + C-ABI parity — должны быть зелёными
cd os/core && $ZIG_BIN build test && cd ../..   # 30/30 OK

# 2. сборка и запуск дампа (именованный модуль, из корня монорепо)
$ZIG_BIN run --dep poler_core \
  -Mroot=tools/verifiers/zig_probe/golden_dump.zig \
  -Mpoler_core=os/core/poler_core.zig \
  > tools/verifiers/golden/pnd_v8_golden_54626.txt

# 3. сверка Python-транслитерации с новым дампом
python3 tools/verifiers/verify_pnd_full.py --sections golden   # BIT-FOR-BIT OK

# 4. сверка с коммитным кешем
git diff --stat tools/verifiers/golden/
```

## Кеш

`../golden/pnd_v8_golden_54626.txt` (54 626 векторов) снят с
`os/core/poler_core.zig` PND **v8.2** (P0-фиксы аудита Шнайера:
двухветвевое 256-битное расписание, PolerDrbg) на Zig 0.14.0.
Включает 272 полных шифрования (16 = собственные тестовые векторы
Zig: 4 ключа × 4 ε) с round-trip флагом и 3 000 слов DRBG-потоков
(3 сида × 1000). Исторический кеш v8.1 (до P0, снят с poler-os @
fc3ffa8) доступен в git-истории этого файла.

## История: почему раньше была probe-копия

До M4 приватные функции (mixColumnsPnd, polerFeistelF, ...) были
недоступны из другого файла Zig, и дампер требовал probe-копию ядра с
12 строками pub-алиасов (байт-в-байт идентичность подтверждал diff).
В v8.2 эти функции открыты как `pub` — ядро монорепозитория
импортируется напрямую, шаг с копированием исключён.
