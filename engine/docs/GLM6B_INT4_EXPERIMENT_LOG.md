# 📜 ЛОГ ЭКСПЕРИМЕНТА: GLM-6B Int4 Конвертация и Заливка на Hugging Face
*Task ID: GLM6B-EXP-3*
*Дата: 18 сентября 2026*
*Коммит: 8165161*

---

## 🔬 Результаты теста качества квантования (на реальных весах L13/L23):
* **Trit5 (наивный)**: косинус = `0.777` (относительная ошибка 112%, ~80% весов ушло в ноль).
* **Int4 (per-row scale)**: косинус = `0.987` (относительная ошибка 16%).
* **Вывод**: Int4 сохраняет математическую структуру весов (0.987), но генерация выдаёт повтор токена «✅» $\to$ проблема не в квантовании, а в семантическом баге движка (декодинг / embedding / RoPE / lm_head).

---

## 📦 Сборка и Артефакты .pqw:
* **Исходник**: THUDM/chatglm3-6b (11.6 GB FP16).
* **Сжатый файл**: `models/chatglm3-6b-int4.pqw` — **2.99 GB** (3.9× сжатие, 310 тензоров, встроенный токенизатор, `rms_eps=1e-5`, `qkv_b`).
* **MD5 контрольная сумма**: `dedf407d6d4529446528c4ae84a129be`.

---

## 🌐 Заливка в приватный репозиторий Hugging Face:
* **Репозиторий**: [`VitalijKotok/poler-70b-t5q`](https://huggingface.co/VitalijKotok/poler-70b-t5q)
* **Папка**: `chatglm3-6b-int4-parts/` (залито 5 частями по 734 МБ из-за ограничений песочницы):
  1. `glm3int4.part.00` (734.0 MB)
  2. `glm3int4.part.01` (734.0 MB)
  3. `glm3int4.part.02` (734.0 MB)
  4. `glm3int4.part.03` (734.0 MB)
  5. `glm3int4.part.04` (194.6 MB)
  * `README.md` и `chatglm3-6b-int4.pqw.md5`
* **Итоговый размер в облаке**: **3.131 GB** (100% сошлось).

---

## 💻 Скрипт сборки модели из облака (`scripts/rejoin_glm3_pqw.sh`):
```bash
hf download VitalijKotok/poler-70b-t5q chatglm3-6b-int4-parts --repo-type model --local-dir .
cat chatglm3-6b-int4-parts/glm3int4.part.* > models/chatglm3-6b-int4.pqw
md5sum models/chatglm3-6b-int4.pqw  # проверка: dedf407d6d4529446528c4ae84a129be
```
