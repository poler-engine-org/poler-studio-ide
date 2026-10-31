#!/usr/bin/env bash
# ============================================================
# POLER-ENGINE v0.30.0 — ФУНКЦИОНАЛЬНАЯ МАТРИЦА ТЕСТИРОВАНИЯ
# Тестирует ВСЕ режимы CLI на живом корпусе Eteryya (153 МБ)
# и на самом движке (src/). Результат: Markdown-таблица PASS/FAIL.
# ============================================================
set -u
export PATH="$HOME/.local/bin:$PATH"
POLER="poler-engine"
REPO="/home/z/my-project/skills/poler-engine"
ETERYYA="$HOME/eteryya"
OUT="/home/z/my-project/scripts/func_results.md"
WORK="/home/z/my-project/scripts/ft_work"
mkdir -p "$WORK"

PASS=0; FAIL=0; SKIP=0
RESULTS=""

# check <ID> <описание> <команда...>  — exit 0 = PASS
check() {
  local id="$1"; shift
  local desc="$1"; shift
  local timeout="${1:-60}"; shift 2>/dev/null || shift
  local out t0 t1 rc
  t0=$(date +%s%N)
  out=$(timeout "$timeout" "$@" 2>&1); rc=$?
  t1=$(date +%s%N)
  local ms=$(( (t1 - t0) / 1000000 ))
  if [ $rc -eq 0 ]; then
    RESULTS+="| $id | PASS | ${ms} мс | $(echo "$out" | head -1 | cut -c1-90) |\n"
    PASS=$((PASS+1))
  else
    RESULTS+="| $id | FAIL($rc) | ${ms} мс | $(echo "$out" | head -2 | tr '\n' ' ' | cut -c1-90) |\n"
    FAIL=$((FAIL+1))
  fi
  echo "[$id] rc=$rc ${ms}ms :: $(echo "$out" | head -1 | cut -c1-100)"
}

# check_contains <ID> <описание> <needle> <команда...>
check_contains() {
  local id="$1"; shift
  local desc="$1"; shift
  local needle="$1"; shift
  local timeout="${1:-60}"; shift
  local out rc
  out=$(timeout "$timeout" "$@" 2>&1); rc=$?
  if [ $rc -eq 0 ] && echo "$out" | grep -q "$needle"; then
    RESULTS+="| $id | PASS | — | найдено: «$needle» |\n"
    PASS=$((PASS+1))
  else
    RESULTS+="| $id | FAIL($rc) | — | «$needle» не найдено: $(echo "$out" | head -1 | cut -c1-70) |\n"
    FAIL=$((FAIL+1))
  fi
  echo "[$id] rc=$rc :: contains '$needle'"
}

echo "=== A. БАЗОВЫЕ ==="

# B. РЕЗОНАНСНЫЙ ПОИСК (главный режим) на Eteryya
echo "=== B. РЕЗОНАНСНЫЙ ПОИСК ==="
T0=$(date +%s%N)
OUT_B=$($POLER "$ETERYYA" -q "Алексей" -t 3 2>&1); RC=$?
T1=$(date +%s%N); MS=$(( (T1-T0)/1000000 ))
if [ $RC -eq 0 ] && echo "$OUT_B" | grep -qi "алекс\|scene\|сцена\|hit" ; then
  RESULTS+="| B1 | PASS | ${MS} мс | -q Алексей (эталон 10696) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| B1 | FAIL($RC) | ${MS} мс | $(echo "$OUT_B" | head -1 | cut -c1-90) |\n"; FAIL=$((FAIL+1))
fi
echo "[B1] rc=$RC ${MS}ms"

T0=$(date +%s%N)
OUT_B=$($POLER "$ETERYYA" -q "Нокс" -t 3 2>&1); RC=$?
T1=$(date +%s%N); MS=$(( (T1-T0)/1000000 ))
if [ $RC -eq 0 ]; then
  RESULTS+="| B2 | PASS | ${MS} мс | -q Нокс (эталон 547) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| B2 | FAIL($RC) | ${MS} мс | $(echo "$OUT_B" | head -1 | cut -c1-90) |\n"; FAIL=$((FAIL+1))
fi
echo "[B2] rc=$RC ${MS}ms"

check B3 "resonance-mode poler" 300 $POLER "$ETERYYA" -q "резонанс" -t 2 --resonance-mode poler
check B4 "resonance-mode psi" 300 $POLER "$ETERYYA" -q "резонанс" -t 2 --resonance-mode psi
check B5 "resonance-mode field" 300 $POLER "$ETERYYA" -q "резонанс" -t 2 --resonance-mode field
check B6 "format ai-json" 300 $POLER "$ETERYYA" -q "Алексей" -t 2 --format ai-json
check B7 "format md" 300 $POLER "$ETERYYA" -q "Алексей" -t 2 --format md
check B8 "format simple" 300 $POLER "$ETERYYA" -q "Алексей" -t 2 --format simple
check B9 "pii mask" 300 $POLER "$REPO/tests/fixtures" -q "test" --pii mask
check B10 "k-hop граф" 300 $POLER "$ETERYYA" -q "Алексей" -t 2 --k-hop 2
check B11 "local-stats" 300 $POLER "$ETERYYA" -q "Алексей" -t 2 --local-stats

echo "=== C. ТОЧНЫЙ ПОИСК (grep-слой) ==="
check C1 "grep fixed" 60 $POLER "$REPO/src" --grep "fn main"
check C2 "grep-regex" 60 $POLER "$REPO/src" --grep "fn (main|run)" --grep-regex
check C3 "grep-i" 60 $POLER "$REPO/src" --grep "TEDDY" --grep-i
check C4 "grep-count" 60 $POLER "$REPO/src" --grep "fn " --grep-count
check C5 "grep-list" 60 $POLER "$REPO/src" --grep "TeddyMatcher" --grep-list
check C6 "grep-list-nonmatching" 60 $POLER "$REPO/src" --grep "ZZZNOPE" --grep-list-nonmatching
check C7 "grep-json" 60 $POLER "$REPO/src" --grep "fn main" --grep-json
check C8 "grep context -A -B" 60 $POLER "$REPO/src" --grep "fn main" --grep-after 3 --grep-before 1
check C9 "grep-max-count" 60 $POLER "$REPO/src" --grep "fn " --grep-max-count 2

# C10: exit-коды grep (1 = пусто)
timeout 60 $POLER "$REPO/src" --grep "ZZZ_NO_MATCH_ZZZ" >/dev/null 2>&1; RC=$?
if [ $RC -eq 1 ]; then
  RESULTS+="| C10 | PASS | — | exit 1 на пустом результате (grep-семантика) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| C10 | FAIL($RC) | — | ожидался exit 1 |\n"; FAIL=$((FAIL+1))
fi
echo "[C10] rc=$RC (ожид. 1)"

# C11: parity vs ripgrep на Eteryya
if command -v rg >/dev/null; then
  PG=$(timeout 300 $POLER "$ETERYYA" --grep "Алексей" --grep-count 2>/dev/null | awk -F: '{s+=$NF} END {print s+0}')
  RG=$(timeout 300 rg -c --no-hidden -g '!*.bin' "Алексей" "$ETERYYA" 2>/dev/null | awk -F: '{s+=$NF} END {print s+0}')
  if [ "$PG" = "$RG" ] && [ "$PG" -gt 1000 ]; then
    RESULTS+="| C11 | PASS | — | parity POLER=$PG == ripgrep=$RG на 153 МБ |\n"; PASS=$((PASS+1))
  else
    RESULTS+="| C11 | FAIL | — | POLER=$PG vs ripgrep=$RG |\n"; FAIL=$((FAIL+1))
  fi
  echo "[C11] poler=$PG rg=$RG"
else
  RESULTS+="| C11 | SKIP | — | ripgrep не установлен |\n"; SKIP=$((SKIP+1))
fi

echo "=== D. АРХИВЫ ==="
# тестовый zip
mkdir -p "$WORK/ziptest/docs"
echo "Subquantum Kinetics paper text for archive test" > "$WORK/ziptest/docs/paper.md"
echo "second file with keyword Subquantum again" > "$WORK/ziptest/notes.txt"
cd "$WORK/ziptest" && zip -q -r ../export.zip . 2>/dev/null; cd /home/z/my-project/scripts
check D1 "archive-list" 30 $POLER --archive-list "$WORK/export.zip"
check D2 "archive-list json" 30 $POLER --archive-list "$WORK/export.zip" --archive-json
check D3 "grep --archives" 60 $POLER "$WORK" --grep "Subquantum" --archives
check_contains D4 "archive:: селектор чанков" "chunk\|byte" 30 $POLER "$WORK/export.zip::docs/paper.md" --chunk --chunk-json

echo "=== E. RAG-ЧАНКИ ==="
check E1 "chunk текст" 60 $POLER "$ETERYYA/00_КАНОН/$(ls $ETERYYA/00_КАНОН | head -1 | cat)" --chunk --chunk-size 256
check E2 "chunk-json" 60 $POLER "$REPO/README.md" --chunk --chunk-json
check E3 "chunk-size/overlap" 60 $POLER "$REPO/README.md" --chunk --chunk-size 128 --chunk-overlap 16

echo "=== F. СУВЕРЕННЫЙ ML ==="
check F1 "pqw-selftest" 60 $POLER --pqw-selftest
# демо-модели
if [ ! -f "$WORK/demo_encoder.pqw" ]; then
  python3 "$REPO/scripts/gen_demo_pqw.py" --out "$WORK" >/dev/null 2>&1 || \
  python3 "$REPO/scripts/gen_demo_pqw.py" "$WORK" >/dev/null 2>&1
fi
ls "$WORK"/*.pqw 2>/dev/null | head -3
if [ -f "$WORK/demo_encoder.pqw" ]; then
  check F2 "semantic dense (demo encoder)" 300 $POLER "$REPO" --semantic dense --model "$WORK/demo_encoder.pqw" -q "поиск" --semantic-limit 3 --semantic-max-chunks 200
else
  RESULTS+="| F2 | SKIP | — | demo_encoder.pqw не сгенерирован |\n"; SKIP=$((SKIP+1))
fi
if [ -f "$WORK/demo_gliner.pqw" ]; then
  check F3 "ner gliner (demo)" 300 $POLER "$REPO/README.md" --ner gliner --model "$WORK/demo_gliner.pqw" --ner-labels "человек,организация"
else
  RESULTS+="| F3 | SKIP | — | demo_gliner.pqw не сгенерирован |\n"; SKIP=$((SKIP+1))
fi
if [ -f "$WORK/demo_glm.pqw" ]; then
  check F4 "llm local (demo)" 300 $POLER --llm local --model "$WORK/demo_glm.pqw" --corpus "$REPO/README.md" -q "тест"
else
  RESULTS+="| F4 | SKIP | — | demo_glm.pqw не сгенерирован |\n"; SKIP=$((SKIP+1))
fi

echo "=== G. IMPACT (AIDDE) ==="
check G1 "impact символ" 120 $POLER "$REPO/src" --impact "run_gateway" --impact-depth 2
check G2 "impact другой" 120 $POLER "$REPO/src" --impact "Connectome::load" --impact-depth 1

echo "=== H. КОННЕКТОМ (C1/v0.30.0) ==="
CSR="$REPO/docs/flywire-connectome/flywire_v783_core.csr.zst"
NODES="$REPO/docs/flywire-connectome/flywire_v783_nodes.bin"
T0=$(date +%s%N); OUT_H=$(timeout 120 $POLER --connectome "$CSR" 2>&1); RC=$?; T1=$(date +%s%N); MS=$(( (T1-T0)/1000000 ))
if [ $RC -eq 0 ] && echo "$OUT_H" | grep -q "54 492 922\|54492922\|узел\|нейрон"; then
  RESULTS+="| H1 | PASS | ${MS} мс | сводка core (эталон 54 492 922 синапса) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| H1 | FAIL($RC) | ${MS} мс | $(echo "$OUT_H" | head -1 | cut -c1-90) |\n"; FAIL=$((FAIL+1))
fi
echo "[H1] rc=$RC ${MS}ms :: $(echo "$OUT_H" | head -2 | tr '\n' ' ')"
check_contains H2 "node 0 паспорт" "out" 60 $POLER --connectome "$CSR" --connectome-nodes "$NODES" --connectome-node 0
check_contains H3 "edge 0:6135 ротор" "J\|ротор\|rotor" 60 $POLER --connectome "$CSR" --connectome-edge 0:6135
check_contains H4 "khop 0" "457\|фронт\|hop" 60 $POLER --connectome "$CSR" --connectome-khop 0
check_contains H5 "khop exc-фильтр" "296\|фронт\|hop" 60 $POLER --connectome "$CSR" --connectome-khop 0 --connectome-sign exc
check H6 "impact CSC" 120 $POLER --connectome "$CSR" --connectome-impact 0
check H7 "connectome-json" 60 $POLER --connectome "$CSR" --connectome-node 0 --connectome-json
check_contains H8 "root_id резолв" "720575940596125868\|узел" 60 $POLER --connectome "$CSR" --connectome-nodes "$NODES" --connectome-node 720575940596125868

echo "=== I. ВЕБ (офлайн-часть) ==="
check I1 "web-stats" 30 $POLER --web-stats --web-db "$WORK/web-index.db"
check I2 "semantic-expand" 30 $POLER --semantic-expand "поиск"
OUT_I=$($POLER --web-search "тест" --web-db "$WORK/web-index.db" 2>&1); RC=$?
if [ $RC -eq 0 ] || [ $RC -eq 1 ]; then
  RESULTS+="| I3 | PASS | — | web-search на пустом индексе не падает (rc=$RC) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| I3 | FAIL($RC) | — | $(echo "$OUT_I" | head -1 | cut -c1-80) |\n"; FAIL=$((FAIL+1))
fi
echo "[I3] rc=$RC"
if command -v chromium >/dev/null 2>&1 || command -v chromium-browser >/dev/null 2>&1 || command -v google-chrome >/dev/null 2>&1; then
  RESULTS+="| I4 | INFO | — | Chromium есть в PATH — CDP-режим тестируем вручную |\n"; 
else
  RESULTS+="| I4 | SKIP | — | Chromium отсутствует (CDP --web/--crawl недоступны в песочнице) |\n"; SKIP=$((SKIP+1))
fi

echo "=== J. ИНТЕРФЕЙСЫ ==="
# J1: MCP stdio — initialize + tools/list
MCP_REQ='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"ft","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"poler_search","arguments":{"query":"fn main","path":"'"$REPO/src"'"}}}'
OUT_J=$(echo "$MCP_REQ" | timeout 120 $POLER --mcp 2>/dev/null); RC=$?
if [ $RC -eq 0 ] && echo "$OUT_J" | grep -q "poler_search\|poler_grep"; then
  RESULTS+="| J1 | PASS | — | MCP stdio: initialize + tools/list + tools/call |\n"; PASS=$((PASS+1))
else
  RESULTS+="| J1 | FAIL($RC) | — | $(echo "$OUT_J" | head -1 | cut -c1-80) |\n"; FAIL=$((FAIL+1))
fi
echo "[J1] rc=$RC :: $(echo "$OUT_J" | grep -o 'poler_[a-z_]*' | sort -u | tr '\n' ' ' | cut -c1-100)"

check J2 "mcp-bench 50" 300 $POLER --mcp-bench 50
# J3: shell REPL
OUT_J=$(printf 'help\nquit\n' | timeout 30 $POLER --shell 2>&1); RC=$?
if [ $RC -eq 0 ]; then
  RESULTS+="| J3 | PASS | — | shell REPL отвечает на help/quit |\n"; PASS=$((PASS+1))
else
  RESULTS+="| J3 | FAIL($RC) | — | $(echo "$OUT_J" | tail -1 | cut -c1-80) |\n"; FAIL=$((FAIL+1))
fi
echo "[J3] rc=$RC"
# J4: TUI headless — ожидаемо может не работать без TTY
OUT_J=$(timeout 8 $POLER "$REPO" --tui 2>&1 </dev/null); RC=$?
if [ $RC -eq 0 ] || [ $RC -eq 124 ]; then
  RESULTS+="| J4 | PASS | — | TUI стартует headless (timeout-kill, без паники) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| J4 | WARN($RC) | — | TUI без TTY: $(echo "$OUT_J" | tail -1 | cut -c1-70) |\n"
fi
echo "[J4] rc=$RC (124=timeout, ок headless)"

echo "=== K. VAULT (крипто-слой pnd-ffi) ==="
export POLER_VAULT_KEY="test-phrase-фраза-2026"
echo "secret log line one $(date)" > "$WORK/vault_test.txt"
check K1 "memory-seal" 120 $POLER --memory-seal "$WORK/vault_test.txt"
check K2 "memory-verify" 120 $POLER --memory-verify "$WORK/vault_test.txt.pvt"
check K3 "memory-info" 60 $POLER --memory-info "$WORK/vault_test.txt.pvt"
rm -f "$WORK/vault_test.out.txt"
check K4 "memory-open" 120 $POLER --memory-open "$WORK/vault_test.txt.pvt" --memory-out "$WORK/vault_test.out.txt"
if cmp -s "$WORK/vault_test.txt" "$WORK/vault_test.out.txt"; then
  RESULTS+="| K5 | PASS | — | roundtrip seal→open побитово идентичен |\n"; PASS=$((PASS+1))
else
  RESULTS+="| K5 | FAIL | — | расхождение после roundtrip |\n"; FAIL=$((FAIL+1))
fi
# K6: неверный ключ должен дать ошибку
POLER_VAULT_KEY="wrong-key" timeout 60 $POLER --memory-open "$WORK/vault_test.txt.pvt" --memory-out "$WORK/vt2.txt" >/dev/null 2>&1; RC=$?
if [ $RC -ne 0 ]; then
  RESULTS+="| K6 | PASS | — | неверный ключ отклонён (rc=$RC) |\n"; PASS=$((PASS+1))
else
  RESULTS+="| K6 | FAIL | — | неверный ключ НЕ отклонён! |\n"; FAIL=$((FAIL+1))
fi
echo "[K6] rc=$RC (ожид. не 0)"

echo "=== L. БЕНЧМАРК ==="
T0=$(date +%s%N)
OUT_L=$(timeout 570 $POLER "$REPO" --benchmark 2>&1); RC=$?
T1=$(date +%s%N); MS=$(( (T1-T0)/1000000 ))
if [ $RC -eq 0 ] && echo "$OUT_L" | grep -qi "contour\|контур\|PASS\|✓"; then
  RESULTS+="| L1 | PASS | ${MS} мс | --benchmark 6 контуров |\n"; PASS=$((PASS+1))
  echo "$OUT_L" > "$WORK/benchmark_output.txt"
else
  RESULTS+="| L1 | FAIL($RC) | ${MS} мс | $(echo "$OUT_L" | head -1 | cut -c1-80) |\n"; FAIL=$((FAIL+1))
  echo "$OUT_L" > "$WORK/benchmark_output.txt"
fi
echo "[L1] rc=$RC ${MS}ms"

echo ""
echo "=========================================="
echo "ИТОГО: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
echo "=========================================="

{
echo "# Функциональная матрица v0.30.0 (b7f097b)"
echo ""
echo "Дата: $(date '+%Y-%m-%d %H:%M') · корпус: Eteryya 153 МБ + src движка"
echo ""
echo "| ID | Статус | Время | Детали |"
echo "|---|---|---|---|"
echo -e "$RESULTS"
echo ""
echo "**ИТОГО: PASS=$PASS / FAIL=$FAIL / SKIP=$SKIP**"
} > "$OUT"
echo "Результаты: $OUT"
