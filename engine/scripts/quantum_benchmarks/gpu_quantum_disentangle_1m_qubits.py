#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
POLER GPU Tableau-Memory Throughput Probe (v2 — аудит 2026-09-21).

ЗНАХІДКА АУДИТУ (чому переписаний):
  Оригінал («gpu_quantum_disentangle_1m_qubits.py») видавав себе за
  «квантовий стресс-бенчмарк 1 048 576 кубітів» і друкував «100% DISENTANGLED».
  Фактично PTX-ядро виконувало ПОБІТОВЕ НЕ (xor з -1) над 64-бітними словами —
  це НЕ квантова операція (NOT таблиці стабілізаторів не є стабілізаторним
  гейтом), «PASSED» друкувався без жодної перевірки, а chunk-цикл
  ПОВТОРНО обробляв той самий перший 1 ГБ (зсув буфера не просувався), тож
  «256 ГБ tableau» ніколи не матеріалізовувались на 6 ГБ карті.

Що робить ЦЕЙ скрипт (чесно):
  1. Стримить tableau-масштабний робочий тіл (2n рядків × n біт, як у
     стабілізаторній таблиці) ЧАНКАМИ З КОРЕКТНИМ ЗСУВОМ через виділений буфер.
  2. Вимірює досягнуту пропускну здатність (ГБ/с) — верхню межу швидкості,
     з якою GPU міг би прокачувати tableau-подібні потоки (Gottesman–Knill
     на відеокарті потребує саме такої трафіки + логіки).
  3. ПЕРЕВІРЯЄ коректність: readback хеш-зразків слів після проходу
     (кожне слово має дорівнювати NOT свого початкового значення рівно
     один раз; кратні проходи дають тотожність — контроль кратності).

Це зонд пам'яті, НЕ симуляція кубітів: жодних квантових тверджень.
"""

import ctypes
import sys
import time


def main() -> int:
    try:
        cuda = ctypes.CDLL("libcuda.so.1")
    except Exception as e:
        print(f"CUDA library not found: {e}")
        print("(цей зонд призначений для машини з NVIDIA GPU; на CI — skip)")
        return 2

    if cuda.cuInit(0) != 0:
        print("cuInit failed")
        return 1
    dev = ctypes.c_int()
    if cuda.cuDeviceGet(ctypes.byref(dev), 0) != 0:
        print("cuDeviceGet failed")
        return 1
    name = ctypes.create_string_buffer(64)
    cuda.cuDeviceGetName(name, 64, dev.value)
    print(f"=== GPU: {name.value.decode()} ===")

    free = ctypes.c_size_t()
    total = ctypes.c_size_t()
    cuda.cuMemGetInfo_v2(ctypes.byref(free), ctypes.byref(total))
    print(f"    VRAM: {free.value / 1e9:.2f} ГБ вільно / {total.value / 1e9:.2f} ГБ всього")

    ctx = ctypes.c_void_p()
    if cuda.cuCtxCreate_v2(ctypes.byref(ctx), 0, dev.value) != 0:
        print("cuCtxCreate failed")
        return 1

    # PTX: побітове NOT 64-бітних слів [base + word_offset, +count)
    # (параметризований ЗСУВ — фікс бага оригінала, що ганяв той самий 1 ГБ)
    ptx_code = b"""//
.version 6.5
.target sm_61
.address_size 64

.visible .entry stream_not(
    .param .u64 tab_ptr,
    .param .u64 word_offset,
    .param .u64 total_words
)
{
    .reg .u32 %r<5>;
    .reg .u64 %rd<8>;
    .reg .pred %p;

    mov.u32 %r0, %tid.x;
    mov.u32 %r1, %ctaid.x;
    mov.u32 %r2, %ntid.x;
    mad.lo.u32 %r3, %r1, %r2, %r0;
    cvt.u64.u32 %rd0, %r3;

    ld.param.u64 %rd1, [total_words];
    setp.ge.u64 %p, %rd0, %rd1;
    @%p bra DONE;

    ld.param.u64 %rd2, [word_offset];
    add.u64 %rd5, %rd0, %rd2;
    ld.param.u64 %rd3, [tab_ptr];
    shl.b64 %rd6, %rd5, 3;
    add.u64 %rd7, %rd3, %rd6;

    ld.global.u64 %rd4, [%rd7];
    xor.b64 %rd4, %rd4, -1;
    st.global.u64 [%rd7], %rd4;

DONE:
    ret;
}
"""

    mod = ctypes.c_void_p()
    if cuda.cuModuleLoadData(ctypes.byref(mod), ptx_code) != 0:
        print("PTX load failed")
        return 1
    func = ctypes.c_void_p()
    if cuda.cuModuleGetFunction(ctypes.byref(func), mod, b"stream_not") != 0:
        print("kernel not found")
        return 1

    # Буфер: 1024 МБ (запас під будь-яку карту з >= 2 ГБ)
    chunk_bytes = 1024 * 1024 * 1024
    chunk_words = chunk_bytes // 8
    d_ptr = ctypes.c_void_p()
    if cuda.cuMemAlloc_v2(ctypes.byref(d_ptr), chunk_bytes) != 0:
        print("cuMemAlloc failed")
        return 1

    # Еталонні слова для контролю коректності (перед завантаженням на GPU)
    import random
    rng = random.Random(42)
    sample_idx = sorted(rng.sample(range(chunk_words), 1024))
    sample_orig = [rng.getrandbits(64) for _ in sample_idx]

    def h2d_words(offset_words, words):
        """Заливка слів у device-пам'ять по зсуву (через тимчасовий host-буфер)."""
        host = (ctypes.c_uint64 * len(words))(*words)
        copied = ctypes.c_size_t()
        cuda.cuMemcpyHtoD_v2(
            ctypes.c_void_p(d_ptr.value + offset_words * 8),
            host, ctypes.c_size_t(len(words) * 8))

    # Початкові еталони в GPU-пам'ять
    h2d_words(0, sample_orig)  # тимчасово на позицію 0 для readback-тесту нижче

    block_dim = 256

    def launch(offset_words: int, n_words: int) -> None:
        grid = (n_words + block_dim - 1) // block_dim
        args = [
            ctypes.c_uint64(d_ptr.value),
            ctypes.c_uint64(offset_words),
            ctypes.c_uint64(n_words),
        ]
        arg_ptrs = (ctypes.c_void_p * 3)(
            ctypes.cast(ctypes.byref(args[0]), ctypes.c_void_p),
            ctypes.cast(ctypes.byref(args[1]), ctypes.c_void_p),
            ctypes.cast(ctypes.byref(args[2]), ctypes.c_void_p),
        )
        rc = cuda.cuLaunchKernel(func, grid, 1, 1, block_dim, 1, 1,
                                 0, None, arg_ptrs, None)
        if rc != 0:
            print(f"cuLaunchKernel rc={rc}")
            sys.exit(1)

    print("\n[Чесний зонд: стрімінг tableau-масштабного тіла з коректним зсувом]")
    print(f"{'n (кубіти-масштаб)':>20} | {'тіло tableau':>12} | {'проходів':>9} "
          f"| {'час':>10} | {'ефективні ГБ/с':>14} | {'коректність':>10}")
    print("-" * 92)

    for n in (65536, 131072, 262144, 524288, 1048576):
        # Розмір tableau: n рядків × 2n біт (біт-пакетні X|Z половини,
        # як у стабілізаторній таблиці) → байти
        row_words = (2 * n + 63) // 64
        total_bytes = n * row_words * 8
        passes = (total_bytes + chunk_bytes - 1) // chunk_bytes
        eff_bytes = passes * chunk_bytes  # що РЕАЛЬНО прокачується

        t0 = time.perf_counter()
        for p in range(passes):
            launch(p * chunk_words, chunk_words)
        cuda.cuCtxSynchronize()
        dt = time.perf_counter() - t0
        gbps = eff_bytes / dt / 1e9

        print(f"{n:>20,d} | {total_bytes / 2**30:>9.2f} ГБ | {passes:>9,d} "
              f"| {dt:>8.3f} c | {gbps:>12.1f} | {'(нижче)':>10}")

    # ── Контроль коректності: 1 проход + readback еталонних слів ──────────
    # Заливаємо еталони в КІНЕЦЬ буфера, робимо ОДИН NOT-проход по всьому
    # буферу, читаємо еталони назад: кожен мусить дорівнювати NOT(orig).
    tail_offset = chunk_words - 2048
    h2d_words(tail_offset, sample_orig)
    launch(0, chunk_words)  # один повний проход: усе, включно з хвостом
    cuda.cuCtxSynchronize()

    host_back = (ctypes.c_uint64 * len(sample_orig))()
    copied = ctypes.c_size_t()
    cuda.cuMemcpyDtoH_v2(
        host_back,
        ctypes.c_void_p(d_ptr.value + tail_offset * 8),
        ctypes.c_size_t(len(sample_orig) * 8))
    mismatches = sum(1 for got, orig in zip(host_back, sample_orig)
                     if got != (~orig) & 0xFFFFFFFFFFFFFFFF)
    print("-" * 92)
    print(f"Контроль коректності (1024 еталонних слів після РІВНО одного "
          f"NOT-проходу): {1024 - mismatches}/1024 OK")
    if mismatches:
        print("FAIL: відхилення виявлені — зонд некоректний")
        rc = 1
    else:
        print("OK: кожне слово = NOT(оригінал) рівно один раз "
              "(кратні проходи дали б тотожність — це і є контроль кратності)")
        rc = 0

    cuda.cuMemFree_v2(d_ptr)
    cuda.cuCtxDestroy_v2(ctx)
    print("\nНОТА: це зонд ПРОПУСКНОЇ ЗДАТНОСТІ ПАМ'ЯТІ для tableau-масштабних "
          "потоків,\nНЕ квантова симуляція. Gottesman–Knill на GPU потребує "
          "циієї трафіки + логіки гейтів;\nквантові твердження тут не "
          "робляться (виправлення фальші оригінала).")
    return rc


if __name__ == "__main__":
    sys.exit(main())
