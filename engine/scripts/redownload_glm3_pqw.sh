#!/bin/bash
# Докачка chatglm3-6b-int4.pqw с HF (5 частей по ~700MB) с потоковой сборкой.
# Пик диска: итоговый файл (3.13GB) + одна часть (0.7GB).
set -e
# Токен HF берём из окружения: export HF_TOKEN=hf_...
TOKEN="${HF_TOKEN:?Нужно export HF_TOKEN=... перед запуском}"
REPO="VitalijKotok/poler-70b-t5q"
DIR="chatglm3-6b-int4-parts"
MODELS="/home/z/my-project/poler-engine/models"
OUT="$MODELS/chatglm3-6b-int4.pqw"
EXPECT_MD5="dedf407d6d4529446528c4ae84a129be"
EXPECT_SIZE="3130642823"

mkdir -p "$MODELS"

# Resume: если файл уже частично скачан — начинаем с чистого листа.
rm -f "$OUT"

for i in 00 01 02 03 04; do
  echo "== часть $i =="
  curl -sL --retry 3 -H "Authorization: Bearer $TOKEN" \
    "https://huggingface.co/$REPO/resolve/main/$DIR/glm3int4.part.$i" \
    -o "$MODELS/.dl.part.$i"
  cat "$MODELS/.dl.part.$i" >> "$OUT"
  rm -f "$MODELS/.dl.part.$i"
  echo "   накоплено: $(stat -c%s "$OUT") байт"
done

SIZE=$(stat -c%s "$OUT")
if [ "$SIZE" != "$EXPECT_SIZE" ]; then
  echo "РАЗМЕР НЕ СХОДИТСЯ: $SIZE != $EXPECT_SIZE"; exit 1
fi
MD5=$(md5sum "$OUT" | cut -d' ' -f1)
echo "md5: $MD5 (ожидался $EXPECT_MD5)"
if [ "$MD5" != "$EXPECT_MD5" ]; then
  echo "MD5 НЕ СХОДИТСЯ"; exit 1
fi
echo "$MD5  $OUT" > "$MODELS/chatglm3-6b-int4.pqw.md5"
echo "OK: модель собрана и верифицирована"
