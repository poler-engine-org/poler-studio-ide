#!/bin/bash
# Посекционная заливка .pqw на HF (LFS, без Xet): extract -> upload -> delete
set -e
SRC="$1"; REPO="VitalijKotok/poler-70b-t5q"; DIR="chatglm3-6b-int4-parts"
SIZE=$(stat -c%s "$SRC"); PART=734003200  # 700MB
NBLOCKS=$(( (SIZE + PART - 1) / PART ))
START=${2:-0}
for (( i=START; i<NBLOCKS; i++ )); do
  P=$(printf "glm3int4.part.%02d" $i)
  echo "=== часть $((i+1))/$NBLOCKS -> $P ==="
  dd if="$SRC" of="pqw_parts/$P" bs=$PART skip=$i count=1 status=none
  timeout 500 hf upload "$REPO" "pqw_parts/$P" "$DIR/$P" --repo-type model \
    --commit-message "GLM3-6B Int4 .pqw part $((i+1))/$NBLOCKS [НЕ ЗАКОНЧЕНО]"
  rm -f "pqw_parts/$P"
done
echo "ALL PARTS DONE"
