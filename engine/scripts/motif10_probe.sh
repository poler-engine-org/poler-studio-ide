#!/usr/bin/env bash
# 10-нейронный ГАМК-мотив: 47410 + топ-входы + хаб (сессия 4)
set -u
CSR=/home/z/my-project/poler-engine-src/docs/flywire-connectome/flywire_v783_core.csr.zst
NODES=/home/z/my-project/poler-engine-src/docs/flywire-connectome/flywire_v783_nodes.bin
OUT=/home/z/my-project/download/experiments/fly_motif10_edges.txt
NEURONS=(79529 47410 55498 101516 74111 106315 11246 52072 23350 34109)
NAMES=("хаб" "лейтенант-ГАМК" "окт-интегратор" "ГАМК-a" "АХ-a" "АХ-b" "ГАМК-b" "ГАМК-c" "ГАМК-d" "ДА-модулятор")

echo "Мотив ГАМК-ганга: 10 нейронов" > "$OUT"
for i in "${!NEURONS[@]}"; do echo "  [$i] ${NEURONS[$i]} — ${NAMES[$i]}" >> "$OUT"; done
echo "Пробы 45 пар:" >> "$OUT"

EDGES_CSV=/home/z/my-project/scripts/motif10_edges.csv
: > "$EDGES_CSV"
N=${#NEURONS[@]}
for ((i=0; i<N; i++)); do
  for ((j=i+1; j<N; j++)); do
    U=${NEURONS[$i]}; V=${NEURONS[$j]}
    R=$(poler-engine --connectome "$CSR" --connectome-nodes "$NODES" --connectome-edge "$U:$V" 2>/dev/null)
    Wuv=$(echo "$R" | grep "^Ребро" | grep -oE "w=[0-9]+" | head -1 | cut -d= -f2)
    Wvu=$(echo "$R" | grep "^Обратное" | grep -oE "w=[0-9]+" | head -1 | cut -d= -f2)
    Wuv=${Wuv:-0}; Wvu=${Wvu:-0}
    TOTAL=$(( Wuv + Wvu ))
    if [ "$TOTAL" -gt 0 ]; then
      echo "$i,$j,$Wuv,$Wvu,$TOTAL" >> "$EDGES_CSV"
      echo "  [$i]-$j]: w_uv=$Wuv w_vu=$Wvu total=$TOTAL" >> "$OUT"
    fi
  done
done
echo "" >> "$OUT"
echo "рёбер: $(wc -l < "$EDGES_CSV") из 45 пар; CSV: $EDGES_CSV" >> "$OUT"
cat "$EDGES_CSV"
echo "---"; tail -3 "$OUT"
