# Исследовательский архив: Open-Source Omni Vision & Real-Time Streaming Reference

> **СТАТУС И НАЗНАЧЕНИЕ:**  
> **ПЕРЕОСМЫСЛИТЬ · УСОВЕРШЕНСТВОВАТЬ · ПЕРЕПИСАТЬ · УЛУЧШИТЬ**  
> Вспомогательные внешние референсы для разработки собственного суверенного зрения ядра **POLER[Ψ]** (M6 / Resident Cognitive Stream).  
> *Данный архив изолирован в `docs/research-archive/` и не входит в производственный код ядра `src/`.*

---

## 0. UE-референсы в родном контейнере `ue_reference.poler` (v0.55.0)

Заголовки Unreal Engine (коммит 266477d: LevelTick.cpp, Actor.h,
ActorComponent.h, ActorChannel.h, GarbageCollection.h,
RenderGraphBuilder.h) хранятся **только** внутри суверенного
`.poler`-контейнера — без распакованных копий на диске.

Протокол «полер-бокс» (директива владельца):

```bash
poler-engine --archive-list ue_reference.poler            # листинг O(1)
poler-engine --grep "RunTickGroup" docs/research-archive/ --archives  # grep внутри
poler-engine --poler-cat ue_reference.poler --file ue_reference/Actor.h  # чтение записи
poler-engine --poler-patch ue_reference.poler --manifest notes.json     # CoW-патч внутри
```

Аналитика — в записи `ue_reference/POLER_NOTES.md` **внутри** архива
(добавлена in-place патчем). Сжатие ratio ≈ 0.21.

---

## 1. Содержимое архива `omni_vision_streaming_references.tar.gz`

1. **`mini-omni` / `mini-omni2` (GPT-Omni)**:
   - Первая открытая сквозная (End-to-End) реализация Full-Duplex стриминга аудио и видео токенов.
   - Архитектура прямого проецирования визуальных и звуковых латентов в единый Transformer-декодер без промежуточного OCR/STT.
2. **`MiniCPM-o 4.5` (OpenBMB / Tsinghua / OmniLMM)**:
   - Локальная модель со сквозным зрением и голосом в реальном времени.
   - Полнодуплексный режим (одновременное восприятие видеопотока и генерация ответа).
   - Оптимизации для on-device инференса (`llama.cpp-omni`).
3. **`vllm-omni` (vLLM Project)**:
   - Архитектура оркестрации и низколатентного сервинга (sub-100ms) с непрерывным батчингом и кольцевым буфером KV-кэша видео/документов.

---

## 2. Задачи переосмысления и адаптации под POLER[Ψ]:

- [ ] **Отказ от Python/PyTorch зависимостей**: переписать ключевые механизмы потокового внимания и токенизации на нативный **Rust / Zig / PQC AVX2/SIMD** стек.
- [ ] **Связка с проектором $\Pi_\Lambda$ и ротором $J = A - A^T$**: направить непрерывный поток внимания не на слепые пиксели, а на топологические инварианты текста и формул ($dp/dt$, $\hat{H}\Psi = 0$).
- [ ] **Resident In-Memory KV-Cache**: удержание смысловой карты документа в RAM для мгновенного доступа (`< 3 мс`) без перезапуска процессов.
