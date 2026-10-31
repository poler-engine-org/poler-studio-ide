#!/bin/bash
# Сборка models/chatglm3-6b-int4.pqw из частей на HuggingFace
# hf download VitalijKotok/poler-70b-t5q chatglm3-6b-int4-parts --repo-type model --local-dir .
cat chatglm3-6b-int4-parts/glm3int4.part.00 chatglm3-6b-int4-parts/glm3int4.part.01 \
    chatglm3-6b-int4-parts/glm3int4.part.02 chatglm3-6b-int4-parts/glm3int4.part.03 \
    chatglm3-6b-int4-parts/glm3int4.part.04 > models/chatglm3-6b-int4.pqw
md5sum -c <(echo "$(cat models/chatglm3-6b-int4.pqw.md5)")
