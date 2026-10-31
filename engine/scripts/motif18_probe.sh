#!/usr/bin/env bash
# Извлечение 18-нейронного мотива из коннектома мухи (сессия 4, эксперимент B)
# Хаб 79529 + 17 топ-партнёров → все парные рёбра через --connectome-edge
set -u
CSR=/home/z/my-project/poler-engine-src/docs/flywire-connectome/flywire_v783_core.csr.zst
NODES=/home/z/my-project/poler-engine-src/docs/flywire-connectome/flywire_v783_nodes.bin
OUT=/home/z/my-project/download/experiments/fly_motif18_edges.txt

# 18 нейронов: хаб + 17 партнёров (топ по весу)
NEURONS=(79529 47410 13871 19862 43346 3746 19804 27491 113804 48759 66395 68059 81371 85387 121406 132504 134353 20789)

echo "Мотив: ${#NEURONS[@]} нейронов (хаб 79529 + 17 партнёров)" > "$OUT"
echo "Пробы --connectome-edge U:V по всем 153 неупорядоченным парам" >> "$OUT"

EDGES_CSV=/home/z/my-project/scripts/motif18_edges.csv
: > "$EDGES_CSV"
N=${#NEURONS[@]}
for ((i=0; i<N; i++)); do
  for ((j=i+1; j<N; j++)); do
    U=${NEURONS[$i]}; V=${NEURONS[$j]}
    R=$(poler-engine --connectome "$CSR" --connectome-nodes "$NODES" --connectome-edge "$U:$V" 2>/dev/null)
    # формат: "Ребро U -> V: ... w=NNN ..." / "Обратное V -> U: ... w=NNN ..."
    Wuv=$(echo "$R" | grep "^Ребро" | grep -oE "w=[0-9]+" | head -1 | cut -d= -f2)
    Wvu=$(echo "$R" | grep "^Обратное" | grep -oE "w=[0-9]+" | head -1 | cut -d= -f2)
    Wuv=${Wuv:-0}; Wvu=${Wvu:-0}
    TOTAL=$(( ${Wuv:-0} + ${Wvu:-0} ))
    if [ "$TOTAL" -gt 0 ]; then
      echo "$i,$j,$Wuv,$Wvu,$TOTAL" >> "$EDGES_CSV"
      echo "пара $U:$V  ($i,$j)  w_uv=$Wuv w_vu=$Wvu total=$TOTAL" >> "$OUT"
    fi
  done
done
echo "" >> "$OUT"
E=$(wc -l < "$EDGES_CSV")
echo "рёбер найдено: $E (из 153 возможных пар)" >> "$OUT"
echo "CSV: $EDGES_CSV"
tail -5 "$OUT"
