#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! CLI `pqc`: Born-сэмплирование поверх контейнера `.poler` / `.pqw` —
//! полностью автономный бинарник (mmap zero-copy, без Python и Qiskit).
//!
//! ```text
//! pqc run <file> [--shots N] [--seed S] [--top K] [--purify N]
//!            [--engine auto|sv|product] [--max-sv-qubits N] [--verify]
//!            [--marginals]
//! pqc demo [--n N] [--shots M] [--seed S]
//! pqc stream (--url URL | --file PATH | --text TEXT | --stdin)
//!            [--dim N] [--epsilon E] [--shots N] [--steps N] [--seed S]
//!            [--eta0 H] [--beta B] [--gamma G] [--decay] [--json] [--out F]
//! pqc inspect <file> [--hex N|all] [--decode all|N] [--graph] [--dot F|-]
//!            [--matrix] [--strings] [--raw-dim N] [--json] [--all]
//! ```

use std::path::Path;
use std::process::exit;

use pqc::inspect::{
    arcs_csr_text, arcs_dot, ascii_matrix, ascii_strings, born_entropy, crypto_recon, detect_kind,
    graph_stats, header_rows, hex_dump, qcm_theory, raw_arcs, reader_arcs, report_json,
    try_pqw_reader, CryptoRecon, DecodedArc, FileKind, ARCS_PREVIEW, MATRIX_D_MAX,
};
use pqc::json::Json;
use pqc::{Ansatz, LoadOptions, Rng, DEFAULT_MAX_SV_QUBITS, MAX_QUBITS};
use pqc::bloch_stream;
use pqw::{PqwReader, PqwWriter};

const USAGE: &str = "\
POLER Quantum Core — statevector + Born sampling над .poler/.pqw

USAGE:
    pqc run <file> [options]
    pqc demo [--n N] [--shots M] [--seed S]
    pqc stream (--url URL | --file PATH | --text TEXT | --stdin) [options]
    pqc unfurl <file> [--threshold T]                         AOT phase unfurling to syntax
    pqc precess <file> [options]                              RQ11: петля архетипа поверх чекпоинта
    pqc bloch <file.pqw> [--head N] [--window W] [--json]    RQ14: триты → углы Блоха (mmap, θ на лету)
    pqc encrypt <IN> --key <ARCH.pqw> --out <C.pqt> [opts]    RQ23: триты GF(3), транспорт+спин-лавина
    pqc decrypt <C.pqt> --key <ARCH.pqw> [--out <PLAIN>]      m = p* ⊕ (a ⊗_ε p*) точно (v1+v2)
    pqc avalanche --key <ARCH.pqw> [--size N] [--probes P]    RQ23: нелинейная спиновая лавина
                                                              GF(3) на больших блоках данных
    pqc inspect <file> [options]
    pqc train (--corpus DIR | --stdin) [options]              накопительное обучение
    pqc generate (--brain F | --corpus TEXT | --corpus-file F) [opts]
                              RQ17: L5-генерация — Born-блуждание по руслам J
    pqc step (--brain F | --corpus TEXT) [--prompt T] [opts]  RQ17: один квант авторегрессии
    pqc ask «вопрос» --brain F [opts]                          RQ17: диалог с памятью
    pqc chat --brain F [opts]                                  RQ23: REPL-диалог с автобиографией —
                                                              помнит собеседника и нить между
                                                              сессиями (--name ИМЯ / --fresh)
    pqc archetype (--brain F [--prompt T|--with F]) [opts]     RQ18: мозг ⊗_ε промпт/мозг
    pqc learn «ТЕМА» --brain F [opts]                          RQ19: интернет-ингест (TLS 1.3 zero-dep)
                                                              RQ22: --docs URL — документации/markdown через HTTPS
    pqc merge A.pqw B.pqw --out M.pqw [opts]                  RQ20: слияние мозгов ⊗_ε → v4
                                                              RQ21: --settle — консолидация волной
    pqc qc <file.qc> [opts]                                  v0.44: Quantum PC — идеальные кубиты
                                                              (QCASM-схемы; --exact — кольцо
                                                              Z[1/sqrt(2), i], бит-в-бит)
    pqc algo <bell|ghz|qft|iqft|grover|bv|dj|period|teleport> [opts]  v0.44: алгоритмы на идеальных кубитах
                                                              period: --period R [--offset O] —
                                                              поиск периода (ядро Шора,
                                                              теоретико-числовой субстрат УДЕ)
                                                              teleport: --theta T [--exact] —
                                                              когерентная телепортация с
                                                              контролем фиделити (цикл P)
    pqc verify <unitary|equiv|teleport> [opts]               цикл P: формальный верификатор —
                                                              унитарность и эквивалентность схем
                                                              точно в кольце Z[1/sqrt(2), i]
    pqc qaoa [opts]                                          цикл Q: MaxCut-ансатц QAOA —
                                                              cost-гамильтониан + mixer, углы
                                                              оптимизируются координатным спуском
                                                              (--edges 0-1,1-2 | демо-граф)
    pqc substrate [opts]                                     v0.44: субстрат УДЕ — P-поток Ауфбау,
                                                              γ-прецессия, SCF-режим

QC OPTIONS (v0.44: POLER Quantum PC):
    --shots <N>               число выстрелов Борна (default 1024)
    --seed <S>                семя xoshiro256++ (default 42)
    --top <K>                 топ-K исходов в отчёте (default 20)
    --exact                   точный режим: амплитуды в Z[1/sqrt(2), i]
                              без единого округления (Clifford+T)
    --amplitudes              печатать амплитуды
    --probs                   полная таблица вероятностей
    --json                    машинно-читаемый отчёт (паритет с qiskit)

ALGO OPTIONS:
    --n <N>                   число кубитов (default 4)
    --marks <a,b,..>          помеченные состояния (grover/dj)
    --secret <S>              секрет BV (default 11 = 0b1011)
    --theta <T>               препарат телепортации Ry(T)|0⟩ (default 0.7)

VERIFY OPTIONS (цикл P):
    unitary <algo> [--n N]    доказать U†U = I для схемы алгоритма
    equiv <A> <B> [--n N]     доказать U_A = U_B [--phase — с точностью
                              до глобальной фазы]
    teleport                  доказать канал телепортации на базисе
                              {|0⟩,|1⟩} (по линейности — ∀α,β)
    --json                    машинно-читаемый отчёт

SUBSTRATE OPTIONS (УДЕ §2.2, цикл G):
    --dim <N>                 размерность гильбертова пространства (default 4)
    --steps <N>               шаги потока (default 400)
    --eta <H>                 дискретизация η (default 0.05)
    --gamma <G>               роторная связь γ: прецессия занятого
                              подпространства (default 0)
    --mu <M>                  химический потенциал (default 1)
    --fill <N>                число частиц (default 2)
    --scf <U>                 самосогласованное поле U (default 0 = F = H)
    --seed <S>                сид случайного H (default 42)
    --trace <N>               каждые N шагов печатать точку (default 25)

ENCRYPT/DECRYPT OPTIONS (RQ13: трит-схема GF(3) по умолчанию, файл 285;
                          --f32 — исследовательская схема RQ12):
    --key <F>                 контейнер-ключ .pqw v3 с гироскопом J
                              (precess --out / train --gyro)
    --out <F>                 encrypt: файл шифртекста .pqt (обязателен);
                              decrypt: файл открытого текста (без — stdout)
    --f32                     RQ12-схема на f32-фазах (×43, без лавины) —
                              для сравнения/воспроизведения RQ12
    --linear                  RQ23: линейная трит-схема RQ13 v1 (чистый
                              транспорт, без спинового слоя) — дефолт
                              теперь нелинейная v2: такт = транспорт +
                              квадратичный спин-проход GF(3)
    --modes <N>               число мод проектора (default: авто-калибровка
                              по помехе проекции; 1..=8)
    --seed <S>                семя IV xoshiro256++ (default: энтропия)
    --text <T>                encrypt: сообщение из строки вместо файла
    --stdin                   encrypt: сообщение из stdin
    --json                    машинно-читаемый отчёт

AVALANCHE OPTIONS (RQ23: pqc avalanche — нелинейная спиновая лавина
                   GF(3) на больших блоках данных):
    --key <F>                 контейнер-ключ .pqw v3+ с гироскопом J
    --size <N>                размер блока данных (default 65536 B;
                              псевдослучайный текст детерминирован сидом)
    --probes <N>              битовых зондов равномерно по тексту (default 16)
    --seed <S>                семя данных (default 42)
    --linear                  только нелинейный отчёт без сравнения с v1
    --json                    машинно-читаемый отчёт (лавина/каскад CBC/
                              распространение трансформа/хи-квадрат)

INSPECT OPTIONS (чтение бинарников, графов и крипто-разведка):
    --hex <N|all>             hex-дамп первых N байтов (default 512)
    --no-hex                  без hex-дампа
    --decode <all|N>          декод дуг: все или первые N (default 64)
    --graph                   CSR-дамп дуг + статистика LENS-графа
    --dot <FILE|->            Graphviz DOT в файл ('-' — в stdout)
    --matrix                  ASCII-матрица смежности (d_pol <= 64)
    --strings                 печатаемые ASCII-строки (min 6)
    --raw-dim <N>             raw Packed4 с d_pol = N (<= 4 × размер)
    --json                    машинно-читаемый отчёт (zero-dep JSON)
    --all                     полный отчёт: весь hex, все дуги, граф,
                              матрица, строки

STREAM OPTIONS (RQ6: zero-storage потоковое обучение):
    --url <URL>               http:// страница (zero-dep клиент; https → --file)
    --file <PATH>             локальный HTML/текст файл
    --text <TEXT>             встроенный текст чанка
    --stdin                   читать стандартный ввод до EOF
    --dim <N>                 размерность d_pol (default 512)
    --epsilon <E>             порог LENS ε-плотности (default 0.2)
    --shots <N>               Born-выстрелов на измерение (default 10000)
    --steps <N>               шагов Active Inference к цели (default 8)
    --seed <S>                семя xoshiro256++ (default 42)
    --eta0 <H>                базовый шаг η₀ (default 0.25)
    --beta <B>                затухание шага по сюрпризу β (default 1.0)
    --gamma <G>               трение γ (default 0.5, полюсная защита RQ5)
    --decay                   политика фона Decay (по умолчанию Hold)
    --json                    машинно-читаемый отчёт (zero-dep JSON)
    --out <F>                 дамп Packed4-контейнера в файл (опционально)

TRAIN OPTIONS (RQ8: плотный LENS-граф; RQ15: --quantized — решётка тритов;
              RQ16: --quantized --gyro — слияние решётки с гироскопом):
    --corpus <DIR>            каталог корпуса (рекурсивно, текст. расширения)
    --stdin                   поток блоков из стандартного ввода
    --dim <N>                 размерность d_pol (default 4096)
    --epsilon <E>             порог LENS ε-плотности (default 0.05 — плотный)
    --block <N>               размер блока в байтах (default 8192)
    --shots <N>               Born-выстрелов на измерение (default 20000)
    --steps <N>               шагов Active Inference на блок (default 10)
    --out <F>                 чекпоинт .pqw накопленной памяти
    --fingerprint <F>         raw Packed4-отпечаток фазовой памяти (снимок CPU)
    --snapshot-every <N>      снапшот каждые N блоков (default 64)
    --max-bytes <N>           потолок корпуса в байтах (default 256 МиБ)
    --log <F>                 файл телеметрии обучения
    --every <N>               прогресс в stdout каждые N файлов (default 25)
    --resume <F>              RQ9: поднять память из .pqw и продолжить обучение
    --curriculum [SCHEDULE]   RQ9: фазовая сборка — уровни с растущим чанком;
                              default 10:1:512:8K,128:2:1024:32K,
                              1024:4:4096:256K + LENS-уровень из --block/--steps
                              формат уровня BLOCK[:STEPS[:SHOTS[:BUDGET]]]
    --gyro [W]                RQ10: гироскоп памяти J = A − Aᵀ — направленная
                              циркуляция смысла в топологической секции v3;
                              окно W токенов (default 256), O(W)/токен — без O(N²)
    --gyro-budget <N>         RQ10: потолок сырых направленных пар (default 65536)
    --quantized               RQ15: квантованный curriculum — born-шаг прямо
                              в 2-битной решётке тритов: память = контейнер v2
                              бит-в-бит (чекпоинт без переквантования),
                              гистерезис π/3, полюса детерминированы;
                              η — калибровка RQ14 (0.6), если --eta0 не задан;
                              несовместим с --decay
    --quantized --gyro [W]    RQ16: СЛИЯНИЕ решётки с гироскопом — J = A − Aᵀ
                              в тритовой решётке русел (плотная треугольная
                              индексация пар, ниббл на пару, ноль HashMap),
                              2-битный насыщающий момент (τ_ign=0.5,
                              τ_stall=1.0, амплитуда 2.0 — инерция без
                              затухания), транспорт L5 p ← Π_Λ(e^{Δt·J}p);
                              --steps N = N шагов авторегрессионного
                              рассуждения после born-шага; чекпоинт v3
                              (фазы + секция GYRO); d_pol ≤ 16384 (O(d²));
                              несовместим с --decay и --gyro-budget
    --dt <H>                  RQ16: шаг Δt транспорта L5 (default 0.6;
                              только с --quantized --gyro)

GENERATE/ASK/CHAT OPTIONS (RQ17: L5-генерация — первые слова и рассуждение):
    --brain <F>               контейнер-мозг .pqw v3/v4 (train --quantized
                              --gyro); v4 несёт лексикон — без него мозг
                              не знает слов (морфемы AOT: --morphemes)
    --corpus <TEXT>           быстрое обучение на лету (inline текст) —
                              движок живёт в памяти, контейнер не нужен
    --corpus-file <F>         то же из файла (абзацы — документы)
    --prompt <T>              промпт-зерно (default: свободная речь —
                              затравка из лексикона)
    --think <N>               шагов мышления (транспорт L5) после слушания
                              (default 4)
    --max-tokens <N>          потолок эмиссий (default 64; pqc step = 1)
    --window <N>              кольцо контекста речи (default 8)
    --seed <S>                сид Born-лотереи (default 42)
    --free                    чистый поток J без моментных ворот (auto при
                              нулевой кинетике)
    --morphemes               AOT-фолбэк: координаты вне лексикона — морфемы
                              синтаксиса (fn/let/mut/…)
    --repeat-veto <N>         анти-заикание: запрет повтора координаты на N
                              шагов (default 1; 0 — выключено)
    --no-learn                ask: без перещёлкивания фаз born-шагом
    --no-bridge               выключить архетипический мост ⊗_ε (RQ18)
    --bridge-eps <F>          порог гейта моста ∈ (0,1] (default 0.5)
    --no-syntax               RQ21: выключить грамматические мосты —
                              чистая ассоциативная топология RQ17
                              (по умолчанию синтаксис ВКЛЮЧЁН:
                              взвешивание цепочек + коннекторы)
    --no-focus                RQ22: выключить маршрутизацию волны от дуг
                              вопроса (по умолчанию ВКЛЮЧЁНА: аттрактор
                              вопроса → BFS-радиус по руслам J, отсечка
                              посторонних веток, релевантность ответа)
    --focus-radius <N>        радиус фокуса волны 0..=8 (default 3:
                              dist 0 → x4, 1 → x3, 2 → x2, дальше в
                              радиусе → x1, за радиусом → вырезано)
    --no-reinforce            RQ22: выключить самоподкрепление грамматики
                              (по умолчанию ВКЛЮЧЕНО: удачные коннекторы
                              укрепляют русла J — насыщение за реплику)
    --name <ИМЯ>              RQ23: запомнить собеседника — имя живёт в
                              контекст-рефлексе W и переживает рестарт
    --fresh                   RQ23: холодный старт волны — не подхватывать
                              нить прошлого диалога из рефлекса W
                              (по умолчанию чат ПОМНИТ: кольцо гироскопа
                              восстанавливается из следа, затравка речи —
                              хвост нити; --brain с секцией REFL v5)
    --json                    машинно-читаемый отчёт (zero-dep JSON)

ARCHETYPE OPTIONS (RQ18: нелинейная алгебра a ⊗_ε b — мышление
архетипами; c = Π_Λ(R + ε·(a∧b)), гейт E = co/min(nnz) ≥ ε):
    --brain <F>               мозг A (контейнер v2/v3/v4, Packed4)
    --with <F>                мозг B: произведение двух мозгов —
                              структурный изоморфизм знаний
    --prompt <T>              архетип промпта как сомножитель B
    --eps <F>                 порог гейта ∈ (0,1] (default 0.5)
    --json                    машинно-читаемый отчёт

LEARN OPTIONS (RQ19: pqc learn \"ТЕМА\" — целенаправленный интернет-ингест;
              TLS 1.3 + HTTP/1.1 с нуля, zero-dep; источник — Wikipedia API;
              RQ22: --docs — источник документаций/markdown по HTTPS):
    --brain <F>               контейнер-мозг .pqw: существует — расширяется,
                              нет — создаётся свежий (v4, d_pol 4096)
    --pages <N>               страниц на раунд поиска (default 5)
    --rounds <N>              раундов; раунд ≥ 2 — самоуправляемый поиск:
                              модель сама уточняет запрос топ-новым словом
                              корпуса (default 2)
    --lang <auto|ru|en>       языковой раздел Wikipedia (default auto:
                              кириллица в теме → ru, иначе en)
    --full                    полные статьи вместо вводных секций (больше
                              текста, дольше ингест)
    --ask <ВОПРОС>            проверка инференса сразу после обучения
    --seed <S>                сид движка (default 42)
    --dim <N>                 d_pol свежего мозга (default 4096; для
                              существующего берётся из контейнера)
    --text <T>                offline-режим: одна «страница» из строки —
                              без сети (тема = заголовок)
    --docs <URL>              RQ22: источник документаций — markdown/текст
                              по HTTPS (повторяется для каждого файла);
                              разметка вычищается, раунд один — список
                              файлов явный (raw.githubusercontent.com …)
    --json                    машинно-читаемый отчёт (zero-dep JSON)

MERGE OPTIONS (RQ20: pqc merge — архетипическое слияние мозгов;
              фазы c = a ⊗_ε b (таблица без гейта — объединение
              знаний), русла J = Π_Λ(J_A + J_B) с кососимметричностью,
              консолидация LEXI; born-консолидация в контейнер;
              RQ21: --settle — консолидация слияния волной):
    --out <F>                контейнер слитого мозга (обязателен)
    --eps <F>                порог диагностики изоморфизма ∈ (0,1]
                              (default 0.5; гейт НЕ заперт — слияние
                              объединяет даже ортогональные домены)
    --settle [N]             RQ21: сеттлинг — N тактов (default 3,
                              ТЗ 2–4, максимум 16) авторегрессионного
                              рассуждения Π_Λ(e^{Δt·J} p) поверх слитых
                              русел: русла доменов прорастают общими
                              связями (гистерезис двух траекторий
                              волны), система оседает к стационару
                              ĤΨ = 0 (ранний выход на покое)
    --ask <ВОПРОС>           проверка мульти-доменного мышления сразу
                              после слияния (без записи в мозг)
    --seed <S>               сид Born-лотереи --ask (default 42)
    --json                   машинно-читаемый отчёт (zero-dep JSON)

INSPECT OPTIONS (дополнительно):
    --modes <N>               RQ10: число резонансных мод Im(P) для v3 (default 8)
                              RQ11: моды печатаются с невязками Ritz (инвариантность
                              плоскости) и ортонормальности ≡ идемпотентности
                              A² = A + захватом компонент

PRECESS OPTIONS (RQ11: уравнение архетипа a ⊗_ε a = a, p* = a ⊗_ε p*):
    --eta <H>                 шаг прецессии (default: eta из контейнера;
                              веса J нормируются на max|J| — H задаёт скорость)
    --ticks <N>               лимит тиков (default 16384)
    --delta <D>               стоп-порог max|Δθ| за тик (default 1e-9)
    --purify-every <K>        проекция McWeeny каждые K тиков (default 0 —
                              чистая унитарная прецессия; K=2..4 — оседание
                              к архетипу: фазы стягиваются в триты)
    --out <F>                 записать итоговое состояние (контейнер v3,
                              фазы p = cos θ, русла J сохраняются)
    --trace                   печатать всю траекторию (тик, |Δθ|, r, torque)
    --json                    машинно-читаемый отчёт

OPTIONS (run/demo):
    --shots <N>               Born-выстрелов (default 1024)
    --seed <S>                семя xoshiro256++ (default 42)
    --top <K>                 топ-K исходов в отчёте (default 8)
    --purify <N>              шаги McWeeny перед кодированием (default 0)
    --engine <auto|sv|product>  выбор движка (default auto)
    --max-sv-qubits <N>       порог statevector-движка (default 20)
    --verify                  проверить SHA-256 payload перед запуском
    --marginals               вывести все маргиналы (по умолчанию первые 20)";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("demo") => cmd_demo(&args[1..]),
        Some("stream") => cmd_stream(&args[1..]),
        Some("unfurl") => cmd_unfurl(&args[1..]),
        Some("precess") => cmd_precess(&args[1..]),
        Some("bloch") => cmd_bloch(&args[1..]),
        Some("encrypt") => cmd_encrypt(&args[1..]),
        Some("decrypt") => cmd_decrypt(&args[1..]),
        Some("avalanche") => cmd_avalanche(&args[1..]),
        Some("inspect") => cmd_inspect(&args[1..]),
        Some("train") => cmd_train(&args[1..]),
        Some("generate") => cmd_generate(&args[1..], false),
        Some("step") => cmd_generate(&args[1..], true),
        Some("ask") => cmd_ask(&args[1..], false),
        Some("chat") => cmd_ask(&args[1..], true),
        Some("archetype") => cmd_archetype(&args[1..]),
        Some("learn") => cmd_learn(&args[1..]),
        Some("merge") => cmd_merge(&args[1..]),
        Some("qc") => cmd_qc(&args[1..]),
        Some("algo") => cmd_algo(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("qaoa") => cmd_qaoa(&args[1..]),
        Some("substrate") => cmd_substrate(&args[1..]),
        Some("stab") => cmd_stab(&args[1..]),
        Some("noise") => cmd_noise(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            2
        }
    };
    exit(code);
}

// --- Источник байтов: zero-copy mmap на unix ---

enum Source {
    #[cfg(unix)]
    Map(pqw::Mmap),
    #[allow(dead_code)] // не используется на unix, но нужен для переносимости
    Mem(Vec<u8>),
}

impl Source {
    fn as_slice(&self) -> &[u8] {
        match self {
            #[cfg(unix)]
            Source::Map(m) => m.as_slice(),
            Source::Mem(v) => v,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            #[cfg(unix)]
            Source::Map(_) => "mmap",
            Source::Mem(_) => "mem",
        }
    }
}

fn load(path: &str) -> Result<Source, String> {
    #[cfg(unix)]
    return pqw::Mmap::open(Path::new(path))
        .map(Source::Map)
        .map_err(|e| e.to_string());
    #[cfg(not(unix))]
    return std::fs::read(path)
        .map(Source::Mem)
        .map_err(|e| e.to_string());
}

// --- Разбор аргументов ---

fn take_value(args: &[String], i: &mut usize, name: &str) -> Result<String, String> {
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn parse_num<T: std::str::FromStr>(s: &str, name: &str) -> Result<T, String> {
    s.parse().map_err(|_| format!("bad value for {name}: {s}"))
}

#[derive(Clone, Copy, PartialEq)]
enum EngineChoice {
    Auto,
    Sv,
    Product,
}

struct RunConfig {
    shots: u64,
    seed: u64,
    top: usize,
    purify: usize,
    engine: EngineChoice,
    max_sv_qubits: usize,
    verify: bool,
    marginals_all: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig {
            shots: 1024,
            seed: 42,
            top: 8,
            purify: 0,
            engine: EngineChoice::Auto,
            max_sv_qubits: DEFAULT_MAX_SV_QUBITS,
            verify: false,
            marginals_all: false,
        }
    }
}

/// Разбор общих опций; `file` заполняется первым позиционным аргументом.
fn parse_common(
    args: &[String],
    cfg: &mut RunConfig,
    file: &mut Option<String>,
) -> Result<(), String> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].clone();
        match a.as_str() {
            "--shots" => cfg.shots = parse_num(&take_value(args, &mut i, "--shots")?, "--shots")?,
            "--seed" => cfg.seed = parse_num(&take_value(args, &mut i, "--seed")?, "--seed")?,
            "--top" => cfg.top = parse_num(&take_value(args, &mut i, "--top")?, "--top")?,
            "--purify" => {
                cfg.purify = parse_num(&take_value(args, &mut i, "--purify")?, "--purify")?
            }
            "--max-sv-qubits" => {
                cfg.max_sv_qubits = parse_num(
                    &take_value(args, &mut i, "--max-sv-qubits")?,
                    "--max-sv-qubits",
                )?
            }
            "--verify" => cfg.verify = true,
            "--marginals" => cfg.marginals_all = true,
            "--engine" => {
                let v = take_value(args, &mut i, "--engine")?;
                cfg.engine = match v.as_str() {
                    "auto" => EngineChoice::Auto,
                    "sv" | "statevector" => EngineChoice::Sv,
                    "product" => EngineChoice::Product,
                    other => return Err(format!("bad engine: {other} (auto|sv|product)")),
                };
            }
            other if !other.starts_with("--") => {
                if file.replace(other.to_string()).is_some() {
                    return Err("file given twice".into());
                }
            }
            other => return Err(format!("unknown option {other}")),
        }
        i += 1;
    }
    Ok(())
}

// --- Общий конвейер отчёта ---

fn run_reader(
    source_desc: &str,
    source_kind: &str,
    size: usize,
    reader: &PqwReader,
    cfg: &RunConfig,
) -> Result<(), String> {
    let hyper = reader.hyperparams();
    println!("POLER Quantum Core — Born sampling");
    println!("source    : {source_desc} ({size} B, {source_kind})");
    println!(
        "container : d_pol={} nnz={} eps={:.3} mcweeny_residual={:.3e}",
        reader.d_pol(),
        reader.nnz(),
        hyper.epsilon_threshold,
        reader.mcweeny_residual()
    );
    println!(
        "hyper     : eta={} gamma={} rho={}",
        hyper.eta, hyper.gamma, hyper.rho
    );

    let mut opts = LoadOptions {
        purify_steps: cfg.purify,
        max_sv_qubits: cfg.max_sv_qubits,
        verify_payload: cfg.verify,
    };
    match cfg.engine {
        EngineChoice::Auto => {}
        EngineChoice::Sv => {
            if reader.d_pol() as usize > MAX_QUBITS {
                return Err(format!(
                    "sv engine: d_pol={} exceeds hard limit {} (use --engine product)",
                    reader.d_pol(),
                    MAX_QUBITS
                ));
            }
            opts.max_sv_qubits = MAX_QUBITS;
        }
        EngineChoice::Product => opts.max_sv_qubits = 0,
    }

    let ansatz = Ansatz::from_reader(reader, &opts).map_err(|e| e.to_string())?;
    match &ansatz {
        Ansatz::Statevector(sv) => println!(
            "engine    : statevector ({} qubits, {} amplitudes)",
            sv.n_qubits(),
            sv.dim()
        ),
        Ansatz::Product(pa) => println!(
            "engine    : product (d_pol={}, nnz={})",
            pa.d_pol(),
            pa.nnz()
        ),
    }
    if cfg.purify > 0 {
        println!("purify    : {} McWeeny steps", cfg.purify);
    }

    let mut rng = Rng::seed_from_u64(cfg.seed);
    let report = ansatz
        .sample(&mut rng, cfg.shots, cfg.top)
        .map_err(|e| e.to_string())?;
    println!(
        "shots     : {}  seed: {}  distinct: {}",
        report.shots, cfg.seed, report.distinct
    );

    if report.top.is_empty() {
        println!("top       : (nnz > 64 — паттерны дуг не помещаются в u64)");
    } else {
        println!("top-{} исходов:", report.top.len());
        let bits = match &ansatz {
            Ansatz::Statevector(sv) => Some(sv.n_qubits()),
            Ansatz::Product(_) => None,
        };
        for (k, ((outcome, count), theory)) in report.top.iter().zip(&report.top_probs).enumerate()
        {
            match bits {
                Some(n) => println!(
                    "  #{k}  |{:0width$b}⟩  count {count:>6}  p̂={:.4}  P={:.4}",
                    outcome,
                    p_hat(*count, report.shots),
                    theory,
                    width = n
                ),
                None => println!(
                    "  #{k}  arcs 0x{outcome:016X}  count {count:>6}  p̂={:.4}  P={:.4}",
                    p_hat(*count, report.shots),
                    theory
                ),
            }
        }
    }

    let limit = if cfg.marginals_all {
        report.marginals.len()
    } else {
        report.marginals.len().min(20)
    };
    println!("marginals P(b=1):");
    for (idx, theory, obs) in report.marginals.iter().take(limit) {
        println!("  arc {idx:>6}: theory {theory:.4}  observed {obs:.4}");
    }
    if limit < report.marginals.len() {
        println!(
            "  ... ещё {} (весь список: --marginals)",
            report.marginals.len() - limit
        );
    }

    // Теоретическая дисперсия веса: независимые биты продукта.
    let var_theory: f64 = match &ansatz {
        Ansatz::Statevector(_) => report
            .marginals
            .iter()
            .map(|&(_, t, _)| t * (1.0 - t))
            .sum(),
        Ansatz::Product(pa) => {
            let background = f64::from(pa.d_pol()) - pa.nnz() as f64;
            background * 0.25
                + pa.arcs()
                    .iter()
                    .map(|&(_, p)| {
                        let m = 0.5 * (1.0 - p);
                        m * (1.0 - m)
                    })
                    .sum::<f64>()
        }
    };
    println!(
        "hamming weight: mean {:.2} (theory {:.2})  var {:.2} (theory {:.2})",
        report.weight_mean, report.expected_weight, report.weight_var, var_theory
    );
    Ok(())
}

fn p_hat(count: u64, shots: u64) -> f64 {
    count as f64 / shots.max(1) as f64
}

// ============================================================================
// pqc stream (RQ6): Zero-Storage Streaming Learning Engine
// ============================================================================

struct StreamConfig {
    url: Option<String>,
    file: Option<String>,
    text: Option<String>,
    stdin: bool,
    dim: u32,
    epsilon: f32,
    shots: u64,
    steps: usize,
    seed: u64,
    eta0: f64,
    beta: f64,
    gamma: f64,
    decay: bool,
    json: bool,
    out: Option<String>,
}

impl Default for StreamConfig {
    fn default() -> Self {
        StreamConfig {
            url: None,
            file: None,
            text: None,
            stdin: false,
            dim: 512,
            epsilon: 0.2,
            shots: 10_000,
            steps: 8,
            seed: 42,
            eta0: 0.25,
            beta: 1.0,
            gamma: 0.5,
            decay: false,
            json: false,
            out: None,
        }
    }
}

fn cmd_stream(args: &[String]) -> i32 {
    let mut cfg = StreamConfig::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].clone();
        let mut val = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match a.as_str() {
            "--url" => match val("--url") {
                Ok(v) => cfg.url = Some(v),
                Err(e) => return stream_usage_err(&e),
            },
            "--file" => match val("--file") {
                Ok(v) => cfg.file = Some(v),
                Err(e) => return stream_usage_err(&e),
            },
            "--text" => match val("--text") {
                Ok(v) => cfg.text = Some(v),
                Err(e) => return stream_usage_err(&e),
            },
            "--stdin" => cfg.stdin = true,
            "--dim" => match val("--dim").and_then(|v| parse_num::<u32>(&v, "--dim")) {
                Ok(v) => cfg.dim = v,
                Err(e) => return stream_usage_err(&e),
            },
            "--epsilon" => match val("--epsilon")
                .and_then(|v| v.parse::<f32>().map_err(|_| "bad --epsilon".to_string()))
            {
                Ok(v) => cfg.epsilon = v,
                Err(_) => return stream_usage_err("bad --epsilon"),
            },
            "--shots" => match val("--shots").and_then(|v| parse_num::<u64>(&v, "--shots")) {
                Ok(v) => cfg.shots = v,
                Err(e) => return stream_usage_err(&e),
            },
            "--steps" => match val("--steps").and_then(|v| parse_num::<usize>(&v, "--steps")) {
                Ok(v) => cfg.steps = v,
                Err(e) => return stream_usage_err(&e),
            },
            "--seed" => match val("--seed").and_then(|v| parse_num::<u64>(&v, "--seed")) {
                Ok(v) => cfg.seed = v,
                Err(e) => return stream_usage_err(&e),
            },
            "--eta0" => match val("--eta0")
                .and_then(|v| v.parse::<f64>().map_err(|_| "bad --eta0".to_string()))
            {
                Ok(v) => cfg.eta0 = v,
                Err(_) => return stream_usage_err("bad --eta0"),
            },
            "--beta" => match val("--beta")
                .and_then(|v| v.parse::<f64>().map_err(|_| "bad --beta".to_string()))
            {
                Ok(v) => cfg.beta = v,
                Err(_) => return stream_usage_err("bad --beta"),
            },
            "--gamma" => match val("--gamma")
                .and_then(|v| v.parse::<f64>().map_err(|_| "bad --gamma".to_string()))
            {
                Ok(v) => cfg.gamma = v,
                Err(_) => return stream_usage_err("bad --gamma"),
            },
            "--decay" => cfg.decay = true,
            "--json" => cfg.json = true,
            "--out" => match val("--out") {
                Ok(v) => cfg.out = Some(v),
                Err(e) => return stream_usage_err(&e),
            },
            other => return stream_usage_err(&format!("unknown option {other}")),
        }
        i += 1;
    }

    // Ровно один источник входа.
    let sources = cfg.url.is_some() as u8
        + cfg.file.is_some() as u8
        + cfg.text.is_some() as u8
        + cfg.stdin as u8;
    if sources != 1 {
        return stream_usage_err("укажите ровно один источник: --url | --file | --text | --stdin");
    }
    if cfg.dim == 0 || cfg.dim > 1 << 20 {
        return stream_usage_err("--dim должен быть в [1, 1048576]");
    }
    if !(0.0..=1.0).contains(&cfg.epsilon) || !cfg.epsilon.is_finite() {
        return stream_usage_err("--epsilon должен быть в [0, 1]");
    }

    // Входные байты.
    let (bytes, source_desc, source_kind) = if let Some(url) = &cfg.url {
        match http_get(url, MAX_HTTP_BYTES) {
            Ok((body, final_url)) => (body, final_url, "http".to_string()),
            Err(e) => {
                eprintln!("pqc stream: {e}");
                return 1;
            }
        }
    } else if let Some(path) = &cfg.file {
        match std::fs::read(path) {
            Ok(b) => (b, path.clone(), "file".to_string()),
            Err(e) => {
                eprintln!("pqc stream: {e}");
                return 1;
            }
        }
    } else if let Some(text) = &cfg.text {
        (
            text.clone().into_bytes(),
            "--text".to_string(),
            "text".to_string(),
        )
    } else {
        use std::io::Read;
        let mut buf = Vec::new();
        if let Err(e) = std::io::stdin().read_to_end(&mut buf) {
            eprintln!("pqc stream: stdin: {e}");
            return 1;
        }
        (buf, "stdin".to_string(), "stdin".to_string())
    };

    // Движок: zero-storage цикл целиком в RAM.
    use pqc::stream_engine::{Forget, StreamEngine};
    let forget = if cfg.decay {
        Forget::Decay
    } else {
        Forget::Hold
    };
    let mut engine = match StreamEngine::new(cfg.dim, cfg.epsilon, cfg.seed) {
        Ok(e) => e
            .with_shots(cfg.shots)
            .with_hyper(cfg.eta0, cfg.beta, cfg.gamma)
            .with_forget(forget),
        Err(e) => {
            eprintln!("pqc stream: {e}");
            return 1;
        }
    };
    let rep = match engine.ingest_html(&bytes, cfg.steps) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc stream: {e}");
            return 1;
        }
    };

    // Опциональный дамп контейнера (единственная точка касания диска).
    if let Some(out) = &cfg.out {
        if let Err(e) = std::fs::write(out, engine.container()) {
            eprintln!("pqc stream: --out: {e}");
            return 1;
        }
    }

    if cfg.json {
        print_stream_json(&rep, &source_desc, &source_kind, bytes.len(), cfg.dim);
    } else {
        print_stream_human(&rep, &source_desc, &source_kind, bytes.len(), &engine);
    }
    0
}

fn stream_usage_err(msg: &str) -> i32 {
    eprintln!("pqc stream: {msg}\n\n{USAGE}");
    2
}

fn print_stream_human(
    rep: &pqc::stream_engine::StreamChunkReport,
    source_desc: &str,
    source_kind: &str,
    input_bytes: usize,
    engine: &pqc::stream_engine::StreamEngine,
) {
    println!("POLER Quantum Core — Zero-Storage Streaming Engine (RQ6)");
    println!(
        "source    : {source_desc} ({:.1} КиБ, {source_kind})",
        input_bytes as f64 / 1024.0
    );
    println!(
        "text      : {} токенов, LENS eps={:.2}",
        rep.tokens,
        engine.epsilon()
    );
    if rep.no_hits {
        println!("barrier   : NO_HITS — свидетельство пусто, детерминированный отказ");
        println!("fock      : [F, DM]_S = 0 точно (нет факта - нет галлюцинации)");
    } else {
        println!(
            "container : d_pol={} nnz={} {} B (Packed4 v0.2: 4 трита/байт, magic POLER_Q2)",
            engine.d_pol(),
            rep.nnz,
            rep.container_bytes
        );
        println!(
            "buffer    : capacity {} B (реюз, системных аллокаций после прогрева нет)",
            rep.buffer_capacity
        );
        println!(
            "fock      : raw={:.4} normalized={:.4} (коммутатор [F, DM]_S свидетельство x память)",
            rep.fock.raw, rep.fock.normalized
        );
        if let Some(s) = &rep.step {
            println!(
                "step 1    : surprise Sigma={:.4}  eta={:.4}  (eta0*e^(-beta*Sigma))",
                s.surprise, s.eta
            );
        }
        println!(
            "loss      : {:.6} ({} шагов Active Inference, gamma={})",
            rep.param_loss,
            rep.steps_run,
            engine.learner().gamma()
        );
        println!(
            "qcm       : theory {:.4}  observed {:.4}  gap {:.4}  ({} выстрелов)",
            rep.qcm.qcm_theory,
            rep.qcm.qcm_observed,
            rep.qcm.qcm_gap(),
            rep.qcm.shots
        );
    }
    println!(
        "elapsed   : {:.3} мс (сквозной цикл в RAM, диск не затронут)",
        rep.elapsed.as_secs_f64() * 1000.0
    );
}

fn print_stream_json(
    rep: &pqc::stream_engine::StreamChunkReport,
    source_desc: &str,
    source_kind: &str,
    input_bytes: usize,
    dim: u32,
) {
    use pqc::json::Json;
    let step_obj = |s: &pqc::learn::ActiveStepReport| {
        Json::Obj(vec![
            ("step".into(), Json::num(s.step as f64)),
            ("shots".into(), Json::num(s.shots as f64)),
            ("surprise".into(), Json::num(s.surprise)),
            ("eta".into(), Json::num(s.eta)),
            ("surrogate_loss".into(), Json::num(s.surrogate_loss)),
            ("grad_norm".into(), Json::num(s.grad_norm)),
            ("max_dp".into(), Json::num(s.max_dp)),
            ("measurement_mad".into(), Json::num(s.measurement_mad)),
            ("purified".into(), Json::Bool(s.purified)),
            ("mean_abs_p".into(), Json::num(s.mean_abs_p)),
        ])
    };
    let obj = Json::Obj(vec![
        ("source".into(), Json::str(source_desc)),
        ("source_kind".into(), Json::str(source_kind)),
        ("input_bytes".into(), Json::num(input_bytes as f64)),
        ("d_pol".into(), Json::num(dim as f64)),
        ("tokens".into(), Json::num(rep.tokens as f64)),
        ("nnz".into(), Json::num(rep.nnz as f64)),
        (
            "container_bytes".into(),
            Json::num(rep.container_bytes as f64),
        ),
        (
            "buffer_capacity".into(),
            Json::num(rep.buffer_capacity as f64),
        ),
        ("docs_seen".into(), Json::num(rep.docs_seen as f64)),
        ("no_hits".into(), Json::Bool(rep.no_hits)),
        (
            "fock".into(),
            Json::Obj(vec![
                ("raw".into(), Json::num(rep.fock.raw)),
                ("normalized".into(), Json::num(rep.fock.normalized)),
            ]),
        ),
        (
            "step".into(),
            match &rep.step {
                Some(s) => step_obj(s),
                None => Json::Null,
            },
        ),
        ("steps_run".into(), Json::num(rep.steps_run as f64)),
        ("param_loss".into(), Json::num(rep.param_loss)),
        (
            "qcm".into(),
            Json::Obj(vec![
                ("theory".into(), Json::num(rep.qcm.qcm_theory)),
                ("observed".into(), Json::num(rep.qcm.qcm_observed)),
                (
                    "born_entropy_bits".into(),
                    Json::num(rep.qcm.born_entropy_bits),
                ),
                ("marginal_mad".into(), Json::num(rep.qcm.marginal_mad)),
            ]),
        ),
        (
            "elapsed_ms".into(),
            Json::num(rep.elapsed.as_secs_f64() * 1000.0),
        ),
    ]);
    println!("{}", obj.to_string());
}

/// Потолок тела HTTP-ответа (защита от бесконечных потоков).
const MAX_HTTP_BYTES: usize = 16 * 1024 * 1024;
/// Лимит переходов по редиректам.
const MAX_REDIRECTS: usize = 3;

/// Минимальный HTTP/1.1 GET-клиент на `std::net::TcpStream` —
/// **ноль внешних зависимостей** (TLS сознательно не поддерживается:
/// https-страницу следует сохранить и передать через `--file`).
fn http_get(url: &str, max_bytes: usize) -> Result<(Vec<u8>, String), String> {
    let mut current = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        let (host, port, path) = parse_http_url(&current)?;
        let stream = std::net::TcpStream::connect((host.as_str(), port))
            .map_err(|e| format!("connect {host}:{port}: {e}"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(15)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(15)))
            .map_err(|e| e.to_string())?;
        let mut stream = stream;
        let host_header = if port == 80 {
            host.clone()
        } else {
            format!("{host}:{port}")
        };
        let req = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host_header}\r\n\
             User-Agent: pqc-stream/0.3\r\n\
             Accept: text/html,application/xhtml+xml,text/plain;q=0.9,*/*;q=0.8\r\n\
             Accept-Encoding: identity\r\n\
             Connection: close\r\n\r\n"
        );
        use std::io::{Read, Write};
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("send: {e}"))?;

        // Ответ целиком (Connection: close упрощает границы тела).
        let mut raw = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            let n = stream.read(&mut chunk).map_err(|e| format!("recv: {e}"))?;
            if n == 0 {
                break;
            }
            if raw.len() + n > max_bytes + 64 * 1024 {
                return Err(format!("ответ превышает лимит {max_bytes} байт"));
            }
            raw.extend_from_slice(&chunk[..n]);
        }

        // Заголовки / тело.
        let sep = find_headers_end(&raw).ok_or("ответ без завершения заголовков")?;
        let head = String::from_utf8_lossy(&raw[..sep]).to_string();
        let body = raw[sep + 4..].to_vec();
        let mut lines = head.split("\r\n");
        let status_line = lines.next().unwrap_or_default().to_string();
        let code: u16 = status_line
            .split_ascii_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| format!("битый статус: {status_line}"))?;

        let mut location = None;
        let mut chunked = false;
        for line in lines {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if k == "location" {
                location = Some(v.to_string());
            }
            if k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked") {
                chunked = true;
            }
        }

        match code {
            200..=299 => {
                let body = if chunked {
                    decode_chunked(&body, max_bytes)?
                } else {
                    body
                };
                return Ok((body, current));
            }
            301 | 302 | 303 | 307 | 308 => {
                let loc = location.ok_or_else(|| format!("редирект {code} без Location"))?;
                current = resolve_url(&current, &loc)?;
                continue;
            }
            _ => return Err(format!("HTTP {code}: {status_line}")),
        }
    }
    Err("слишком много редиректов".into())
}

/// `http://host[:port]/path?query` → (host, port, path).
fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    if url.starts_with("https://") {
        return Err(format!(
            "https не поддерживается zero-dep клиентом: сохраните страницу и передайте --file ({url})"
        ));
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("неподдерживаемая схема (нужен http://): {url}"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err("пустой хост".into());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>().map_err(|_| format!("битый порт: {p}"))?,
        ),
        None => (authority.to_string(), 80),
    };
    Ok((host, port, path.to_string()))
}

/// Индекс `\r\n\r\n` (конец заголовков).
fn find_headers_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Декодирование Transfer-Encoding: chunked.
fn decode_chunked(body: &[u8], max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let Some(nl) = body[i..].windows(2).position(|w| w == b"\r\n") else {
            return Err("битый chunked: нет конца size-строки".into());
        };
        let size_line = String::from_utf8_lossy(&body[i..i + nl]).to_string();
        let size_hex = size_line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|_| format!("битый chunked размер: {size_hex}"))?;
        i += nl + 2;
        if size == 0 {
            break;
        }
        if out.len() + size > max_bytes {
            return Err(format!("chunked тело превышает лимит {max_bytes} байт"));
        }
        if i + size > body.len() {
            return Err("битый chunked: обрыв данных".into());
        }
        out.extend_from_slice(&body[i..i + size]);
        i += size + 2; // данные + CRLF
    }
    Ok(out)
}

/// Относительный Location → абсолютный URL.
fn resolve_url(base: &str, location: &str) -> Result<String, String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        if location.starts_with("https://") {
            return Err("редирект на https не поддерживается zero-dep клиентом".into());
        }
        return Ok(location.to_string());
    }
    let (host, port, _) = parse_http_url(base)?;
    let path = if location.starts_with('/') {
        location.to_string()
    } else {
        // Относительный путь от корня (упрощение: без разбора '..').
        format!("/{location}")
    };
    Ok(format!("http://{host}:{port}{path}"))
}

// --- Команды ---

fn cmd_inspect(args: &[String]) -> i32 {
    struct InspectConfig {
        hex_limit: Option<usize>, // None — без hex
        decode: Option<usize>,    // None — без декода; usize::MAX — все дуги
        graph: bool,
        dot: Option<String>,
        matrix: bool,
        strings: bool,
        raw_dim: Option<u32>,
        json: bool,
        modes: usize, // RQ10: число резонансных мод гироскопа
    }
    let mut cfg = InspectConfig {
        hex_limit: Some(512),
        decode: Some(ARCS_PREVIEW),
        graph: false,
        dot: None,
        matrix: false,
        strings: false,
        raw_dim: None,
        json: false,
        modes: 8,
    };
    let mut file: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        macro_rules! val {
            ($name:expr) => {{
                i += 1;
                let Some(v) = args.get(i) else {
                    eprintln!("pqc inspect: missing value for {}\n\n{USAGE}", $name);
                    return 2;
                };
                v.clone()
            }};
        }
        match a {
            "--hex" => {
                let v = val!("--hex");
                cfg.hex_limit = if v == "all" {
                    Some(usize::MAX)
                } else {
                    match v.parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            eprintln!("pqc inspect: bad --hex: {v} (N|all)\n\n{USAGE}");
                            return 2;
                        }
                    }
                };
            }
            "--no-hex" => cfg.hex_limit = None,
            "--decode" => {
                let v = val!("--decode");
                cfg.decode = if v == "all" {
                    Some(usize::MAX)
                } else {
                    match v.parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            eprintln!("pqc inspect: bad --decode: {v} (all|N)\n\n{USAGE}");
                            return 2;
                        }
                    }
                };
            }
            "--graph" => cfg.graph = true,
            "--dot" => cfg.dot = Some(val!("--dot")),
            "--matrix" => cfg.matrix = true,
            "--strings" => cfg.strings = true,
            "--raw-dim" => {
                let v = val!("--raw-dim");
                match v.parse::<u32>() {
                    Ok(n) if n > 0 => cfg.raw_dim = Some(n),
                    _ => {
                        eprintln!("pqc inspect: bad --raw-dim: {v} (> 0)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--json" => cfg.json = true,
            "--modes" => {
                let v = val!("--modes");
                match v.parse::<usize>() {
                    Ok(n) if n > 0 && n <= 64 => cfg.modes = n,
                    _ => {
                        eprintln!("pqc inspect: bad --modes: {v} (1..=64)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--all" => {
                cfg.hex_limit = Some(usize::MAX);
                cfg.decode = Some(usize::MAX);
                cfg.graph = true;
                cfg.matrix = true;
                cfg.strings = true;
            }
            other if !other.starts_with("--") => {
                if file.replace(other.to_string()).is_some() {
                    eprintln!("pqc inspect: file given twice\n\n{USAGE}");
                    return 2;
                }
            }
            other => {
                eprintln!("pqc inspect: unknown option {other}\n\n{USAGE}");
                return 2;
            }
        }
        i += 1;
    }

    let Some(file) = file else {
        eprintln!("pqc inspect: file required\n\n{USAGE}");
        return 2;
    };
    let src = match load(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pqc inspect: {e}");
            return 1;
        }
    };
    let data = src.as_slice();
    let size = data.len();

    // ── Детект формата и загрузка дуг ──
    let mut kind = detect_kind(data);
    let mut parse_error: Option<String> = None;
    let mut reader: Option<PqwReader> = None;
    let mut arcs: Vec<DecodedArc> = Vec::new();
    let mut d_pol: u32 = 0;

    match kind {
        FileKind::PqwV1 | FileKind::PqwV2 | FileKind::PqwV3 | FileKind::PqwV4
        | FileKind::PqwV5 => {
            match try_pqw_reader(data) {
                Ok(Some(r)) => {
                    d_pol = r.d_pol();
                    arcs = reader_arcs(&r);
                    reader = Some(r);
                }
                Ok(None) => parse_error = Some("file shorter than the 128-byte header".into()),
                Err(e) => parse_error = Some(e),
            }
        }
        FileKind::RawPacked4 { d_pol: d } => {
            d_pol = cfg.raw_dim.unwrap_or(d);
            if d_pol as usize > size * 4 {
                eprintln!(
                    "pqc inspect: --raw-dim {d_pol} exceeds 4 × {size} = {} trits",
                    size * 4
                );
                return 2;
            }
            arcs = raw_arcs(data);
            arcs.retain(|a| a.index < d_pol);
        }
        FileKind::Opaque => {
            if let Some(d) = cfg.raw_dim {
                kind = FileKind::RawPacked4 { d_pol: d };
                d_pol = d;
                if d_pol as usize > size * 4 {
                    eprintln!(
                        "pqc inspect: --raw-dim {d_pol} exceeds 4 × {size} = {} trits",
                        size * 4
                    );
                    return 2;
                }
                arcs = raw_arcs(data);
                arcs.retain(|a| a.index < d_pol);
            }
        }
    }

    let recon: CryptoRecon = crypto_recon(data);

    // ── RQ10: гироскопная топология v3 — J = A − Aᵀ + моды Im(P) ──
    let gyro_section = reader.as_ref().and_then(|r| r.gyro());
    let gyro_modes = gyro_section.as_ref().map(|g| {
        let pairs: Vec<(u32, u32, f64)> = g
            .pairs()
            .iter()
            .map(|p| (p.i, p.j, p.weight))
            .collect();
        pqc::gyro::resonant_modes_from_pairs(&pairs, cfg.modes)
    });
    let gyro_json: Option<Json> = gyro_section.as_ref().map(|g| {
        let mut obj = vec![
            ("present".into(), Json::Bool(true)),
            ("window".into(), Json::num(g.window() as f64)),
            ("ticks".into(), Json::num(g.ticks() as f64)),
            ("pairs".into(), Json::num(g.pairs().len() as f64)),
            ("scale".into(), Json::num(g.scale() as f64)),
            ("index16".into(), Json::Bool(g.index16())),
            (
                "codec".into(),
                Json::num(if g.codec() == pqw::GYRO_SECTION_VERSION_RLE {
                    2.0
                } else {
                    1.0
                }),
            ),
        ];
        // RQ23: статистика сжатия gap-RLE против разреженного кодека.
        if g.codec() == pqw::GYRO_SECTION_VERSION_RLE {
            let data = pqw::GyroData::new(
                g.window(),
                g.ticks(),
                g.pairs().iter().map(|p| (p.i, p.j, p.weight)).collect(),
                d_pol,
            );
            if let Ok(data) = data {
                let sparse = data.encode(reader.as_ref().map(|r| r.header().flags.index16()).unwrap_or(false));
                if let Ok(sparse) = sparse {
                    let stored = data
                        .encode_rle()
                        .map(|v| v.len())
                        .unwrap_or_default();
                    if sparse.len() > 0 && stored > 0 {
                        obj.push(("sparse_bytes".into(), Json::num(sparse.len() as f64)));
                        obj.push(("rle_bytes".into(), Json::num(stored as f64)));
                        obj.push((
                            "compression".into(),
                            Json::num(sparse.len() as f64 / stored as f64),
                        ));
                    }
                }
            }
        }
        if let Some(modes) = &gyro_modes {
            // Фазовый портрет Im(P): p̂ и θ = arccos(p̂) дуг моды —
            // из фазовой секции контейнера.
            let phase: std::collections::HashMap<u32, f64> =
                arcs.iter().map(|a| (a.index, a.p_hat)).collect();
            let comp = |v: &[(u32, f64)]| -> Json {
                Json::Arr(
                    v.iter()
                        .map(|&(n, c)| {
                            let mut fields = vec![
                                ("idx".into(), Json::num(f64::from(n))),
                                ("comp".into(), Json::num(c)),
                            ];
                            if let Some(&p) = phase.get(&n) {
                                fields.push(("p".into(), Json::num(p)));
                                fields.push(("theta".into(), Json::num(p.acos())));
                            }
                            Json::Obj(fields)
                        })
                        .collect(),
                )
            };
            let modes_json: Vec<Json> = modes
                .iter()
                .map(|m| {
                    Json::Obj(vec![
                        ("lambda".into(), Json::num(m.lambda)),
                        ("ritz_residual".into(), Json::num(m.ritz_residual)),
                        ("ortho_residual".into(), Json::num(m.ortho_residual)),
                        ("capture".into(), Json::num(m.capture)),
                        ("u".into(), comp(&m.u)),
                        ("v".into(), comp(&m.v)),
                    ])
                })
                .collect();
            obj.push(("modes".into(), Json::Arr(modes_json)));
        }
        Json::Obj(obj)
    });

    // ── JSON-режим: единый объект и выход ──
    if cfg.json {
        // RQ23: контекст-рефлекс W — автобиография диалога.
        let mut report = report_json(&file, data, kind, &arcs, d_pol, &recon, gyro_json);
        if let Json::Obj(pairs) = &mut report {
            if let Some(r) = reader.as_ref().and_then(|r| r.reflex()) {
                pairs.push((
                    "reflex".into(),
                    Json::Obj(vec![
                        ("interlocutor".into(), Json::str(r.interlocutor())),
                        ("turns".into(), Json::num(r.turns() as f64)),
                        ("trail_events".into(), Json::num(r.events().len() as f64)),
                    ]),
                ));
            }
        }
        println!("{}", report.to_string());
        return 0;
    }

    // ── Человекочитаемый отчёт ──
    println!("POLER Quantum Core — inspect");
    println!("file      : {file} ({size} B, {})", src.kind());
    println!("sha256    : {}", pqc::inspect::sha256_hex(data));
    if let Some(e) = &parse_error {
        println!("parse     : ERROR — {e} (дальше — сырая разведка)");
    }

    println!("\nFORMAT");
    println!("  kind       : {}", kind.name());
    if d_pol > 0 {
        let neg = arcs.iter().filter(|a| a.trit < 0).count();
        let pos = arcs.iter().filter(|a| a.trit > 0).count();
        let density = arcs.len() as f64 / f64::from(d_pol) * 100.0;
        println!("  d_pol      : {d_pol}");
        println!(
            "  nnz        : {} ({} × −1, {} × +1), density {density:.3}%",
            arcs.len(),
            neg,
            pos
        );
        println!(
            "  born       : H = {:.4} bits, QCM = {:.6}",
            born_entropy(&arcs),
            qcm_theory(&arcs, d_pol)
        );
    }

    // ── RQ10: гироскоп J = A − Aᵀ (топологическая секция v3) ──
    if let Some(g) = &gyro_section {
        let h = reader.as_ref().map(|r| r.header()).unwrap();
        println!("\nGYRO (J = A − Aᵀ, кососимметричный резонансный оператор)");
        println!(
            "  window     : {} токенов направленного контекста",
            g.window()
        );
        println!("  ticks      : {} наблюдённых событий потока", g.ticks());
        println!(
            "  pairs      : {} хранимых пар (верхний треугольник, ε-ворота)",
            g.pairs().len()
        );
        println!(
            "  section    : {} B @ 0x{:x} (5 B/пара, веса i8, scale {:.4})",
            h.topology_len, h.topology_offset, g.scale()
        );
        // Превью главных русел циркуляции.
        let mut top: Vec<&pqw::GyroPair> = g.pairs().iter().collect();
        top.sort_by(|a, b| b.weight.abs().total_cmp(&a.weight.abs()));
        let preview = top.len().min(8);
        if preview > 0 {
            println!("  top flow   : (src → dst, вес — направление потока смысла)");
            for p in &top[..preview] {
                println!("               {:>5} → {:<5} {:+.4}", p.i, p.j, p.weight);
            }
        }
        // Резонансные моды: Im(P) — фазовые векторы смысла.
        if let Some(modes) = &gyro_modes {
            if !modes.is_empty() {
                println!("\n  моды Im(P): плоскости вращения (u, v), λ = угловая скорость;");
                println!("               θ = arccos(p̂) — фазовый угол дуги (вектор смысла);");
                println!("               Ritz — инвариантность плоскости, орто ≡ идемпотентность");
                println!("               A² = A (уравнение архетипа), захват — доля массы в ≤12 дугах");
                let phase: std::collections::HashMap<u32, f64> =
                    arcs.iter().map(|a| (a.index, a.p_hat)).collect();
                for (k, m) in modes.iter().enumerate() {
                    println!(
                        "  λ{} = {:.4}  [Ritz {:.1e} | орто {:.1e} | захват {:.1}%]",
                        k + 1,
                        m.lambda,
                        m.ritz_residual,
                        m.ortho_residual,
                        100.0 * m.capture
                    );
                    for (label, vec) in [("u", &m.u), ("v", &m.v)] {
                        let shown = vec.len().min(6);
                        if shown == 0 {
                            continue;
                        }
                        let comps: Vec<String> = vec[..shown]
                            .iter()
                            .map(|&(n, c)| {
                                match phase.get(&n) {
                                    Some(&p) => format!(
                                        "{n}({:+.2}|p{:+.1}|θ{:.2}π)",
                                        c,
                                        p,
                                        p.acos() / std::f64::consts::PI
                                    ),
                                    None => format!("{n}({c:+.2})"),
                                }
                            })
                            .collect();
                        println!("    {label}      : {}", comps.join("  "));
                    }
                }
            }
        }
    }

    println!("\nCRYPTO RECON");
    println!("  entropy    : {:.4} / 8.0000 bits/byte", recon.entropy);
    if recon.chi2.is_finite() {
        println!(
            "  chi2       : {:.1} (uniform band 255 ± 352) — {}",
            recon.chi2,
            if (recon.chi2 - 255.0).abs() <= 352.0 {
                "uniform"
            } else {
                "NOT uniform"
            }
        );
    }
    println!("  distinct   : {} / 256 byte values", recon.distinct);
    if !recon.blocks.is_empty() {
        let line: Vec<String> = recon.blocks.iter().map(|h| format!("{h:.2}")).collect();
        println!("  blocks 64B : {}", line.join(" "));
    }
    if recon.container_hits.is_empty() {
        println!("  containers : — (шифро-контейнеров не найдено)");
    } else {
        println!("  containers : {}", recon.container_hits.join("; "));
    }
    match recon.verdict {
        "structured" => {
            println!("  verdict    : structured — шифрования НЕТ, данные полностью читаемы")
        }
        "compressed" => {
            println!("  verdict    : compressed — похоже на сжатые данные (структура скрыта)")
        }
        _ => println!(
            "  verdict    : encrypted-like — похоже на шифр/случайность; без ключа не читать"
        ),
    }

    // ── Побайтовая карта заголовка .pqw ──
    if let Some(r) = &reader {
        println!("\nBYTE MAP (header 0x00..0x80)");
        println!("  offset size  field                    value");
        for row in header_rows(data, r) {
            println!(
                "  0x{:04x} {:>4}   {:<24} {}",
                row.offset, row.size, row.name, row.value
            );
        }
        println!("  секции: header 0x00..0x80 → topology → phases → EOF");
    }

    // ── Hex-дамп ──
    if let Some(limit) = cfg.hex_limit {
        let shown = limit.min(size);
        println!("\nHEX (first {shown} B of {size})");
        print!("{}", hex_dump(data, shown));
        if shown < size {
            println!("  … (обрезано; --hex all — весь файл)");
        }
    }

    // ── Декод дуг ──
    if let Some(limit) = cfg.decode {
        if !arcs.is_empty() {
            let shown = limit.min(arcs.len());
            println!("\nARCS ({} of {}; --decode all — все)", shown, arcs.len());
            println!("       idx   hex   trit   p̂        θ̂");
            print!("{}", arcs_csr_text(&arcs[..shown]));
        }
    }

    // ── Граф LENS ──
    if cfg.graph && !arcs.is_empty() {
        let indices: Vec<u32> = arcs.iter().map(|a| a.index).collect();
        let stats = graph_stats(&indices, d_pol);
        println!("\nGRAPH (LENS: рёбра = соседние дуги u_k → u_k+1)");
        println!(
            "  nodes {}, edges {}, components {}, density {:.3}%",
            stats.nodes,
            stats.edges,
            stats.components,
            stats.density * 100.0
        );
        let csr: Vec<String> = indices.iter().map(|i| i.to_string()).collect();
        println!("  CSR indices: [{}]", csr.join(", "));
    }

    // ── DOT ──
    if let Some(target) = &cfg.dot {
        let dot = arcs_dot(&arcs, d_pol);
        if target == "-" {
            println!("\nDOT");
            print!("{dot}");
        } else {
            match std::fs::write(target, dot) {
                Ok(()) => println!("\nDOT       : written to {target}"),
                Err(e) => {
                    eprintln!("pqc inspect: cannot write --dot {target}: {e}");
                    return 1;
                }
            }
        }
    }

    // ── ASCII-матрица ──
    if cfg.matrix {
        if d_pol == 0 {
            println!("\nMATRIX    : нет дуг — матрица пуста");
        } else if let Some(m) = ascii_matrix(&arcs, d_pol) {
            println!("\nMATRIX (adjacency, d_pol = {d_pol}; '#' — активная дуга, 'X' — ребро)");
            print!("{m}");
        } else {
            println!(
                "\nMATRIX    : пропущена — d_pol = {d_pol} > {MATRIX_D_MAX} (матрица осмысленна при d_pol ≤ {MATRIX_D_MAX})"
            );
        }
    }

    // ── Строки ──
    if cfg.strings {
        let strings = ascii_strings(data, 6);
        if strings.is_empty() {
            println!("\nSTRINGS   : — (печатаемых ASCII-строк ≥ 6 нет)");
        } else {
            println!("\nSTRINGS (ASCII ≥ 6, первые {})", strings.len());
            for s in &strings {
                println!("  {s}");
            }
        }
    }

    // ── Целостность (для контейнеров .pqw) ──
    if let Some(r) = &reader {
        println!("\nINTEGRITY");
        println!("  header checksum : OK (FNV-1a64, валидирована при разборе)");
        match r.verify_payload() {
            Ok(()) => println!("  payload digest  : OK (SHA-256 trunc-24)"),
            Err(e) => println!("  payload digest  : FAIL — {e}"),
        }
        println!("  mcweeny stored  : {:.3e}", r.mcweeny_residual());
        let residual = pqc::inspect::mcweeny_of_arcs(&arcs);
        if residual.is_finite() {
            println!("  mcweeny actual  : {residual:.3e} (max |λ² − λ| по дугам)");
        }
    } else if kind.is_pqw() {
        println!("\nINTEGRITY");
        println!("  недоступна: контейнер не разобран (см. parse ERROR выше)");
    }

    0
}

fn cmd_run(args: &[String]) -> i32 {
    let mut cfg = RunConfig::default();
    let mut file = None;
    if let Err(e) = parse_common(args, &mut cfg, &mut file) {
        eprintln!("pqc run: {e}\n\n{USAGE}");
        return 2;
    }
    let Some(file) = file else {
        eprintln!("pqc run: file required\n\n{USAGE}");
        return 2;
    };
    let src = match load(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pqc run: {e}");
            return 1;
        }
    };
    let reader = match PqwReader::from_bytes(src.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc run: {e}");
            return 1;
        }
    };
    match run_reader(&file, src.kind(), src.as_slice().len(), &reader, &cfg) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("pqc run: {e}");
            1
        }
    }
}

fn cmd_demo(args: &[String]) -> i32 {
    let mut cfg = RunConfig {
        shots: 4096,
        ..RunConfig::default()
    };
    let mut n: usize = 12;

    // --n — опция только demo: вырезаем её до общего разбора.
    let mut rest: Vec<String> = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--n" {
            let Some(v) = args.get(i + 1) else {
                eprintln!("pqc demo: missing value for --n\n\n{USAGE}");
                return 2;
            };
            match v.parse::<usize>() {
                Ok(val) => n = val,
                Err(_) => {
                    eprintln!("pqc demo: bad --n: {v}");
                    return 2;
                }
            }
            i += 2;
        } else {
            rest.push(args[i].clone());
            i += 1;
        }
    }
    let mut file = None;
    if let Err(e) = parse_common(&rest, &mut cfg, &mut file) {
        eprintln!("pqc demo: {e}\n\n{USAGE}");
        return 2;
    }
    if let Some(f) = file {
        eprintln!("pqc demo: unexpected argument {f}");
        return 2;
    }
    if n == 0 || n > 1_000_000 {
        eprintln!("pqc demo: --n must be in [1, 1000000]");
        return 2;
    }

    // Детерминированный фазовый вектор: смешанный фон и спайки.
    let mut ps = vec![0.0_f32; n];
    for (i, p) in ps.iter_mut().enumerate() {
        let x = (i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x1234_5678_9ABC_DEF0);
        let v = ((x >> 33) % 2001) as i64 - 1000;
        *p = v as f32 / 1000.0;
    }
    let mut w = match PqwWriter::new(n as u32) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("pqc demo: {e}");
            return 1;
        }
    };
    w = w.hyperparams(0.01, 0.1, 0.99, 0.1);
    if let Err(e) = w.add_state(&ps) {
        eprintln!("pqc demo: {e}");
        return 1;
    }
    let bytes = match w.to_bytes() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pqc demo: {e}");
            return 1;
        }
    };
    let reader = match PqwReader::from_bytes(&bytes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc demo: {e}");
            return 1;
        }
    };
    match run_reader("demo (in-memory)", "mem", bytes.len(), &reader, &cfg) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("pqc demo: {e}");
            1
        }
    }
}

// ============================================================================
// pqc precess — RQ11: петля архетипа поверх чекпоинта.
//
// Уравнение архетипа в алгебре смыслов (O, ⊕, ⊗_ε):
//   * a ⊗_ε a = a      — идемпотентность (McWeeny / спектральные проекторы мод);
//   * p* = a ⊗_ε p*    — фиксация (неподвижная точка транспорта фаз).
//
// Петля: транспорт фаз precess_step (унитарный, O(nnz(J))) с опциональной
// проекцией McWeeny каждые K тиков (диссипативная половина). Чистая
// прецессия орбитирует вокруг архетипа (спектр J мнимый); с очисткой
// фазы садятся на триты {0, ±1} при погашенных моментах — контейнер-
// архетип с нулевым остатком идемпотентности.
// ============================================================================

struct PrecessConfig {
    eta: Option<f64>,
    ticks: u64,
    delta: f64,
    purify_every: u64,
    out: Option<String>,
    trace: bool,
    json: bool,
}

impl Default for PrecessConfig {
    fn default() -> PrecessConfig {
        PrecessConfig {
            eta: None,
            ticks: 16_384,
            delta: 1e-9,
            purify_every: 0,
            out: None,
            trace: false,
            json: false,
        }
    }
}

fn cmd_precess(args: &[String]) -> i32 {
    let mut cfg = PrecessConfig::default();
    let mut file: Option<String> = None;
    let mut i = 0usize;
    macro_rules! val {
        ($name:literal) => {{
            i += 1;
            if i >= args.len() {
                eprintln!("pqc precess: {} требует значение\n\n{USAGE}", $name);
                return 2;
            }
            args[i].clone()
        }};
    }
    while i < args.len() {
        match args[i].as_str() {
            "--eta" => {
                let v = val!("--eta");
                match v.parse::<f64>() {
                    Ok(h) if h > 0.0 && h.is_finite() => cfg.eta = Some(h),
                    _ => {
                        eprintln!("pqc precess: bad --eta: {v} (> 0)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--ticks" => {
                let v = val!("--ticks");
                match v.parse::<u64>() {
                    Ok(n) if n > 0 && n <= 100_000_000 => cfg.ticks = n,
                    _ => {
                        eprintln!("pqc precess: bad --ticks: {v} (1..=1e8)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--delta" => {
                let v = val!("--delta");
                match v.parse::<f64>() {
                    Ok(d) if d > 0.0 && d.is_finite() => cfg.delta = d,
                    _ => {
                        eprintln!("pqc precess: bad --delta: {v} (> 0)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--purify-every" => {
                let v = val!("--purify-every");
                match v.parse::<u64>() {
                    Ok(k) if k <= 1024 => cfg.purify_every = k,
                    _ => {
                        eprintln!("pqc precess: bad --purify-every: {v} (0..=1024)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--out" => cfg.out = Some(val!("--out")),
            "--trace" => cfg.trace = true,
            "--json" => cfg.json = true,
            other if !other.starts_with("--") => {
                if file.replace(other.to_string()).is_some() {
                    eprintln!("pqc precess: file given twice\n\n{USAGE}");
                    return 2;
                }
            }
            other => {
                eprintln!("pqc precess: неизвестный флаг {other}\n\n{USAGE}");
                return 2;
            }
        }
        i += 1;
    }
    let Some(file_path) = file else {
        eprintln!("pqc precess: требуется путь к контейнеру .poler / .pqw (v3 с --gyro)\n\n{USAGE}");
        return 2;
    };

    let raw = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pqc precess: ошибка чтения {file_path}: {e}");
            return 1;
        }
    };
    let reader = match PqwReader::from_bytes(&raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc precess: ошибка парсинга {file_path}: {e}");
            return 1;
        }
    };
    let Some(section) = reader.gyro() else {
        eprintln!(
            "pqc precess: контейнер без топологической секции (v1/v2) — \\\n\
             петле архетипа нужен гироскоп J = A − Aᵀ; обучите с --gyro"
        );
        return 1;
    };

    let d_pol = reader.d_pol() as usize;
    let hyper = reader.hyperparams();
    let eta = cfg.eta.unwrap_or(f64::from(hyper.eta));

    // Вектор фаз: θ = arccos(p̂) для хранимых дуг, фон π/2 (p = 0).
    let mut thetas = vec![std::f64::consts::FRAC_PI_2; d_pol];
    for (idx, p) in reader.decoded() {
        if (idx as usize) < d_pol {
            thetas[idx as usize] = p.clamp(-1.0, 1.0).acos();
        }
    }

    let pairs: Vec<(u32, u32, f64)> = section
        .pairs()
        .iter()
        .map(|p| (p.i, p.j, p.weight))
        .collect();
    // Дуги русел J (уникальные) — для сводки.
    let mut channel_arcs: Vec<u32> = pairs.iter().flat_map(|&(i, j, _)| [i, j]).collect();
    channel_arcs.sort_unstable();
    channel_arcs.dedup();

    if !cfg.json {
        println!("POLER Quantum Core — Precess: петля архетипа (RQ11)");
        println!("container : {file_path} ({} B)", raw.len());
        println!(
            "d_pol     : {d_pol} | фазы: {} дуг | русла J: {} пар, {} дуг | такты гироскопа: {}",
            reader.nnz(),
            pairs.len(),
            channel_arcs.len(),
            section.ticks()
        );
        println!(
            "eta       : {eta:.4} ({}) | лимит: {} тиков | стоп: |Δθ| < {:.0e} | McWeeny: {}",
            if cfg.eta.is_some() { "флаг" } else { "из контейнера" },
            cfg.ticks,
            cfg.delta,
            match cfg.purify_every {
                0 => "выключена (чистая прецессия)".to_string(),
                k => format!("каждые {k} тиков"),
            }
        );
        if cfg.purify_every == 0 {
            println!("режим     : унитарная прецессия — орбита вокруг архетипа (спектр J мнимый);");
            println!("            для оседания к тритам добавьте --purify-every 4");
        }
    }

    // Петля.
    let report = pqc::archetype::precess_to_fixpoint(
        &pairs,
        &mut thetas,
        eta,
        cfg.ticks,
        cfg.delta,
        cfg.purify_every,
    );

    if cfg.json {
        let mut obj = vec![
            ("file".into(), Json::str(&file_path)),
            ("d_pol".into(), Json::num(d_pol as f64)),
            ("nnz".into(), Json::num(reader.nnz() as f64)),
            ("eta".into(), Json::num(eta)),
            ("ticks_limit".into(), Json::num(cfg.ticks as f64)),
            ("stop_delta".into(), Json::num(cfg.delta)),
            ("purify_every".into(), Json::num(cfg.purify_every as f64)),
            ("gyro_pairs".into(), Json::num(pairs.len() as f64)),
            ("gyro_ticks".into(), Json::num(section.ticks() as f64)),
            ("fixated".into(), Json::Bool(report.fixated)),
            ("ticks".into(), Json::num(report.ticks as f64)),
            ("participants".into(), Json::num(report.participants as f64)),
            ("delta_first".into(), Json::num(report.delta_first)),
            ("delta_last".into(), Json::num(report.delta_last)),
            ("contraction".into(), Json::num(report.contraction)),
            ("r_initial".into(), Json::num(report.r_initial)),
            ("r_final".into(), Json::num(report.r_final)),
            ("torque_initial".into(), Json::num(report.torque_initial)),
            ("torque_final".into(), Json::num(report.torque_final)),
            ("omega".into(), Json::num(report.omega)),
            ("travel_max".into(), Json::num(report.travel_max)),
            ("travel_mean".into(), Json::num(report.travel_mean)),
            ("trits_exact".into(), Json::Bool(report.trits_exact)),
        ];
        let trace: Vec<Json> = report
            .trace
            .iter()
            .map(|p| {
                Json::Obj(vec![
                    ("tick".into(), Json::num(p.tick as f64)),
                    ("delta".into(), Json::num(p.delta)),
                    ("r".into(), Json::num(p.r)),
                    ("torque".into(), Json::num(p.torque)),
                ])
            })
            .collect();
        obj.push(("trace".into(), Json::Arr(trace)));
        println!("{}", Json::Obj(obj).to_string());
    } else {
        println!("\nтик        max|Δθ|      r       torque");
        let print_point = |p: &pqc::archetype::TracePoint| {
            println!(
                "{:>8}  {:>10.3e}  {:7.4}  {:8.2e}{}",
                p.tick,
                p.delta,
                p.r,
                p.torque,
                if p.tick == report.ticks && report.fixated {
                    "   ← фиксация"
                } else {
                    ""
                }
            );
        };
        if cfg.trace {
            for p in &report.trace {
                print_point(p);
            }
        } else {
            let n = report.trace.len();
            for p in &report.trace[..n.min(4)] {
                print_point(p);
            }
            if n > 7 {
                println!("       ... ({} точек, --trace — вся траектория)", n);
                for p in &report.trace[n - 3..] {
                    print_point(p);
                }
            }
        }

        println!("\nРЕЗУЛЬТАТ");
        println!(
            "фиксация     : {}",
            if report.fixated {
                format!("достигнута за {} тиков (лимит {})", report.ticks, cfg.ticks)
            } else if report.torque_final < 1e-6 {
                format!(
                    "квази-фиксация: невязка p* = {:.1e}, остаточная микро-орбита |Δθ| = {:.1e} \
                     (меньше η — например --eta 0.05 — снимает её)",
                    report.torque_final, report.delta_last
                )
            } else {
                format!("нет за {} тиков — орбита/маховик", report.ticks)
            }
        );
        println!(
            "сжатие ρ     : {:.5} за тик ({})",
            report.contraction,
            if report.contraction < 1.0 {
                "Банах: отображение сжимающее"
            } else {
                "сжатия нет — унитарный транспорт"
            }
        );
        println!(
            "r (Курамото) : {:.4} → {:.4}",
            report.r_initial, report.r_final
        );
        println!(
            "невязка p*   : torque {:.2e} → {:.2e}   (уравнение p* = a ⊗_ε p*)",
            report.torque_initial, report.torque_final
        );
        if !report.fixated && report.torque_final >= 1e-6 {
            println!(
                "маховик ω    : {:+.6} рад/тик (циркуляция не утихает)",
                report.omega
            );
        }
        println!(
            "пробег дуг   : max {:.3} рад, средний {:.3} рад",
            report.travel_max, report.travel_mean
        );
        println!(
            "триты        : {}",
            if report.trits_exact {
                "точные (|cos θ| ∈ {0,1}) — идемпотентность a ⊗_ε a = a"
            } else {
                "нет (фазы вне решётки {0, π/2, π})"
            }
        );
        let both = report.trits_exact && report.torque_final < 1e-6;
        println!(
            "архетип      : {}",
            if both {
                "ДОСТИГНУТ — оба уравнения выполнены"
            } else if report.torque_final < 1e-6 {
                "фиксация без идемпотентности (добавьте --purify-every 2..4)"
            } else {
                "вращение (орбита вокруг архетипа)"
            }
        );
    }

    // Запись итогового состояния: контейнер v3, фазы p = cos θ,
    // русла J переносятся без изменений (веса исходного масштаба).
    if let Some(out_path) = &cfg.out {
        let state_f32: Vec<f32> = thetas.iter().map(|&t| t.cos() as f32).collect();
        let gyro_pairs: Vec<(u32, u32, f64)> = section
            .pairs()
            .iter()
            .map(|p| (p.i, p.j, p.weight))
            .collect();
        let write = (|| -> Result<usize, String> {
            let mut w = PqwWriter::new(d_pol as u32)
                .map_err(|e| e.to_string())?
                .hyperparams(hyper.eta, hyper.gamma, hyper.rho, hyper.epsilon_threshold);
            w.add_state(&state_f32).map_err(|e| e.to_string())?;
            let data = pqw::GyroData::new(section.window(), section.ticks(), gyro_pairs, d_pol as u32)
                .map_err(|e| e.to_string())?;
            let mut buf = Vec::new();
            w.write_v3(&mut buf, &data).map_err(|e| e.to_string())?;
            std::fs::write(out_path, &buf).map_err(|e| e.to_string())?;
            Ok(buf.len())
        })();
        match write {
            Ok(len) => {
                if !cfg.json {
                    let back = PqwReader::from_bytes(&std::fs::read(out_path).unwrap_or_default())
                        .ok()
                        .map(|r| r.nnz());
                    println!(
                        "\nзаписано     : {out_path} (v3, {len} B, {} дуг, {} пар J)",
                        back.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                        section.pairs().len()
                    );
                    if !report.trits_exact {
                        println!("               (снимок орбиты — фазы не на тритовой решётке)");
                    }
                }
            }
            Err(e) => {
                eprintln!("pqc precess: --out: {e}");
                return 1;
            }
        }
    }

    0
}

// pqc encrypt / decrypt — RQ12: крипто-схема алгебры архетипа (файл 285).
//
// Уравнение: p* = a ⊗_ε p* ⊕ m, восстановление m = p* ⊕ (a ⊗_ε p*).
// Ключ — контейнер v3 с гироскопом J (готовый `precess --out`):
// из русел J детерминированно берутся плотные моды Im(P), проектор
// a = Σ (u uᵀ + v vᵀ) идемпотентен (RQ11: ортонормальность ≡
// идемпотентность), сообщение — паттерн δ = (I−a)(ε·b) в дополнении
// подпространства мод, ключевой поток — a·p₀ (проекция фаз ключа).

struct CryptoConfig {
    key: Option<String>,
    out: Option<String>,
    modes: usize,
    seed: Option<u64>,
    text: Option<String>,
    stdin: bool,
    json: bool,
    /// RQ12-схема на f32-фазах (legacy; трит-схема GF(3) — по умолчанию).
    legacy_f32: bool,
    /// RQ23: линейная трит-схема RQ13 v1 (чистый транспорт, без
    /// нелинейного спинового слоя) — дефолт теперь v2.
    linear_v1: bool,
    /// RQ23: размер сообщения для команды avalanche (байт).
    size: usize,
    /// RQ23: число зондов лавины.
    probes: usize,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        CryptoConfig {
            key: None,
            out: None,
            modes: 0,
            seed: None,
            text: None,
            stdin: false,
            json: false,
            legacy_f32: false,
            linear_v1: false,
            size: 0,
            probes: 0,
        }
    }
}

/// Общий парсер флагов encrypt/decrypt.
fn parse_crypto_args(cmd: &str, args: &[String]) -> Result<(Option<String>, CryptoConfig), i32> {
    let mut cfg = CryptoConfig::default();
    let mut file: Option<String> = None;
    let mut i = 0usize;
    macro_rules! val {
        ($name:literal) => {{
            i += 1;
            if i >= args.len() {
                eprintln!("pqc {cmd}: {} требует значение\n\n{USAGE}", $name);
                return Err(2);
            }
            args[i].clone()
        }};
    }
    while i < args.len() {
        match args[i].as_str() {
            "--key" => cfg.key = Some(val!("--key")),
            "--out" => cfg.out = Some(val!("--out")),
            "--modes" => {
                let v = val!("--modes");
                match v.parse::<usize>() {
                    Ok(k) if k <= pqc::MAX_MODES => cfg.modes = k,
                    _ => {
                        eprintln!("pqc {cmd}: bad --modes: {v} (1..={})", pqc::MAX_MODES);
                        return Err(2);
                    }
                }
            }
            "--seed" => {
                let v = val!("--seed");
                match v.parse::<u64>() {
                    Ok(sd) => cfg.seed = Some(sd),
                    _ => {
                        eprintln!("pqc {cmd}: bad --seed: {v} (u64)");
                        return Err(2);
                    }
                }
            }
            "--text" => cfg.text = Some(val!("--text")),
            "--stdin" => cfg.stdin = true,
            "--json" => cfg.json = true,
            "--f32" => cfg.legacy_f32 = true,
            "--linear" => cfg.linear_v1 = true,
            "--size" => {
                let v = val!("--size");
                match v.parse::<usize>() {
                    Ok(n) if n > 0 && n <= 16 * 1024 * 1024 => cfg.size = n,
                    _ => {
                        eprintln!("pqc {cmd}: bad --size: {v} (1..=16 МиБ)");
                        return Err(2);
                    }
                }
            }
            "--probes" => {
                let v = val!("--probes");
                match v.parse::<usize>() {
                    Ok(n) if n > 0 && n <= 256 => cfg.probes = n,
                    _ => {
                        eprintln!("pqc {cmd}: bad --probes: {v} (1..=256)");
                        return Err(2);
                    }
                }
            }
            other if !other.starts_with("--") => {
                if file.replace(other.to_string()).is_some() {
                    eprintln!("pqc {cmd}: файл задан дважды\n\n{USAGE}");
                    return Err(2);
                }
            }
            other => {
                eprintln!("pqc {cmd}: неизвестный флаг {other}\n\n{USAGE}");
                return Err(2);
            }
        }
        i += 1;
    }
    Ok((file, cfg))
}

/// Загрузка ключа-архетипа.
fn load_cipher_key(path: &str, modes: usize) -> Result<pqc::CipherKey, i32> {
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pqc crypto: ошибка чтения ключа {path}: {e}");
            return Err(1);
        }
    };
    let reader = match PqwReader::from_bytes(&raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc crypto: ошибка парсинга ключа {path}: {e}");
            return Err(1);
        }
    };
    match pqc::CipherKey::from_reader(&reader, modes) {
        Ok(k) => Ok(k),
        Err(e) => {
            eprintln!("pqc crypto: {e}");
            Err(1)
        }
    }
}

fn load_trite_key(path: &str, modes: usize) -> Result<pqc::trite::TritKey, i32> {
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pqc crypto: ошибка чтения ключа {path}: {e}");
            return Err(1);
        }
    };
    let reader = match PqwReader::from_bytes(&raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc crypto: ошибка парсинга ключа {path}: {e}");
            return Err(1);
        }
    };
    match pqc::trite::TritKey::from_reader(&reader, modes) {
        Ok(k) => Ok(k),
        Err(e) => {
            eprintln!("pqc crypto: {e}");
            Err(1)
        }
    }
}

fn cmd_encrypt(args: &[String]) -> i32 {
    let (file, cfg) = match parse_crypto_args("encrypt", args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let Some(key_path) = &cfg.key else {
        eprintln!("pqc encrypt: требуется --key <ARCH.pqw> (контейнер v3 с гироскопом)");
        return 2;
    };
    let Some(out_path) = &cfg.out else {
        eprintln!("pqc encrypt: требуется --out <CIPHER.pqt>");
        return 2;
    };
    // Сообщение: --text | --stdin | файл.
    let msg: Vec<u8> = if let Some(text) = &cfg.text {
        text.clone().into_bytes()
    } else if cfg.stdin {
        use std::io::Read;
        let mut buf = Vec::new();
        if std::io::stdin().read_to_end(&mut buf).is_err() {
            eprintln!("pqc encrypt: ошибка чтения stdin");
            return 1;
        }
        buf
    } else if let Some(path) = &file {
        match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("pqc encrypt: ошибка чтения {path}: {e}");
                return 1;
            }
        }
    } else {
        eprintln!("pqc encrypt: нужен вход: файл, --text или --stdin");
        return 2;
    };

    if !cfg.legacy_f32 {
        return cmd_encrypt_trite(key_path, out_path, &msg, &cfg);
    }

    let key = match load_cipher_key(key_path, cfg.modes) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let mut rng = match cfg.seed {
        Some(s) => pqc::Rng::seed_from_u64(s),
        None => pqc::Rng::from_entropy(),
    };
    let t0 = std::time::Instant::now();
    let (cipher, rep) = match pqc::encrypt(&key, &msg, &mut rng) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc encrypt: {e}");
            return 1;
        }
    };
    let dt = t0.elapsed();
    if let Err(e) = std::fs::write(out_path, &cipher) {
        eprintln!("pqc encrypt: ошибка записи {out_path}: {e}");
        return 1;
    }

    if cfg.json {
        let obj = pqc::Json::Obj(vec![
            ("key".into(), pqc::Json::str(key_path)),
            ("out".into(), pqc::Json::str(out_path)),
            ("msg_len".into(), pqc::Json::num(rep.msg_len as f64)),
            ("out_len".into(), pqc::Json::num(rep.out_len as f64)),
            ("blocks".into(), pqc::Json::num(rep.blocks as f64)),
            ("d_pol".into(), pqc::Json::num(key.d_pol as f64)),
            ("k_modes".into(), pqc::Json::num(rep.k_modes as f64)),
            ("raw_pairs".into(), pqc::Json::num(key.raw_pairs as f64)),
            ("interference_max".into(), pqc::Json::num(rep.interference_max)),
            ("margin_min".into(), pqc::Json::num(rep.margin_min)),
            ("ritz_max".into(), pqc::Json::num(key.ritz_max)),
            ("ortho_max".into(), pqc::Json::num(key.ortho_max)),
            ("digest".into(), pqc::Json::str(&rep.digest_hex)),
            ("seconds".into(), pqc::Json::num(dt.as_secs_f64())),
        ]);
        println!("{}", obj.to_string());
    } else {
        println!("POLER Quantum Core — Encrypt: крипто-схема алгебры архетипа (RQ12)");
        println!("key       : {key_path} (d_pol={}, русел J={}, мод K={})", key.d_pol, key.raw_pairs, key.k_modes);
        println!("            Ritz {:.1e} | орто {:.1e} (идемпотентность a ⊗ a = a)", key.ritz_max, key.ortho_max);
        println!("message   : {} B → {} блоков", rep.msg_len, rep.blocks);
        println!("cipher    : {out_path} ({} B, расширение ×{:.1})", rep.out_len, rep.out_len as f64 / (rep.msg_len.max(1)) as f64);
        println!("помеха    : max |a·(ε·b)| = {:.4} (порог ε/2 = {:.4})", rep.interference_max, pqc::crypto::DECODE_THRESHOLD);
        println!("запас     : min |δ̂ − ε/2| = {:.4}", rep.margin_min);
        println!("digest    : sha256-24 = {}", rep.digest_hex);
        println!("время     : {:.3} с", dt.as_secs_f64());
        println!("уравнение : p* = a ⊗_ε p* ⊕ m — коллапс за 1 такт (a идемпотентен)");
    }
    0
}

/// RQ13: шифрование трит-схемы GF(3) (по умолчанию).
fn cmd_encrypt_trite(key_path: &str, out_path: &str, msg: &[u8], cfg: &CryptoConfig) -> i32 {
    let key = match load_trite_key(key_path, cfg.modes) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let mut rng = match cfg.seed {
        Some(s) => pqc::Rng::seed_from_u64(s),
        None => pqc::Rng::from_entropy(),
    };
    let t0 = std::time::Instant::now();
    // RQ23: дефолт — нелинейная схема v2 (транспорт + спин-раунд GF(3));
    // --linear возвращает чистый транспорт RQ13 (v1).
    let (cipher, rep) = if cfg.linear_v1 {
        match pqc::trite::encrypt_v1(&key, msg, &mut rng) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("pqc encrypt: {e}");
                return 1;
            }
        }
    } else {
        match pqc::trite::encrypt(&key, msg, &mut rng) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("pqc encrypt: {e}");
                return 1;
            }
        }
    };
    let dt = t0.elapsed();
    if let Err(e) = std::fs::write(out_path, &cipher) {
        eprintln!("pqc encrypt: ошибка записи {out_path}: {e}");
        return 1;
    }

    if cfg.json {
        let obj = pqc::Json::Obj(vec![
            ("key".into(), pqc::Json::str(key_path)),
            ("out".into(), pqc::Json::str(out_path)),
            ("scheme".into(), pqc::Json::str("trite-gf3")),
            ("version".into(), pqc::Json::num(rep.version as f64)),
            ("msg_len".into(), pqc::Json::num(rep.msg_len as f64)),
            ("out_len".into(), pqc::Json::num(rep.out_len as f64)),
            ("blocks".into(), pqc::Json::num(rep.blocks as f64)),
            ("d_pol".into(), pqc::Json::num(key.d_pol as f64)),
            ("k_modes".into(), pqc::Json::num(rep.k_modes as f64)),
            ("capacity_trites".into(), pqc::Json::num(rep.capacity_trites as f64)),
            ("raw_pairs".into(), pqc::Json::num(key.raw_pairs as f64)),
            ("pairs_total".into(), pqc::Json::num(rep.pairs_total as f64)),
            ("ticks".into(), pqc::Json::num(rep.ticks as f64)),
            ("nl_rounds".into(), pqc::Json::num(rep.nl_rounds as f64)),
            ("avalanche".into(), pqc::Json::num(rep.avalanche)),
            ("expansion".into(), pqc::Json::num(rep.expansion)),
            ("ritz_max".into(), pqc::Json::num(key.ritz_max)),
            ("ortho_max".into(), pqc::Json::num(key.ortho_max)),
            ("digest".into(), pqc::Json::str(&rep.digest_hex)),
            ("seconds".into(), pqc::Json::num(dt.as_secs_f64())),
        ]);
        println!("{}", obj.to_string());
    } else {
        let scheme_name = if rep.version == pqc::trite::TRITE_VERSION_V2 {
            "нелинейная v2: транспорт + спин-раунд GF(3) (RQ23)"
        } else {
            "линейная v1: чистый транспорт (RQ13)"
        };
        println!("POLER Quantum Core — Encrypt: трит-схема алгебры архетипа (GF(3), {scheme_name})");
        println!("key       : {key_path} (d_pol={}, русел J={}, мод K={})", key.d_pol, key.raw_pairs, key.k_modes);
        println!("            Ritz {:.1e} | орто {:.1e} (идемпотентность a ⊗ a = a)", key.ritz_max, key.ortho_max);
        println!("message   : {} B → {} блоков × {} трит (упаковка {}↔{})", rep.msg_len, rep.blocks, rep.capacity_trites, 19, 30);
        println!("cipher    : {out_path} ({} B, расширение ×{:.2} — было ×43 в RQ12)", rep.out_len, rep.expansion);
        println!("диффузия  : {} тактов × {} русл (J {} + решётка LENS {}){}",
            rep.ticks,
            rep.pairs_total,
            key.raw_pairs,
            rep.pairs_total - key.raw_pairs,
            if rep.nl_rounds > 0 {
                format!(" + {} спин-раундов (квадратичный T-проход)", rep.nl_rounds)
            } else {
                String::new()
            }
        );
        println!("лавина    : {:.1}% трит блока от 1 бита, тот же IV (потолок GF(3) 66.7%)", 100.0 * rep.avalanche);
        println!("digest    : sha256-24 = {}", rep.digest_hex);
        println!("время     : {:.3} с", dt.as_secs_f64());
        println!("уравнение : p* = a ⊗_ε p* ⊕ m — Packed4-триты, расшифровка побитово точна (GF(3))");
    }
    0
}

/// `pqc avalanche --key F.pqw [--size N] [--probes P] [--linear] [--json]`:
/// измерение нелинейной спиновой лавины GF(3) на большом блоке данных
/// (RQ23). Псевдослучайный текст детерминирован сидом (--seed, дефолт 42):
/// одинаковые прогоны сравнимы побитово.
fn cmd_avalanche(args: &[String]) -> i32 {
    let (file, cfg) = match parse_crypto_args("avalanche", args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let Some(key_path) = &cfg.key else {
        eprintln!("pqc avalanche: требуется --key <ARCH.pqw> (контейнер v3+ с гироскопом)");
        return 2;
    };
    if file.is_some() {
        eprintln!("pqc avalanche: вход не нужен — текст генерируется сидом (--size)");
        return 2;
    }
    let size = if cfg.size > 0 { cfg.size } else { 65_536 };
    let probes = if cfg.probes > 0 { cfg.probes } else { 16 };
    let seed = cfg.seed.unwrap_or(42);

    let key = match load_trite_key(key_path, cfg.modes) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let msg = pqc::spin_avalanche::msg_from_seed(seed, size);
    let t0 = std::time::Instant::now();
    let stats = match pqc::spin_avalanche::measure_avalanche(&key, &msg, probes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pqc avalanche: {e}");
            return 1;
        }
    };
    let dt = t0.elapsed();
    // Линейный прогон для сравнения слоёв: та же схема измерения
    // (флип первого зонда, тот же IV-сид), но тело v1 без спина.
    let (linear_mean, linear_spread) = if cfg.linear_v1 {
        (None, None)
    } else {
        let iv_seed = 0xA11CE_u64;
        let mut rng = pqc::Rng::seed_from_u64(iv_seed);
        let (_, data, _) = key.encrypt_body_version(&msg, &mut rng, pqc::trite::TRITE_VERSION);
        // Флип того же бита, что и первый зонд.
        let mut flipped = msg.clone();
        flipped[0] ^= 1;
        let mut rng3 = pqc::Rng::seed_from_u64(iv_seed);
        let (_, data3, _) = key.encrypt_body_version(&flipped, &mut rng3, pqc::trite::TRITE_VERSION);
        let n = data.len() * 4;
        let t1 = pqc::trite::unpack_trites(&data, n);
        let t3 = pqc::trite::unpack_trites(&data3, n);
        let av = t1
            .iter()
            .zip(t3.iter())
            .filter(|(a, b)| a != b)
            .count() as f64
            / n as f64;
        let spread = pqc::spin_avalanche::transform_spread(
            key.pairs(),
            key.keystream(),
            key.d_pol,
            key.positions()[0] as usize,
            key.ticks,
        );
        (Some(av), Some(spread))
    };

    if cfg.json {
        let mut obj = vec![
            ("key".into(), pqc::Json::str(key_path)),
            ("scheme".into(), pqc::Json::str("spin-avalanche-gf3")),
            ("version".into(), pqc::Json::num(pqc::trite::TRITE_VERSION_V2 as f64)),
            ("size".into(), pqc::Json::num(size as f64)),
            ("probes".into(), pqc::Json::num(probes as f64)),
            ("seed".into(), pqc::Json::num(seed as f64)),
            ("d_pol".into(), pqc::Json::num(key.d_pol as f64)),
            ("blocks".into(), pqc::Json::num(stats.blocks as f64)),
            ("ticks".into(), pqc::Json::num(stats.ticks as f64)),
            ("nl_rounds".into(), pqc::Json::num(stats.nl_rounds as f64)),
            ("ticks_linear".into(), pqc::Json::num(key.ticks as f64)),
            ("avalanche_mean".into(), pqc::Json::num(stats.avalanche_mean)),
            (
                "avalanche_from_probe".into(),
                pqc::Json::num(stats.avalanche_from_probe),
            ),
            ("avalanche_min".into(), pqc::Json::num(stats.avalanche_min)),
            ("avalanche_max".into(), pqc::Json::num(stats.avalanche_max)),
            ("ceiling".into(), pqc::Json::num(stats.ceiling)),
            ("cascade_after".into(), pqc::Json::num(stats.cascade_after)),
            (
                "transform_spread_mean".into(),
                pqc::Json::num(stats.transform_spread_mean),
            ),
            ("chi2_blocks".into(), pqc::Json::num(stats.chi2_blocks)),
            ("seconds".into(), pqc::Json::num(dt.as_secs_f64())),
        ];
        if let Some(lm) = linear_mean {
            obj.push(("linear_avalanche_probe".into(), pqc::Json::num(lm)));
        }
        if let Some(ls) = linear_spread {
            obj.push(("linear_spread_probe".into(), pqc::Json::num(ls)));
        }
        println!("{}", pqc::Json::Obj(obj).to_string());
    } else {
        println!("POLER Quantum Core — Avalanche: нелинейная спиновая лавина GF(3) (RQ23)");
        println!("key       : {key_path} (d_pol={}, русел J={})", key.d_pol, key.raw_pairs);
        println!("данные    : {size} B (сид {seed}) → {} блоков × {} трит, зондов {probes}", stats.blocks, stats.block_trites);
        println!("схема     : v2 — такт = транспорт + спин-раунд ({} тактов); линейная v1 — {} тактов", stats.ticks, key.ticks);
        println!("лавина    : {:.1}% трит всего шифртекста (мин {:.1}% / макс {:.1}%)",
            100.0 * stats.avalanche_mean, 100.0 * stats.avalanche_min, 100.0 * stats.avalanche_max);
        println!("            от блока зонда вперёд — {:.1}% (честная диффузия; префикс CBC не трогает), потолок GF(3) {:.1}%",
            100.0 * stats.avalanche_from_probe, 100.0 * stats.ceiling);
        println!("каскад    : {:.1}% трит в блоках ПОСЛЕ зонда (цепочка CBC разносит замену вперёд)", 100.0 * stats.cascade_after);
        println!("трансформ : {:.1}% позиций блока от одной триты (зонды состояния)", 100.0 * stats.transform_spread_mean);
        println!("хи-квадрат: {:.2} по {} блокам (равномерность изменений)", stats.chi2_blocks, stats.blocks);
        if let (Some(lm), Some(ls)) = (linear_mean, linear_spread) {
            println!("сравнение : линейная v1 — зонд {:.1}% / распространение {:.1}% (спин ускоряет диффузию)", 100.0 * lm, 100.0 * ls);
        }
        println!("время     : {:.3} с", dt.as_secs_f64());
        println!("физика    : квадратичный T-проход x[i] += c[i]·x[i+1]·x[i+2] ломает линейность GF(3)");
    }
    0
}

fn cmd_decrypt(args: &[String]) -> i32 {
    let (file, cfg) = match parse_crypto_args("decrypt", args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let Some(key_path) = &cfg.key else {
        eprintln!("pqc decrypt: требуется --key <ARCH.pqw>");
        return 2;
    };
    let Some(cipher_path) = &file else {
        eprintln!("pqc decrypt: требуется путь к шифртексту .pqt / .pqc");
        return 2;
    };
    let cipher = match std::fs::read(cipher_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pqc decrypt: ошибка чтения {cipher_path}: {e}");
            return 1;
        }
    };
    // Автодетект схемы по магии контейнера: PQT1 — триты RQ13,
    // PQC1 — f32-фазы RQ12 (legacy).
    if cipher.len() >= 4 && cipher[..4] == pqc::trite::TRITE_MAGIC {
        return cmd_decrypt_trite(key_path, &cipher, &cfg, cipher_path);
    }
    let key = match load_cipher_key(key_path, cfg.modes) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let t0 = std::time::Instant::now();
    let (msg, rep) = match pqc::decrypt(&key, &cipher) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc decrypt: {e}");
            return 1;
        }
    };
    let dt = t0.elapsed();
    match &cfg.out {
        Some(out_path) => {
            if let Err(e) = std::fs::write(out_path, &msg) {
                eprintln!("pqc decrypt: ошибка записи {out_path}: {e}");
                return 1;
            }
        }
        None => {
            // stdout: как текст (бинарный поток теряется — используйте --out).
            match String::from_utf8(msg.clone()) {
                Ok(text) => print!("{text}"),
                Err(_) => {
                    eprintln!("pqc decrypt: бинарное сообщение — запишите через --out");
                    return 1;
                }
            }
        }
    }

    if cfg.json {
        let obj = pqc::Json::Obj(vec![
            ("key".into(), pqc::Json::str(key_path)),
            ("cipher".into(), pqc::Json::str(cipher_path)),
            ("blocks".into(), pqc::Json::num(rep.blocks as f64)),
            ("msg_len".into(), pqc::Json::num(rep.msg_len as f64)),
            ("margin_min".into(), pqc::Json::num(rep.margin_min)),
            ("margin_mean".into(), pqc::Json::num(rep.margin_mean)),
            ("seconds".into(), pqc::Json::num(dt.as_secs_f64())),
        ]);
        println!("{}", obj.to_string());
    } else {
        println!("POLER Quantum Core — Decrypt: m = p* ⊕ (a ⊗_ε p*) (RQ12)");
        println!("cipher    : {cipher_path} ({} блоков)", rep.blocks);
        println!(
            "message   : {} B → {}",
            rep.msg_len,
            cfg.out.as_deref().unwrap_or("<stdout>")
        );
        println!("запас     : min {:.4} | средний {:.4} (порог ε/2 = {:.4})",
            rep.margin_min, rep.margin_mean, pqc::crypto::DECODE_THRESHOLD);
        println!("время     : {:.3} с", dt.as_secs_f64());
    }
    0
}

/// RQ13: расшифровка трит-схемы GF(3) — побитово точная (без порогов).
fn cmd_decrypt_trite(
    key_path: &str,
    cipher: &[u8],
    cfg: &CryptoConfig,
    cipher_path: &str,
) -> i32 {
    let key = match load_trite_key(key_path, cfg.modes) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let t0 = std::time::Instant::now();
    let (msg, rep) = match pqc::trite::decrypt(&key, cipher) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc decrypt: {e}");
            return 1;
        }
    };
    let dt = t0.elapsed();
    match &cfg.out {
        Some(out_path) => {
            if let Err(e) = std::fs::write(out_path, &msg) {
                eprintln!("pqc decrypt: ошибка записи {out_path}: {e}");
                return 1;
            }
        }
        None => match String::from_utf8(msg.clone()) {
            Ok(text) => print!("{text}"),
            Err(_) => {
                eprintln!("pqc decrypt: бинарное сообщение — запишите через --out");
                return 1;
            }
        },
    }

    if cfg.json {
        let obj = pqc::Json::Obj(vec![
            ("key".into(), pqc::Json::str(key_path)),
            ("cipher".into(), pqc::Json::str(cipher_path)),
            ("scheme".into(), pqc::Json::str("trite-gf3")),
            ("blocks".into(), pqc::Json::num(rep.blocks as f64)),
            ("msg_len".into(), pqc::Json::num(rep.msg_len as f64)),
            ("ticks".into(), pqc::Json::num(rep.ticks as f64)),
            ("seconds".into(), pqc::Json::num(dt.as_secs_f64())),
        ]);
        println!("{}", obj.to_string());
    } else {
        println!("POLER Quantum Core — Decrypt: m = p* ⊕ (a ⊗_ε p*) в GF(3) (RQ13)");
        println!("cipher    : {cipher_path} ({} блоков, {} тактов прецессии)", rep.blocks, rep.ticks);
        println!(
            "message   : {} B → {}",
            rep.msg_len,
            cfg.out.as_deref().unwrap_or("<stdout>")
        );
        println!("точность  : GF(3)-арифметика целая — расшифровка побитово точна");
        println!("время     : {:.3} с", dt.as_secs_f64());
    }
    0
}

fn cmd_unfurl(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("pqc unfurl: требуется путь к файлу .poler / .pqw");
        return 2;
    }
    let file_path = &args[0];
    let raw = match std::fs::read(file_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pqc unfurl: ошибка чтения {file_path}: {e}");
            return 1;
        }
    };
    let reader = match PqwReader::from_bytes(&raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc unfurl: ошибка парсинга {file_path}: {e}");
            return 1;
        }
    };

    println!("POLER Quantum Core — AOT Syntax Unfolder");
    println!(
        "source    : {} ({} B, d_pol={})",
        file_path,
        raw.len(),
        reader.d_pol()
    );

    let mut ps = vec![0.0f64; reader.d_pol() as usize];
    if reader.header().is_packed() {
        if let Ok(iter) = reader.iter_packed_trits() {
            for (_i, trit) in iter.enumerate() {
                ps[trit.0 as usize] = match trit.1 {
                    pqw::Trit::Pos => 1.0,
                    pqw::Trit::Neg => -1.0,
                    pqw::Trit::Zero => 0.0,
                };
            }
        }
    }

    let mut out_buf = [0u8; pqc::syntax_unfolder::MAX_OUT];
    let written = pqc::syntax_unfolder::unfurl(&ps, &mut out_buf);

    if written > 0 {
        let s = std::str::from_utf8(&out_buf[..written]).unwrap_or("<non-utf8 bytes>");
        println!("unfurled  : '{}' ({} bytes, zero heap alloc)", s, written);
    } else {
        println!("unfurled  : <zero / background state> (0 bytes)");
    }

    0
}

// ============================================================================
// pqc train — накопительное глубокое обучение с плотным LENS-графом (RQ8).
//
// Отличие от `pqc stream`: движок ОДИН на весь корпус, чанки (блоки по
// --block байт) льются подряд, support объединяется (merge_support),
// а снапшот сериализует НАКОПЛЕННУЮ память engine.model() — не буфер
// последнего чанка. Файл-отпечаток (raw Packed4) обновляется каждые
// --snapshot-every блоков: состояние можно мониторить на живую
// (pqc inspect / hexdump / radare2) прямо во время обучения.
// ============================================================================

/// Текстовые расширения корпуса обучения.
const TRAIN_EXTENSIONS: &[&str] = &[
    "rs", "py", "md", "txt", "json", "c", "h", "cpp", "hpp", "cc", "js", "ts", "html", "htm",
    "css", "toml", "yaml", "yml", "sh", "java", "scala", "kt", "go", "rb", "php", "sql", "tex",
];

/// Потолок размера одного файла корпуса (гигантские тома пропускаем).
const TRAIN_FILE_MAX: u64 = 8 << 20;

struct TrainConfig {
    corpus: Option<String>,
    stdin: bool,
    dim: u32,
    epsilon: f32,
    block: usize,
    shots: u64,
    steps: usize,
    seed: u64,
    eta0: f64,
    beta: f64,
    gamma: f64,
    decay: bool,
    out: Option<String>,
    fingerprint: Option<String>,
    snapshot_every: usize,
    max_bytes: u64,
    log: Option<String>,
    json: bool,
    every: usize,
    /// RQ9: поднять накопленную память из .pqw-чекпоинта перед обучением.
    resume: Option<String>,
    /// RQ9: фазовая сборка — расписание уровней с растущим чанком.
    curriculum: Option<Vec<TrainStage>>,
    /// RQ10: окно гироскопа J = A − Aᵀ (None — гироскоп выключен).
    gyro: Option<usize>,
    /// RQ10: бюджет сырых направленных пар гироскопа.
    gyro_budget: usize,
    /// RQ15: квантованный curriculum — born-шаг в тритовой решётке.
    quantized: bool,
    /// --eta0 задан явно (иначе квантованный путь берёт калибровку RQ14 η=0.6).
    eta0_explicit: bool,
    /// RQ16: шаг транспорта L5 — Δt оператора Π_Λ(e^{Δt·J} p).
    dt: f64,
}

/// Один уровень фазовой сборки (RQ9): размер чанка, шаги Born-петли,
/// выстрелы на измерение и бюджет уровня в байтах (0 = без потолка).
#[derive(Clone)]
struct TrainStage {
    block: usize,
    steps: usize,
    shots: u64,
    budget: u64,
}

/// Статистика пройденного уровня curriculum.
struct StageStats {
    level: usize,
    name: &'static str,
    block: usize,
    steps: usize,
    shots: u64,
    budget: u64,
    blocks: u64,
    tokens: u64,
    nnz_before: u64,
    nnz_after: u64,
    support_before: usize,
    support_after: usize,
    elapsed: f64,
}

impl Default for TrainConfig {
    fn default() -> Self {
        TrainConfig {
            corpus: None,
            stdin: false,
            dim: 4096,
            epsilon: 0.05,
            block: 8192,
            shots: 20_000,
            steps: 10,
            seed: 42,
            eta0: 0.25,
            beta: 1.0,
            gamma: 0.5,
            decay: false,
            out: None,
            fingerprint: None,
            snapshot_every: 64,
            max_bytes: 256 << 20,
            log: None,
            json: false,
            every: 25,
            resume: None,
            curriculum: None,
            gyro: None,
            gyro_budget: 65_536,
            quantized: false,
            eta0_explicit: false,
            dt: 0.6,
        }
    }
}

/// Расписание фазовой сборки по умолчанию: три уровня разогрева памяти
/// (микрочанки → морфемы → синтаксис) + финальный LENS-уровень из
/// --block/--steps/--shots. Бюджеты уровней — префиксы корпуса.
///
/// Калибровка (физика стоимости): Born-шаг стоит O(shots × support), а
/// микро-чанки рождают дуги почти на каждый токен — support взлетает к
/// ~90% d_pol за первые же сотни блоков. Поэтому бюджеты разогрева —
/// выборки алфавита (он повторяется!), а не весь корпус: алфавит и
/// морфемы выучиваются на малой доле данных, полную LENS-топологию
/// строит финальный уровень на всём объёме.
fn default_curriculum(cfg: &TrainConfig) -> Vec<TrainStage> {
    vec![
        TrainStage {
            block: 10,
            steps: 1,
            shots: 512,
            budget: 8 << 10,
        },
        TrainStage {
            block: 128,
            steps: 2,
            shots: 1024,
            budget: 32 << 10,
        },
        TrainStage {
            block: 1024,
            steps: 4,
            shots: 4096,
            budget: 256 << 10,
        },
        TrainStage {
            block: cfg.block,
            steps: cfg.steps,
            shots: cfg.shots,
            budget: 0,
        },
    ]
}

/// Имя уровня для баннера (физика фазовой сборки).
fn stage_name(level: usize, total: usize) -> &'static str {
    match (level, total) {
        (1, 4) => "РЕГИСТРЫ: буквы, опкоды, шум тактов",
        (2, 4) => "МОРФЕМЫ: стыковка корней и типов",
        (3, 4) => "СИНТАКСИС: правила блоков и скобок",
        (4, 4) => "LENS-ТОПОЛОГИЯ: граф связей реальности",
        _ => "УРОВЕНЬ",
    }
}

/// Парсер расписания `B1[:S1[:H1[:Z1]]],B2:...` (block:steps:shots:budget).
fn parse_curriculum(spec: &str) -> Result<Vec<TrainStage>, String> {
    let mut stages = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err("пустой уровень в --curriculum".to_string());
        }
        let fields: Vec<&str> = part.split(':').collect();
        if fields.len() > 4 {
            return Err(format!(
                "уровень '{part}': максимум 4 поля block:steps:shots:budget"
            ));
        }
        let block: usize = fields[0]
            .parse()
            .map_err(|_| format!("уровень '{part}': block не число"))?;
        if block == 0 {
            return Err(format!("уровень '{part}': block должен быть > 0"));
        }
        let steps: usize = match fields.get(1) {
            Some(s) => s
                .parse()
                .map_err(|_| format!("уровень '{part}': steps не число"))?,
            None => 1,
        };
        let shots: u64 = match fields.get(2) {
            Some(s) => s
                .parse()
                .map_err(|_| format!("уровень '{part}': shots не число"))?,
            None => 2_000,
        };
        let budget: u64 = match fields.get(3) {
            Some(s) => s
                .parse()
                .map_err(|_| format!("уровень '{part}': budget не число"))?,
            None => 0,
        };
        stages.push(TrainStage {
            block,
            steps,
            shots,
            budget,
        });
    }
    if stages.is_empty() {
        return Err("--curriculum: пустое расписание".to_string());
    }
    Ok(stages)
}

/// Счётчики цикла обучения.
struct TrainStats {
    total_tokens: u64,
    total_blocks: u64,
    no_hits: u64,
    last_loss: f64,
    last_qcm_gap: f64,
    blocks_since_snapshot: usize,
    snapshot: (usize, u64, usize, usize),
    /// RQ15: суммарные перещёлкивания тритов за прогон.
    moved_total: usize,
    /// RQ15: доля перещёлкиваний последнего блока.
    last_moved_frac: f64,
}

fn train_usage_err(msg: &str) -> i32 {
    eprintln!("pqc train: {msg}\n\n{USAGE}");
    2
}

/// Рекурсивный обход каталога: текстовые файлы ≤ TRAIN_FILE_MAX, сортировка путей.
fn collect_corpus(root: &Path, files: &mut Vec<std::path::PathBuf>, total: &mut u64, cap: u64) {
    let rd = match std::fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if *total >= cap {
            return;
        }
        if p.is_dir() {
            collect_corpus(&p, files, total, cap);
        } else if p.is_file() {
            let ext_ok = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| TRAIN_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            if !ext_ok {
                continue;
            }
            let sz = match p.metadata() {
                Ok(m) => m.len(),
                Err(_) => continue,
            };
            if sz == 0 || sz > TRAIN_FILE_MAX {
                continue;
            }
            *total += sz;
            files.push(p);
        }
    }
}

/// Диспетчер учебных движков cmd_train: плотный LENS (RQ8),
/// квантованная решётка тритов (RQ15) или слияние решётки с гироскопом
/// (RQ16: квантованный гироскоп + 2-битный момент + транспорт L5).
enum Trainer {
    Dense(pqc::stream_engine::StreamEngine),
    Quantized(pqc::qcurriculum::QuantizedCurriculum),
    QuantizedGyro(pqc::gyro_lattice::QuantizedGyroCurriculum),
}

/// Итог одного блока обучения для счётчиков цикла.
struct FeedOutcome {
    tokens: usize,
    no_hits: bool,
    param_loss: f64,
    qcm_gap: f64,
    moved: usize,
    moved_frac: f64,
}

impl Trainer {
    /// Прогон одного блока живых данных через активный движок.
    fn feed(&mut self, text: &str, steps: usize) -> pqc::Result<FeedOutcome> {
        match self {
            Trainer::Dense(e) => {
                let r = e.ingest(text, steps)?;
                Ok(FeedOutcome {
                    tokens: r.tokens,
                    no_hits: r.no_hits,
                    param_loss: r.param_loss,
                    qcm_gap: r.qcm.qcm_gap(),
                    moved: 0,
                    moved_frac: 0.0,
                })
            }
            Trainer::Quantized(q) => {
                let r = q.ingest(text, steps)?;
                Ok(FeedOutcome {
                    tokens: r.tokens,
                    no_hits: r.no_hits,
                    param_loss: r.param_loss,
                    qcm_gap: 0.0,
                    moved: r.moved,
                    moved_frac: r.moved_frac,
                })
            }
            // RQ16: steps = шаги авторегрессионного рассуждения (транспорт
            // L5 по руслам J) после born-шага.
            Trainer::QuantizedGyro(q) => {
                let r = q.ingest(text, steps)?;
                Ok(FeedOutcome {
                    tokens: r.tokens,
                    no_hits: r.no_hits,
                    param_loss: r.param_loss,
                    qcm_gap: 0.0,
                    moved: r.moved,
                    moved_frac: r.moved_frac,
                })
            }
        }
    }

    /// Бюджет выстрелов уровня curriculum.
    fn set_shots(&mut self, shots: u64) {
        match self {
            Trainer::Dense(e) => e.set_shots(shots),
            Trainer::Quantized(q) => {
                q.set_shots(shots);
            }
            Trainer::QuantizedGyro(q) => {
                q.set_shots(shots);
            }
        }
    }

    /// Поднять накопленную память из чекпоинта.
    fn resume(&mut self, reader: &PqwReader) -> pqc::Result<usize> {
        match self {
            Trainer::Dense(e) => e.resume_from_reader(reader),
            Trainer::Quantized(q) => q.resume_from_reader(reader),
            Trainer::QuantizedGyro(q) => q.resume_from_reader(reader),
        }
    }

    /// Слов в лексиконе кристалла (RQ17): None — движок без лексикона
    /// (плотный/квантованный пути RQ8/RQ15).
    fn lexicon_len(&self) -> Option<usize> {
        match self {
            Trainer::Dense(_) | Trainer::Quantized(_) => None,
            Trainer::QuantizedGyro(q) => Some(q.lexicon_len()),
        }
    }
}

/// Снапшот НАКОПЛЕННОЙ памяти: .pqw-контейнер + raw Packed4-отпечаток.
///
/// RQ10: при включённом гироскопе и живой циркуляции пишется контейнер
/// v3 (POLER_Q3): фазы Packed4 + топологическая секция J = A − Aᵀ.
/// RQ15: квантованный путь выписывает решётку бит-в-бит — нулевая
/// переквантование, чекпоинт = сама память.
/// Отпечаток --fingerprint — только фазовые блоки (сравнимость мониторинга).
///
/// Возвращает (байты контейнера, nnz фаз, support, пар гироскопа).
fn train_snapshot(
    trainer: &Trainer,
    cfg: &TrainConfig,
) -> Result<(usize, u64, usize, usize), String> {
    let mut gyro_pairs = 0usize;
    // Контейнер: плотный путь кодирует модель (v2/v3), квантованный —
    // выписывает решётку бит-в-бит (RQ15).
    let buf = match trainer {
        Trainer::Dense(engine) => {
            let model = engine.model();
            let mut buf = Vec::new();
            let rho = if cfg.decay { 0.0 } else { 1.0 };
            let mut w = PqwWriter::new(engine.d_pol())
                .map_err(|e| e.to_string())?
                .hyperparams(cfg.eta0 as f32, cfg.gamma as f32, rho, cfg.epsilon);
            let state_f32: Vec<f32> = model.iter().map(|&p| p as f32).collect();
            w.add_state(&state_f32).map_err(|e| e.to_string())?;
            // v3 с гироскопом (если циркуляция пережила ε-ворота), иначе v2.
            let gyro_data = engine
                .gyro()
                .and_then(|g| g.gyro_data(cfg.epsilon as f64, engine.d_pol()));
            match &gyro_data {
                Some(data) => {
                    gyro_pairs = data.pairs().len();
                    w.write_v3(&mut buf, data).map_err(|e| e.to_string())?;
                }
                None => {
                    w.write_packed_trits(&mut buf).map_err(|e| e.to_string())?;
                }
            }
            buf
        }
        Trainer::Quantized(q) => q.checkpoint().map_err(|e| e.to_string())?,
        // RQ16: чекпоинт v3 — фазы бит-в-бит + секция GYRO из русел J.
        Trainer::QuantizedGyro(q) => {
            gyro_pairs = q.channel_count();
            q.checkpoint().map_err(|e| e.to_string())?
        }
    };
    let phase_len = (cfg.dim as usize).div_ceil(4);
    let reader = PqwReader::from_bytes(&buf).map_err(|e| e.to_string())?;
    let nnz = reader.nnz();
    let support = match trainer {
        Trainer::Dense(engine) => engine.model_arcs().len(),
        // Решётка: union support ≡ ненулевые триты (память = контейнер).
        Trainer::Quantized(q) => q.nnz(),
        Trainer::QuantizedGyro(q) => q.nnz(),
    };
    if let Some(p) = &cfg.out {
        std::fs::write(p, &buf).map_err(|e| format!("--out: {e}"))?;
    }
    if let Some(p) = &cfg.fingerprint {
        // Только фазовые блоки: размер и семантика отпечатка неизменны
        // при любом формате контейнера (v2/v3).
        let end = (pqw::HEADER_SIZE + phase_len).min(buf.len());
        let payload = &buf[pqw::HEADER_SIZE.min(buf.len())..end];
        std::fs::write(p, payload).map_err(|e| format!("--fingerprint: {e}"))?;
    }
    Ok((buf.len(), nnz, support, gyro_pairs))
}

/// RQ14: mmap-разворот тритов в углы Блоха — θ = arccos(p) на лету.
///
/// Стриминг Packed4 → углы: LUT из трёх констант (θ ∈ {0, π/2, π}),
/// полный вектор углов не материализуется.
fn cmd_bloch(args: &[String]) -> i32 {
    let mut file: Option<String> = None;
    let mut json = false;
    let mut head: usize = 8; // превью первых углов
    let mut window: usize = 4096; // окно стримера
    let mut i = 0usize;
    macro_rules! val {
        ($name:literal) => {{
            i += 1;
            if i >= args.len() {
                eprintln!("pqc bloch: {} требует значение\n\n{USAGE}", $name);
                return 2;
            }
            args[i].clone()
        }};
    }
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json = true,
            "--head" => {
                let v = val!("--head");
                match v.parse::<usize>() {
                    Ok(n) => head = n,
                    Err(_) => {
                        eprintln!("pqc bloch: bad --head: {v}\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            "--window" => {
                let v = val!("--window");
                match v.parse::<usize>() {
                    Ok(n) if n > 0 => window = n,
                    _ => {
                        eprintln!("pqc bloch: bad --window: {v} (> 0)\n\n{USAGE}");
                        return 2;
                    }
                }
            }
            other => {
                if other.starts_with("--") {
                    eprintln!("pqc bloch: неизвестный флаг {other}\n\n{USAGE}");
                    return 2;
                }
                if file.is_some() {
                    eprintln!("pqc bloch: один файл за вызов\n\n{USAGE}");
                    return 2;
                }
                file = Some(other.to_string());
            }
        }
        i += 1;
    }
    let Some(path) = file else {
        eprintln!("pqc bloch: нужен контейнер v2 (.pqw)\n\n{USAGE}");
        return 2;
    };
    let src = match load(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pqc bloch: {path}: {e}");
            return 1;
        }
    };
    let reader = match PqwReader::from_bytes(src.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc bloch: {path}: {e}");
            return 1;
        }
    };
    let counts = match reader.bloch_counts() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pqc bloch: контейнер не Packed4 (v2): {e}");
            return 1;
        }
    };
    let d = reader.d_pol() as usize;

    // Полный стриминговый проход по углам (микробенчмарк на лету).
    let t0 = std::time::Instant::now();
    let mut stream = bloch_stream::Packed4Angles::new(reader.phase_bytes(), d, window);
    let mut theta_sum = 0.0_f64;
    while let Some((_, w)) = stream.next_chunk() {
        for &t in w {
            theta_sum += t;
        }
    }
    let elapsed = t0.elapsed();
    let per_trit_ns = elapsed.as_nanos() as f64 / d.max(1) as f64;

    let preview: Vec<(u32, f64)> = reader
        .bloch_angles()
        .unwrap()
        .take(head)
        .collect::<Vec<_>>();

    if json {
        let report = Json::Obj(vec![
            (
                "cmd".into(),
                Json::Str("bloch".into()),
            ),
            ("file".into(), Json::Str(path.clone())),
            ("d_pol".into(), Json::Num(d as f64)),
            (
                "encoding".into(),
                Json::Str(reader.encoding().name().into()),
            ),
            ("pos".into(), Json::Num(counts.pos as f64)),
            ("neg".into(), Json::Num(counts.neg as f64)),
            ("zero".into(), Json::Num(counts.zero as f64)),
            ("density".into(), Json::Num(counts.density())),
            ("balance".into(), Json::Num(counts.balance())),
            (
                "theta_head".into(),
                Json::Arr(
                    preview
                        .iter()
                        .map(|(i, t)| Json::Arr(vec![Json::Num(*i as f64), Json::Num(*t)]))
                        .collect(),
                ),
            ),
            (
                "theta_mean".into(),
                Json::Num(theta_sum / d.max(1) as f64),
            ),
            (
                "stream_ns_per_trit".into(),
                Json::Num(per_trit_ns),
            ),
            (
                "window".into(),
                Json::Num(window as f64),
            ),
        ]);
        println!("{}", report.to_string());
        return 0;
    }

    println!("RQ14: разворот тритов в углы Блоха — mmap, θ = arccos(p) на лету");
    println!("  файл      : {path} ({})", src.kind());
    println!("  d_pol     : {d} тритов, кодировка {}", reader.encoding().name());
    println!(
        "  решётка   : +1 × {}, −1 × {}, 0 × {}  (плотность LENS {:.4}, баланс {:.4})",
        counts.pos,
        counts.neg,
        counts.zero,
        counts.density(),
        counts.balance()
    );
    println!(
        "  стриминг  : окно {window}, {:.2} нс/трит (полный проход, без материализации)",
        per_trit_ns
    );
    println!("  средний θ : {:.6} рад", theta_sum / d.max(1) as f64);
    if !preview.is_empty() {
        let s = preview
            .iter()
            .map(|(i, t)| format!("#{i}: {:.4}", t))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  первые углы: {s}");
    }
    0
}

fn cmd_train(args: &[String]) -> i32 {
    let mut cfg = TrainConfig::default();
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        let mut val = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match a.as_str() {
            "--corpus" => match val("--corpus") {
                Ok(v) => cfg.corpus = Some(v),
                Err(e) => return train_usage_err(&e),
            },
            "--stdin" => cfg.stdin = true,
            "--dim" => match val("--dim").and_then(|v| parse_num::<u32>(&v, "--dim")) {
                Ok(v) => cfg.dim = v,
                Err(e) => return train_usage_err(&e),
            },
            "--epsilon" => match val("--epsilon")
                .and_then(|v| v.parse::<f32>().map_err(|_| "bad --epsilon".to_string()))
            {
                Ok(v) => cfg.epsilon = v,
                Err(_) => return train_usage_err("bad --epsilon"),
            },
            "--block" => match val("--block").and_then(|v| parse_num::<usize>(&v, "--block")) {
                Ok(v) => cfg.block = v,
                Err(e) => return train_usage_err(&e),
            },
            "--shots" => match val("--shots").and_then(|v| parse_num::<u64>(&v, "--shots")) {
                Ok(v) => cfg.shots = v,
                Err(e) => return train_usage_err(&e),
            },
            "--steps" => match val("--steps").and_then(|v| parse_num::<usize>(&v, "--steps")) {
                Ok(v) => cfg.steps = v,
                Err(e) => return train_usage_err(&e),
            },
            "--seed" => match val("--seed").and_then(|v| parse_num::<u64>(&v, "--seed")) {
                Ok(v) => cfg.seed = v,
                Err(e) => return train_usage_err(&e),
            },
            "--eta0" => match val("--eta0")
                .and_then(|v| v.parse::<f64>().map_err(|_| "bad --eta0".to_string()))
            {
                Ok(v) => {
                    cfg.eta0 = v;
                    cfg.eta0_explicit = true;
                }
                Err(_) => return train_usage_err("bad --eta0"),
            },
            "--beta" => match val("--beta")
                .and_then(|v| v.parse::<f64>().map_err(|_| "bad --beta".to_string()))
            {
                Ok(v) => cfg.beta = v,
                Err(_) => return train_usage_err("bad --beta"),
            },
            "--gamma" => match val("--gamma")
                .and_then(|v| v.parse::<f64>().map_err(|_| "bad --gamma".to_string()))
            {
                Ok(v) => cfg.gamma = v,
                Err(_) => return train_usage_err("bad --gamma"),
            },
            "--decay" => cfg.decay = true,
            "--quantized" => cfg.quantized = true,
            "--dt" => match val("--dt").and_then(|v| parse_num::<f64>(&v, "--dt")) {
                Ok(v) if v.is_finite() && v >= 0.0 => cfg.dt = v,
                _ => return train_usage_err("--dt должен быть конечным ≥ 0"),
            },
            "--out" => match val("--out") {
                Ok(v) => cfg.out = Some(v),
                Err(e) => return train_usage_err(&e),
            },
            "--fingerprint" => match val("--fingerprint") {
                Ok(v) => cfg.fingerprint = Some(v),
                Err(e) => return train_usage_err(&e),
            },
            "--snapshot-every" => {
                match val("--snapshot-every")
                    .and_then(|v| parse_num::<usize>(&v, "--snapshot-every"))
                {
                    Ok(v) => cfg.snapshot_every = v.max(1),
                    Err(e) => return train_usage_err(&e),
                }
            }
            "--max-bytes" => {
                match val("--max-bytes").and_then(|v| parse_num::<u64>(&v, "--max-bytes")) {
                    Ok(v) => cfg.max_bytes = v,
                    Err(e) => return train_usage_err(&e),
                }
            }
            "--log" => match val("--log") {
                Ok(v) => cfg.log = Some(v),
                Err(e) => return train_usage_err(&e),
            },
            "--every" => match val("--every").and_then(|v| parse_num::<usize>(&v, "--every")) {
                Ok(v) => cfg.every = v.max(1),
                Err(e) => return train_usage_err(&e),
            },
            "--json" => cfg.json = true,
            "--resume" => match val("--resume") {
                Ok(v) => cfg.resume = Some(v),
                Err(e) => return train_usage_err(&e),
            },
            "--curriculum" => {
                // Значение опционально: следующий аргумент без "--" — расписание,
                // иначе расписание по умолчанию (пустой вектор — маркер,
                // разворачивается после парсинга, когда известны --block/--steps).
                let next = args.get(i + 1).map(|s| s.as_str());
                match next {
                    Some(s) if !s.starts_with("--") => {
                        i += 1;
                        match parse_curriculum(s) {
                            Ok(st) => cfg.curriculum = Some(st),
                            Err(e) => return train_usage_err(&e),
                        }
                    }
                    _ => cfg.curriculum = Some(Vec::new()),
                }
            }
            "--gyro" => {
                // RQ10: окно направленного контекста (default 256);
                // значение опционально — как у --curriculum.
                let next = args.get(i + 1).map(|s| s.as_str());
                match next {
                    Some(s) if !s.starts_with("--") => match s.parse::<usize>() {
                        Ok(w) if w > 0 => {
                            i += 1;
                            cfg.gyro = Some(w);
                        }
                        _ => {
                            return train_usage_err("--gyro: окно должно быть > 0")
                        }
                    },
                    _ => cfg.gyro = Some(256),
                }
            }
            "--gyro-budget" => {
                match val("--gyro-budget")
                    .and_then(|v| parse_num::<usize>(&v, "--gyro-budget"))
                {
                    Ok(v) if v >= 16 => cfg.gyro_budget = v,
                    _ => return train_usage_err("--gyro-budget должен быть ≥ 16"),
                }
            }
            other => return train_usage_err(&format!("unknown option {other}")),
        }
        i += 1;
    }

    if cfg.corpus.is_none() && !cfg.stdin {
        return train_usage_err("укажите --corpus DIR или --stdin");
    }
    if cfg.dim == 0 || cfg.dim > 1 << 20 {
        return train_usage_err("--dim должен быть в [1, 1048576]");
    }
    if !(0.0..=1.0).contains(&cfg.epsilon) || !cfg.epsilon.is_finite() {
        return train_usage_err("--epsilon должен быть в [0, 1]");
    }
    if cfg.block == 0 {
        return train_usage_err("--block должен быть > 0");
    }
    // RQ15/RQ16: честные ограничения квантованных путей.
    if cfg.quantized && cfg.decay {
        return train_usage_err(
            "--quantized несовместим с --decay (политика Hold — фон заморожен)",
        );
    }
    // RQ16 РАЗБЛОКИРУЕТ --quantized --gyro: слияние решётки с гироскопом
    // (J в тритовой решётке пар + 2-битный момент + транспорт L5).
    if cfg.quantized
        && cfg.gyro.is_some()
        && cfg.dim > pqc::gyro_lattice::MAX_DIM_GYRO
    {
        return train_usage_err(&format!(
            "--quantized --gyro: решётка пар O(d²) — d_pol ≤ {} (задано {})",
            pqc::gyro_lattice::MAX_DIM_GYRO,
            cfg.dim
        ));
    }
    if cfg.quantized && cfg.gyro_budget != 65_536 {
        return train_usage_err(
            "--gyro-budget несовместим с --quantized: плотная решётка пар \
             не бюджетируется (RQ16: индексация пар без HashMap)",
        );
    }
    if cfg.dt != 0.6 && !(cfg.quantized && cfg.gyro.is_some()) {
        return train_usage_err("--dt требует --quantized --gyro (транспорт L5)");
    }

    // Расписание фазовой сборки: маркер по умолчанию разворачивается здесь,
    // когда --block/--steps/--shots уже известны. Без --curriculum —
    // одиночный проход классическими параметрами.
    if let Some(stages) = &cfg.curriculum {
        if stages.is_empty() {
            cfg.curriculum = Some(default_curriculum(&cfg));
        }
    }
    let stages: Vec<TrainStage> = cfg.curriculum.clone().unwrap_or_else(|| {
        vec![TrainStage {
            block: cfg.block,
            steps: cfg.steps,
            shots: cfg.shots,
            budget: 0,
        }]
    });

    // Корпус: каталог (рекурсивно) или stdin.
    let mut log_file = cfg.log.as_ref().and_then(|p| {
        std::fs::File::create(p)
            .map(|mut f| {
                use std::io::Write;
                let title = if cfg.quantized && cfg.gyro.is_some() {
                    "КВАНТОВАННЫЙ ГИРОСКОП-СЛИЯНИЕ (RQ16): J в тритах + момент 2 бит + транспорт L5"
                } else if cfg.quantized {
                    "КВАНТОВАННЫЙ CURRICULUM (RQ15): born-шаг в решётке тритов"
                } else {
                    "ПЛОТНОЕ LENS-ОБУЧЕНИЕ"
                };
                let _ = writeln!(
                    f,
                    "=== {title} (eps={}, block={} B, d_pol={}) ===",
                    cfg.epsilon, cfg.block, cfg.dim
                );
                f
            })
            .ok()
    });

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut corpus_bytes: u64 = 0;
    if let Some(root) = &cfg.corpus {
        let mut total = 0u64;
        collect_corpus(Path::new(root), &mut files, &mut total, cfg.max_bytes);
        corpus_bytes = total;
        if files.is_empty() {
            eprintln!("pqc train: в {root} не найдено текстовых файлов");
            return 1;
        }
    }

    // Движок ОДИН на весь корпус: память накапливается между блоками.
    // RQ15: --quantized — born-шаг прямо в 2-битной решётке (η = --eta0
    // напрямую; β не используется — расписание сюрприза остаётся у плотного).
    // RQ16: --quantized --gyro — слияние: J в тритовой решётке пар,
    // 2-битный насыщающий момент, транспорт L5; --steps = шаги рассуждения.
    use pqc::stream_engine::{Forget, StreamEngine};
    let mut trainer = if cfg.quantized && cfg.gyro.is_some() {
        let window = cfg.gyro.unwrap();
        match pqc::gyro_lattice::QuantizedGyroCurriculum::new(cfg.dim, cfg.epsilon, cfg.seed, window)
        {
            Ok(mut q) => {
                let eta = if cfg.eta0_explicit { cfg.eta0 } else { 0.6 };
                q.set_shots(cfg.shots).set_eta(eta).set_dt(cfg.dt);
                Trainer::QuantizedGyro(q)
            }
            Err(e) => {
                eprintln!("pqc train: {e}");
                return 1;
            }
        }
    } else if cfg.quantized {
        match pqc::qcurriculum::QuantizedCurriculum::new(cfg.dim, cfg.epsilon, cfg.seed) {
            Ok(mut q) => {
                // Калибровка: плотный дефолт η₀ = 0.25 НЕ пересекает порог
                // кристаллизации решётки (η·v* = 0.5 < π/6) — квантованный
                // путь берёт η = 0.6 из RQ14, если --eta0 не задан явно.
                let eta = if cfg.eta0_explicit {
                    cfg.eta0
                } else {
                    0.6
                };
                q.set_shots(cfg.shots).set_hyper(eta, cfg.gamma);
                Trainer::Quantized(q)
            }
            Err(e) => {
                eprintln!("pqc train: {e}");
                return 1;
            }
        }
    } else {
        let forget = if cfg.decay {
            Forget::Decay
        } else {
            Forget::Hold
        };
        match StreamEngine::new(cfg.dim, cfg.epsilon, cfg.seed) {
            Ok(e) => {
                // RQ10: гироскоп J = A − Aᵀ — только плотный путь
                // (валидация выше уже отклонила --quantized --gyro).
                let e = if let Some(window) = cfg.gyro {
                    e.with_gyro(window, cfg.gyro_budget)
                } else {
                    e
                };
                Trainer::Dense(
                    e.with_shots(cfg.shots)
                        .with_hyper(cfg.eta0, cfg.beta, cfg.gamma)
                        .with_forget(forget),
                )
            }
            Err(e) => {
                eprintln!("pqc train: {e}");
                return 1;
            }
        }
    };

    // RQ9: resume — поднять накопленную память из .pqw-чекпоинта.
    let mut resumed_nnz: Option<usize> = None;
    if let Some(path) = &cfg.resume {
        let raw = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("pqc train: --resume: ошибка чтения {path}: {e}");
                return 1;
            }
        };
        let reader = match pqw::PqwReader::from_bytes(&raw) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("pqc train: --resume: ошибка парсинга {path}: {e}");
                return 1;
            }
        };
        if reader.d_pol() != cfg.dim {
            eprintln!(
                "pqc train: --resume: d_pol контейнера {} ≠ --dim {}",
                reader.d_pol(),
                cfg.dim
            );
            return 1;
        }
        match trainer.resume(&reader) {
            Ok(n) => resumed_nnz = Some(n),
            Err(e) => {
                eprintln!("pqc train: --resume: {e}");
                return 1;
            }
        }
    }

    if !cfg.json {
        let src = if let Some(root) = &cfg.corpus {
            format!(
                "{root} ({} файлов, {:.1} МиБ)",
                files.len(),
                corpus_bytes as f64 / 1048576.0
            )
        } else {
            "stdin".to_string()
        };
        let fused = cfg.quantized && cfg.gyro.is_some();
        println!("POLER Quantum Core — {}", if fused {
            "Quantized Gyro Trainer (RQ16): слияние решётки с гироскопом J = A − Aᵀ, транспорт L5"
        } else if cfg.quantized {
            "Quantized Born Trainer (RQ15): решётка тритов, born-шаг в 2-битных регистрах"
        } else {
            "Dense LENS Trainer (RQ8/RQ9)"
        });
        println!("corpus    : {src}");
        if fused {
            let pair_bytes =
                (cfg.dim as usize) * (cfg.dim as usize - 1) / 2 / 2;
            println!(
                "lattice   : d_pol={} → фазы {} Б + момент {} Б + русла J {} Б — ноль HashMap",
                cfg.dim,
                (cfg.dim as usize).div_ceil(4),
                (cfg.dim as usize).div_ceil(4),
                pair_bytes
            );
            println!(
                "physics   : η={}, Δt={}, момент 2 бит (τ_ign=0.5, τ_stall=1.0, амплитуда 2.0), инерция без затухания{}",
                if cfg.eta0_explicit { cfg.eta0 } else { 0.6 },
                cfg.dt,
                if cfg.eta0_explicit { "" } else { " (η — калибровка RQ14)" }
            );
        } else if cfg.quantized {
            println!(
                "lattice   : d_pol={} тритов × 2 бита = {} Б Packed4 (память = контейнер v2)",
                cfg.dim,
                (cfg.dim as usize).div_ceil(4)
            );
            println!(
                "physics   : η={}, γ={}, гистерезис π/3, полюса детерминированы{}",
                if cfg.eta0_explicit { cfg.eta0 } else { 0.6 },
                cfg.gamma,
                if cfg.eta0_explicit { "" } else { " (η — калибровка RQ14)" }
            );
        }
        if stages.len() > 1 {
            println!(
                "curriculum: {} уровней фазовой сборки — чанк растёт, память одна",
                stages.len()
            );
        } else {
            println!(
                "engine    : d_pol={}, eps={}, block={} B, shots={}, steps={}, seed={}",
                cfg.dim, cfg.epsilon, cfg.block, cfg.shots, cfg.steps, cfg.seed
            );
        }
        if let Some(n) = resumed_nnz {
            println!(
                "resumed   : {} ({} дуг поднято из чекпоинта)",
                cfg.resume.as_deref().unwrap_or("?"),
                n
            );
        }
        if let Some(w) = cfg.gyro {
            if fused {
                println!(
                    "gyro      : J = A − Aᵀ в тритах русел, окно {w} токенов, порог насыщения 2 события"
                );
            } else {
                println!(
                    "gyro      : J = A − Aᵀ, окно {w} токенов, бюджет {} пар — топологическая секция v3",
                    cfg.gyro_budget
                );
            }
        }
        if !cfg.quantized {
            println!(
                "policy    : forget={:?}, snapshot every {} блоков",
                if cfg.decay { Forget::Decay } else { Forget::Hold },
                cfg.snapshot_every
            );
        } else if fused {
            println!(
                "reasoning : --steps {} = шаги Π_Λ(e^{{Δt·J}}p) после born-шага, snapshot every {} блоков",
                cfg.steps, cfg.snapshot_every
            );
        } else {
            println!("policy    : Hold (фон заморожен), snapshot every {} блоков", cfg.snapshot_every);
        }
    }

    let t0 = std::time::Instant::now();
    // Счётчики цикла обучения (владеет замыкание ниже).
    let mut st = TrainStats {
        total_tokens: 0,
        total_blocks: 0,
        no_hits: 0,
        last_loss: 0.0,
        last_qcm_gap: 0.0,
        blocks_since_snapshot: 0,
        snapshot: (0usize, 0u64, 0usize, 0usize),
        moved_total: 0,
        last_moved_frac: 0.0,
    };

    // Начальный снапшот: нулевой (или поднятый из чекпоинта) отпечаток.
    match train_snapshot(&trainer, &cfg) {
        Ok(s) => st.snapshot = s,
        Err(e) => {
            eprintln!("pqc train: {e}");
            return 1;
        }
    }

    // stdin читается ОДИН раз до цикла уровней: уровни берут префиксы буфера.
    let stdin_buf: Vec<u8> = if cfg.stdin {
        use std::io::Read;
        let mut buf = Vec::new();
        if let Err(e) = std::io::stdin().read_to_end(&mut buf) {
            eprintln!("pqc train: stdin: {e}");
            return 1;
        }
        buf
    } else {
        Vec::new()
    };

    let feed_block = |data: &[u8],
                      trainer: &mut Trainer,
                      st: &mut TrainStats,
                      steps: usize|
     -> Result<(), String> {
        let text = String::from_utf8_lossy(data);
        let rep = trainer.feed(&text, steps).map_err(|e| e.to_string())?;
        st.total_tokens += rep.tokens as u64;
        st.total_blocks += 1;
        if rep.no_hits {
            st.no_hits += 1;
        } else {
            st.last_loss = rep.param_loss;
            st.last_qcm_gap = rep.qcm_gap;
            st.moved_total += rep.moved;
            st.last_moved_frac = rep.moved_frac;
        }
        st.blocks_since_snapshot += 1;
        if st.blocks_since_snapshot >= cfg.snapshot_every {
            st.snapshot = train_snapshot(trainer, &cfg)?;
            st.blocks_since_snapshot = 0;
        }
        Ok(())
    };

    // Статистика уровней фазовой сборки (RQ9).
    let mut stage_stats: Vec<StageStats> = Vec::new();

    for (li, stage) in stages.iter().enumerate() {
        let level = li + 1;
        // Пересборка ученика под бюджет выстрелов уровня (детерминизм).
        trainer.set_shots(stage.shots);

        // Снапшот на входе в уровень: отпечаток фиксирует границу перехода.
        match train_snapshot(&trainer, &cfg) {
            Ok(s) => st.snapshot = s,
            Err(e) => {
                eprintln!("pqc train: {e}");
                return 1;
            }
        }
        let (nnz_before, support_before) = (st.snapshot.1, st.snapshot.2);
        let (blocks_before, tokens_before) = (st.total_blocks, st.total_tokens);
        let t_stage = std::time::Instant::now();

        if !cfg.json && stages.len() > 1 {
            let budget = if stage.budget == 0 {
                "весь корпус".to_string()
            } else {
                format!("{:.0} КиБ", stage.budget as f64 / 1024.0)
            };
            println!(
                "── УРОВЕНЬ {}/{} «{}»: чанк {} B × steps={} × shots={}, бюджет {} ──",
                level,
                stages.len(),
                stage_name(level, stages.len()),
                stage.block,
                stage.steps,
                stage.shots,
                budget
            );
        }

        if cfg.stdin {
            let input: &[u8] = if stage.budget > 0 {
                &stdin_buf[..stdin_buf.len().min(stage.budget as usize)]
            } else {
                &stdin_buf
            };
            for chunk in input.chunks(stage.block) {
                if let Err(e) = feed_block(chunk, &mut trainer, &mut st, stage.steps) {
                    eprintln!("pqc train: {e}");
                    return 1;
                }
            }
        } else {
            // Префикс корпуса под бюджет уровня (0 = весь корпус).
            // Бюджет обрезает ПОТОК БАЙТ, а не список файлов: один крупный
            // том не должен раздувать уровень (файл может быть больше
            // бюджета — тогда уровень видит лишь его префикс).
            let root = cfg.corpus.as_deref().unwrap_or(".");
            let stage_files: Vec<std::path::PathBuf> = if stage.budget > 0 {
                let mut f = Vec::new();
                let mut t = 0u64;
                collect_corpus(Path::new(root), &mut f, &mut t, stage.budget);
                f
            } else {
                files.clone()
            };
            let mut fed: u64 = 0;
            'files: for (fi, path) in stage_files.iter().enumerate() {
                let data = match std::fs::read(path) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let take: usize = if stage.budget > 0 {
                    let remaining = (stage.budget.saturating_sub(fed)) as usize;
                    if remaining == 0 {
                        break 'files;
                    }
                    data.len().min(remaining)
                } else {
                    data.len()
                };
                fed += take as u64;
                let tokens_before_file = st.total_tokens;
                let mut file_blocks: u64 = 0;
                for chunk in data[..take].chunks(stage.block) {
                    if let Err(e) = feed_block(chunk, &mut trainer, &mut st, stage.steps) {
                        eprintln!("pqc train: {e}");
                        return 1;
                    }
                    file_blocks += 1;
                }
                let file_tokens = st.total_tokens - tokens_before_file;
                if let Some(f) = log_file.as_mut() {
                    use std::io::Write;
                    let _ = writeln!(
                        f,
                        "[L{}/{} {}/{}] {:<36} | tokens={:<6} blocks={:<3} | loss={:.6} nnz_lens={} support={}",
                        level,
                        stages.len(),
                        fi + 1,
                        stage_files.len(),
                        path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        file_tokens,
                        file_blocks,
                        st.last_loss,
                        st.snapshot.1,
                        st.snapshot.2
                    );
                }
                if !cfg.json && (fi + 1) % cfg.every == 0 {
                    let el = t_stage.elapsed().as_secs_f64();
                    println!(
                        "  [{:>4}/{}] {:<5.1}s | tokens={:<8} nnz_lens={:<5} support={:<5} loss={:.6}",
                        fi + 1,
                        stage_files.len(),
                        el,
                        st.total_tokens,
                        st.snapshot.1,
                        st.snapshot.2,
                        st.last_loss
                    );
                }
                if stage.budget > 0 && fed >= stage.budget {
                    break 'files;
                }
            }
        }

        // Снапшот на выходе уровня: отпечаток = завершённый уровень.
        match train_snapshot(&trainer, &cfg) {
            Ok(s) => st.snapshot = s,
            Err(e) => {
                eprintln!("pqc train: {e}");
                return 1;
            }
        }
        st.blocks_since_snapshot = 0;
        let stats = StageStats {
            level,
            name: stage_name(level, stages.len()),
            block: stage.block,
            steps: stage.steps,
            shots: stage.shots,
            budget: stage.budget,
            blocks: st.total_blocks - blocks_before,
            tokens: st.total_tokens - tokens_before,
            nnz_before,
            nnz_after: st.snapshot.1,
            support_before,
            support_after: st.snapshot.2,
            elapsed: t_stage.elapsed().as_secs_f64(),
        };
        if !cfg.json && stages.len() > 1 {
            println!(
                "  L{}: blocks={} tokens={} nnz {}→{} support {}→{} ({:.1}s)",
                stats.level,
                stats.blocks,
                stats.tokens,
                stats.nnz_before,
                stats.nnz_after,
                stats.support_before,
                stats.support_after,
                stats.elapsed
            );
        }
        stage_stats.push(stats);
    }

    // Финальный снапшот (гарантированно свежий).
    match train_snapshot(&trainer, &cfg) {
        Ok(s) => st.snapshot = s,
        Err(e) => {
            eprintln!("pqc train: {e}");
            return 1;
        }
    }
    let elapsed = t0.elapsed().as_secs_f64();
    let (container_bytes, nnz_lens, support, gyro_pairs) = st.snapshot;
    let density = nnz_lens as f64 / cfg.dim as f64 * 100.0;
    let total_tokens = st.total_tokens;
    let total_blocks = st.total_blocks;
    let no_hits = st.no_hits;
    let last_loss = st.last_loss;
    let last_qcm_gap = st.last_qcm_gap;

    if let Some(f) = log_file.as_mut() {
        use std::io::Write;
        let _ = writeln!(
            f,
            "=== ИТОГ: blocks={} tokens={} nnz_lens={} support={} density={:.2}% moved={} elapsed={:.1}s ===",
            total_blocks, total_tokens, nnz_lens, support, density, st.moved_total, elapsed
        );
    }

    if cfg.json {
        use pqc::json::Json;
        let mut pairs: Vec<(String, Json)> = vec![
            ("command".into(), Json::str("train")),
            (
                "mode".into(),
                Json::str(if cfg.quantized && cfg.gyro.is_some() {
                    "quantized_gyro"
                } else if cfg.quantized {
                    "quantized"
                } else {
                    "dense"
                }),
            ),
            ("d_pol".into(), Json::num(cfg.dim as f64)),
            ("epsilon".into(), Json::num(cfg.epsilon as f64)),
            ("block".into(), Json::num(cfg.block as f64)),
            ("blocks".into(), Json::num(total_blocks as f64)),
            ("tokens".into(), Json::num(total_tokens as f64)),
            ("files".into(), Json::num(files.len() as f64)),
            ("no_hits".into(), Json::num(no_hits as f64)),
            ("nnz_lens".into(), Json::num(nnz_lens as f64)),
            ("support".into(), Json::num(support as f64)),
            (
                "density_pct".into(),
                Json::num((density * 100.0).round() / 100.0),
            ),
            (
                "final_loss".into(),
                Json::num((last_loss * 1e8).round() / 1e8),
            ),
            (
                "qcm_gap".into(),
                Json::num((last_qcm_gap * 1e8).round() / 1e8),
            ),
            ("container_bytes".into(), Json::num(container_bytes as f64)),
            (
                "elapsed_sec".into(),
                Json::num((elapsed * 100.0).round() / 100.0),
            ),
        ];
        // RQ15/RQ16: телеметрия квантованной решётки — «обучение произошло»
        // в числах: перещёлкивания тритов и живой момент.
        if cfg.quantized {
            pairs.push(("moved_total".into(), Json::num(st.moved_total as f64)));
            pairs.push((
                "moved_frac".into(),
                Json::num((st.last_moved_frac * 1e6).round() / 1e6),
            ));
            pairs.push((
                "lattice_bytes".into(),
                Json::num((cfg.dim as usize).div_ceil(4) as f64),
            ));
        }
        // RQ16: слияние решётки с гироскопом — русла J в тритах, момент,
        // квантованные шаги авторегрессионного рассуждения (транспорт L5).
        if let (Some(window), Trainer::QuantizedGyro(q)) = (cfg.gyro, &trainer) {
            let mem = q.memory();
            let section_bytes = container_bytes
                .saturating_sub(pqw::HEADER_SIZE + (cfg.dim as usize).div_ceil(4));
            pairs.push((
                "gyro".into(),
                Json::Obj(vec![
                    ("window".into(), Json::num(window as f64)),
                    ("ticks".into(), Json::num(q.gyro_ticks() as f64)),
                    ("channels".into(), Json::num(q.channel_count() as f64)),
                    ("pair_bytes".into(), Json::num(q.pair_bytes() as f64)),
                    ("section_bytes".into(), Json::num(section_bytes as f64)),
                ]),
            ));
            pairs.push((
                "momentum".into(),
                Json::Obj(vec![
                    ("ignited_total".into(), Json::num(q.ignited_total() as f64)),
                    ("stalled_total".into(), Json::num(q.stalled_total() as f64)),
                    ("kinetic_now".into(), Json::num(q.momentum_kinetic() as f64)),
                    ("ignite_threshold".into(), Json::num(q.momentum_ignite())),
                    ("stall_threshold".into(), Json::num(q.momentum_stall())),
                ]),
            ));
            pairs.push((
                "transport".into(),
                Json::Obj(vec![
                    ("steps".into(), Json::num(q.reasoning_steps_total() as f64)),
                    ("moved_total".into(), Json::num(q.transport_moved_total() as f64)),
                    (
                        "theta_shift".into(),
                        Json::num((q.transport_theta_total() * 1e6).round() / 1e6),
                    ),
                    ("dt".into(), Json::num(cfg.dt)),
                ]),
            ));
            pairs.push((
                "memory_bytes".into(),
                Json::Obj(vec![
                    ("lattice".into(), Json::num(mem.lattice as f64)),
                    ("momentum".into(), Json::num(mem.momentum as f64)),
                    ("pairs".into(), Json::num(mem.pairs as f64)),
                    ("doc_freq".into(), Json::num(mem.doc_freq as f64)),
                    ("total".into(), Json::num(mem.total as f64)),
                ]),
            ));
        }
        // RQ10: гироскоп J = A − Aᵀ — статистика реляционной памяти
        // (только плотный путь; --quantized --gyro отклонён валидацией).
        if let (Some(window), Trainer::Dense(engine)) = (cfg.gyro, &trainer) {
            if let Some(g) = engine.gyro() {
                let section_bytes = container_bytes
                    .saturating_sub(pqw::HEADER_SIZE + (cfg.dim as usize).div_ceil(4));
                let lambda_top = g
                    .resonant_modes(cfg.epsilon as f64, 1)
                    .first()
                    .map(|m| m.lambda)
                    .unwrap_or(0.0);
                pairs.push((
                    "gyro".into(),
                    Json::Obj(vec![
                        ("window".into(), Json::num(window as f64)),
                        ("budget".into(), Json::num(cfg.gyro_budget as f64)),
                        ("ticks".into(), Json::num(g.ticks() as f64)),
                        ("pairs_raw".into(), Json::num(g.raw_pairs() as f64)),
                        ("pairs_stored".into(), Json::num(gyro_pairs as f64)),
                        ("section_bytes".into(), Json::num(section_bytes as f64)),
                        (
                            "lambda_top".into(),
                            Json::num((lambda_top * 1e6).round() / 1e6),
                        ),
                    ]),
                ));
            }
        }
        if let Some(p) = &cfg.resume {
            pairs.push(("resume_path".into(), Json::str(p.clone())));
            if let Some(n) = resumed_nnz {
                pairs.push(("resume_nnz".into(), Json::num(n as f64)));
            }
        }
        if stages.len() > 1 {
            let arr: Vec<Json> = stage_stats
                .iter()
                .map(|s| {
                    Json::Obj(vec![
                        ("level".into(), Json::num(s.level as f64)),
                        ("name".into(), Json::str(s.name)),
                        ("block".into(), Json::num(s.block as f64)),
                        ("steps".into(), Json::num(s.steps as f64)),
                        ("shots".into(), Json::num(s.shots as f64)),
                        ("budget_bytes".into(), Json::num(s.budget as f64)),
                        ("blocks".into(), Json::num(s.blocks as f64)),
                        ("tokens".into(), Json::num(s.tokens as f64)),
                        ("nnz_before".into(), Json::num(s.nnz_before as f64)),
                        ("nnz_after".into(), Json::num(s.nnz_after as f64)),
                        ("support_before".into(), Json::num(s.support_before as f64)),
                        ("support_after".into(), Json::num(s.support_after as f64)),
                        (
                            "elapsed_sec".into(),
                            Json::num((s.elapsed * 100.0).round() / 100.0),
                        ),
                    ])
                })
                .collect();
            pairs.push(("curriculum".into(), Json::Arr(arr)));
        }
        let j = Json::Obj(pairs);
        println!("{}", j.to_string());
    } else {
        println!("------------------------------------------------------------");
        if stages.len() > 1 {
            println!("фазовая сборка (память одна, чанк растёт):");
            for s in &stage_stats {
                println!(
                    "  L{} {:<38} чанк {:>5} B | nnz {:>4}→{:<4} | support {:>4}→{:<4} | {:>6.1}s",
                    s.level,
                    format!("«{}»", s.name),
                    s.block,
                    s.nnz_before,
                    s.nnz_after,
                    s.support_before,
                    s.support_after,
                    s.elapsed
                );
            }
            println!("------------------------------------------------------------");
        }
        println!(
            "learned   : nnz_lens={} дуг (union support={}), плотность {:.2}% от d_pol={}",
            nnz_lens, support, density, cfg.dim
        );
        if let (Some(_), Trainer::QuantizedGyro(q)) = (cfg.gyro, &trainer) {
            let section_bytes = container_bytes
                .saturating_sub(pqw::HEADER_SIZE + (cfg.dim as usize).div_ceil(4));
            println!(
                "stream    : {} блоков, {} токенов, no_hits={}, loss={:.6}, перещёлкнуто тритов={} (доля {:.4})",
                total_blocks, total_tokens, no_hits, last_loss, st.moved_total, st.last_moved_frac
            );
            println!(
                "gyro      : {} тактов, {} насыщенных русел J, решётка пар {} Б (ниббл на пару), секция {} Б",
                q.gyro_ticks(),
                q.channel_count(),
                q.pair_bytes(),
                section_bytes
            );
            println!(
                "momentum  : зажиганий {}, срывов {}, кинетических дуг сейчас {} — инерция без затухания",
                q.ignited_total(),
                q.stalled_total(),
                q.momentum_kinetic()
            );
            println!(
                "reasoning : {} шагов Π_Λ(e^{{Δt·J}}p), транспорт двинул {} тритов (Σ|Δθ| = {:.1})",
                q.reasoning_steps_total(),
                q.transport_moved_total(),
                q.transport_theta_total()
            );
        } else if cfg.quantized {
            println!(
                "stream    : {} блоков, {} токенов, no_hits={}, loss={:.6}, перещёлкнуто тритов={} (доля {:.4})",
                total_blocks, total_tokens, no_hits, last_loss, st.moved_total, st.last_moved_frac
            );
        } else {
            println!(
                "stream    : {} блоков, {} токенов, no_hits={}, loss={:.6}, QCM gap={:.6}",
                total_blocks, total_tokens, no_hits, last_loss, last_qcm_gap
            );
        }
        if let Some(p) = &cfg.out {
            let fmt = if gyro_pairs > 0 {
                // RQ17: чекпоинт v4 (фазы + русла + лексикон), если слова
                // усвоены; иначе v3 как в RQ16.
                match trainer.lexicon_len() {
                    Some(n) if n > 0 => {
                        "POLER_Q4 Packed4 + гироскоп J + лексикон LEXI"
                    }
                    _ => "POLER_Q3 Packed4 + гироскоп J = A − Aᵀ",
                }
            } else {
                "POLER_Q2 Packed4"
            };
            println!("checkpoint: {p} ({container_bytes} B, {fmt})");
        }
        if let (Some(_), Trainer::Dense(engine)) = (cfg.gyro, &trainer) {
            if let Some(g) = engine.gyro() {
                let section_bytes = container_bytes
                    .saturating_sub(pqw::HEADER_SIZE + (cfg.dim as usize).div_ceil(4));
                println!(
                    "gyro      : {} тактов, {} сырых пар → {} хранимых, секция {} B ({} Б/пара)",
                    g.ticks(),
                    g.raw_pairs(),
                    gyro_pairs,
                    section_bytes,
                    if gyro_pairs > 0 {
                        section_bytes / gyro_pairs
                    } else {
                        0
                    }
                );
                let lambda_top = g
                    .resonant_modes(cfg.epsilon as f64, 1)
                    .first()
                    .map(|m| m.lambda)
                    .unwrap_or(0.0);
                if lambda_top > 0.0 {
                    println!(
                        "моды Im(P): λ_max = {lambda_top:.4} — главная плоскость вращения смысла"
                    );
                }
            }
        }
        if let Some(p) = &cfg.fingerprint {
            let bytes = cfg.dim as usize / 4;
            println!("fingerprint: {p} ({bytes} B raw Packed4 — снимок фазовой памяти)");
        }
        println!(
            "elapsed   : {:.1}s ({:.1} блоков/с)",
            elapsed,
            total_blocks as f64 / elapsed.max(1e-9)
        );
    }
    0
}

// ============================================================================
// RQ17: L5-ГЕНЕРАЦИЯ — первые слова и рассуждение
// ============================================================================

use pqc::archetype_lattice::{archetype_product_packed4, DEFAULT_BRIDGE_EPS};
use pqc::generate::{
    GenerationReport, GeneratorConfig, L5Generator, DEFAULT_FOCUS_RADIUS, FOCUS_RADIUS_MAX,
};
use pqc::gyro_lattice::QuantizedGyroCurriculum;
use pqc::merge::{merge_brains, MergeConfig};

/// Общая конфигурация команд генерации.
struct GenConfig {
    brain: Option<String>,
    corpus: Option<String>,
    corpus_file: Option<String>,
    prompt: String,
    think: usize,
    max_tokens: usize,
    window: usize,
    seed: u64,
    free: bool,
    morphemes: bool,
    repeat_veto: usize,
    bridge: bool,
    bridge_eps: f64,
    /// RQ21: грамматические мосты (включены по умолчанию в CLI —
    /// качество речи; `--no-syntax` возвращает чистую топологию RQ17).
    syntax: bool,
    /// RQ22: радиус фокуса волны (маршрутизация от дуг вопроса;
    /// `--focus-radius N` / `--no-focus`; дефолт 3).
    focus_radius: usize,
    /// RQ22: самоподкрепление грамматики (`--no-reinforce` снимает).
    reinforce: bool,
    /// RQ23: имя собеседника для автобиографической памяти
    /// (`--name ИМЯ`; пусто — аноним).
    name: Option<String>,
    /// RQ23: холодный старт волны (`--fresh` — не подхватывать нить
    /// прошлого диалога из контекст-рефлекса W).
    fresh: bool,
    learn: bool,
    json: bool,
    /// Позиционный вопрос (ask/chat).
    question: Option<String>,
}

impl Default for GenConfig {
    fn default() -> Self {
        GenConfig {
            brain: None,
            corpus: None,
            corpus_file: None,
            prompt: String::new(),
            think: 4,
            max_tokens: 64,
            window: 8,
            seed: 42,
            free: false,
            morphemes: false,
            repeat_veto: 1,
            bridge: true,
            bridge_eps: DEFAULT_BRIDGE_EPS,
            syntax: true,
            focus_radius: DEFAULT_FOCUS_RADIUS,
            reinforce: true,
            name: None,
            fresh: false,
            learn: true,
            json: false,
            question: None,
        }
    }
}

fn gen_usage_err(msg: &str) -> i32 {
    eprintln!("pqc generate/ask: {msg}\n(см. pqc --help: GENERATE/ASK/CHAT OPTIONS)");
    2
}

/// Разбор общих опций генерации (generate/step/ask/chat).
fn parse_gen_args(args: &[String], cfg: &mut GenConfig) -> Result<(), String> {
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        let mut val = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match a.as_str() {
            "--brain" => cfg.brain = Some(val("--brain")?),
            "--corpus" => cfg.corpus = Some(val("--corpus")?),
            "--corpus-file" => cfg.corpus_file = Some(val("--corpus-file")?),
            "--prompt" => cfg.prompt = val("--prompt")?,
            "--think" => {
                cfg.think = val("--think")?
                    .parse()
                    .map_err(|_| "bad --think".to_string())?
            }
            "--max-tokens" => {
                cfg.max_tokens = val("--max-tokens")?
                    .parse()
                    .map_err(|_| "bad --max-tokens".to_string())?
            }
            "--window" => {
                cfg.window = val("--window")?
                    .parse()
                    .map_err(|_| "bad --window".to_string())?
            }
            "--seed" => {
                cfg.seed = val("--seed")?
                    .parse()
                    .map_err(|_| "bad --seed".to_string())?
            }
            "--free" => cfg.free = true,
            "--morphemes" => cfg.morphemes = true,
            "--repeat-veto" => {
                cfg.repeat_veto = val("--repeat-veto")?
                    .parse()
                    .map_err(|_| "bad --repeat-veto".to_string())?
            }
            "--no-bridge" => cfg.bridge = false,
            "--no-syntax" => cfg.syntax = false,
            "--no-focus" => cfg.focus_radius = 0,
            "--no-reinforce" => cfg.reinforce = false,
            "--focus-radius" => {
                let v = val("--focus-radius")?;
                let r: usize = v
                    .parse()
                    .map_err(|_| "--focus-radius: целое 0..=8".to_string())?;
                if r > FOCUS_RADIUS_MAX {
                    return Err("--focus-radius: целое 0..=8 (0 — выключить фокус)".to_string());
                }
                cfg.focus_radius = r;
            }
            "--bridge-eps" => {
                cfg.bridge_eps = val("--bridge-eps")?
                    .parse()
                    .map_err(|_| "bad --bridge-eps".to_string())?;
                if !cfg.bridge_eps.is_finite() || cfg.bridge_eps <= 0.0 || cfg.bridge_eps > 1.0
                {
                    return Err("--bridge-eps: порог гейта ∈ (0, 1]".to_string());
                }
            }
            "--no-learn" => cfg.learn = false,
            "--name" => cfg.name = Some(val("--name")?),
            "--fresh" => cfg.fresh = true,
            "--json" => cfg.json = true,
            _ if !a.starts_with("--") && cfg.question.is_none() => {
                cfg.question = Some(a);
            }
            _ => return Err(format!("неизвестная опция: {a}")),
        }
        i += 1;
    }
    Ok(())
}

/// Источник движка: мозг из файла или обучение на лету.
enum BrainSource {
    /// Загруженный контейнер (путь для обратной записи памяти диалога).
    File { path: String, engine: QuantizedGyroCurriculum },
    /// Движок, обученный на лету (без контейнера).
    Inline(QuantizedGyroCurriculum),
}

/// Размерность/пороги по умолчанию для обучения на лету.
const GEN_INLINE_DIM: u32 = 1024;
const GEN_INLINE_EPSILON: f32 = 0.05;
const GEN_INLINE_WINDOW: usize = 8;

fn load_gen_engine(cfg: &GenConfig) -> Result<BrainSource, String> {
    if let Some(path) = &cfg.brain {
        let src = load(path)?;
        let reader = pqw::PqwReader::from_bytes(src.as_slice())
            .map_err(|e| format!("--brain {path}: {e}"))?;
        if reader.encoding() != pqw::phase::TritEncoding::Packed4 {
            return Err(format!(
                "--brain {path}: нужен контейнер v2/v3/v4/v5 (Packed4); \
                 обучите pqc train --quantized --gyro"
            ));
        }
        let window = reader
            .gyro()
            .map(|g| g.window().max(1) as usize)
            .unwrap_or(GEN_INLINE_WINDOW);
        let eps = reader.hyperparams().epsilon_threshold;
        let mut engine = QuantizedGyroCurriculum::new(reader.d_pol(), eps, cfg.seed, window)
            .map_err(|e| e.to_string())?;
        engine
            .resume_from_reader(&reader)
            .map_err(|e| format!("--brain {path}: {e}"))?;
        if engine.lexicon_len() == 0 {
            eprintln!(
                "pqc: предупреждение: мозг {path} без лексикона (v3 от v0.6.0) — \
                 слов нет; перезапустите обучение pqc train --quantized --gyro \
                 (v0.7.0+) или включите --morphemes"
            );
        }
        return Ok(BrainSource::File {
            path: path.clone(),
            engine,
        });
    }
    // Обучение на лету: inline текст или файл (абзацы — документы).
    let text = if let Some(t) = &cfg.corpus {
        t.clone()
    } else if let Some(p) = &cfg.corpus_file {
        std::fs::read_to_string(p).map_err(|e| format!("--corpus-file {p}: {e}"))?
    } else {
        return Err(
            "нужен --brain F (контейнер-мозг) или --corpus TEXT / --corpus-file F \
             (обучение на лету)"
                .to_string(),
        );
    };
    let mut engine = QuantizedGyroCurriculum::new(
        GEN_INLINE_DIM,
        GEN_INLINE_EPSILON,
        cfg.seed,
        GEN_INLINE_WINDOW,
    )
    .map_err(|e| e.to_string())?;
    for para in text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        engine
            .ingest(para, 1)
            .map_err(|e| format!("corpus ingest: {e}"))?;
    }
    Ok(BrainSource::Inline(engine))
}

/// JSON-отчёт генерации (машинно-читаемый протокол RQ17).
fn generation_json_pairs(
    rep: &GenerationReport,
    learned: Option<&Json>,
) -> Vec<(String, Json)> {
    let mut obj = vec![
        ("prompt_tokens".into(), Json::num(rep.prompt_tokens as f64)),
        ("prompt_arcs".into(), Json::num(rep.prompt_arcs as f64)),
        ("ignited".into(), Json::num(rep.ignited as f64)),
        ("auto_free".into(), Json::Bool(rep.auto_free)),
        (
            "bridge".into(),
            Json::Obj(vec![
                ("energy".into(), Json::num(rep.bridge_energy)),
                ("co_support".into(), Json::num(rep.bridge_co as f64)),
                ("resonance".into(), Json::num(rep.bridge_resonance as f64)),
                ("conflict".into(), Json::num(rep.bridge_conflict as f64)),
                ("resonant".into(), Json::Bool(rep.bridge_resonant)),
            ]),
        ),
        ("think_moved".into(), Json::num(rep.think_moved as f64)),
        ("think_theta_shift".into(), Json::num(rep.think_theta_shift)),
        (
            "steps".into(),
            Json::Arr(
                rep.steps
                    .iter()
                    .map(|s| {
                        Json::Obj(vec![
                            ("coord".into(), Json::num(s.coord as f64)),
                            ("token".into(), Json::str(&s.token)),
                            (
                                "source".into(),
                                Json::str(match s.source {
                                    pqc::generate::TicketSource::Flow => "flow",
                                    pqc::generate::TicketSource::Backtrack => "backtrack",
                                    pqc::generate::TicketSource::Archetype => "archetype",
                                    pqc::generate::TicketSource::Kinetic => "kinetic",
                                    pqc::generate::TicketSource::Bridge => "bridge",
                                }),
                            ),
                            ("tickets".into(), Json::num(s.tickets as f64)),
                            ("born_bit".into(), Json::Bool(s.born_bit)),
                            ("moved".into(), Json::num(s.moved as f64)),
                            ("theta_shift".into(), Json::num(s.theta_shift)),
                            ("morpheme".into(), Json::Bool(s.morpheme)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("text".into(), Json::str(&rep.text)),
        ("tokens".into(), Json::num(rep.steps.len() as f64)),
        (
            "morpheme_tokens".into(),
            {
                let n = rep.steps.iter().filter(|s| s.morpheme).count();
                Json::num(n as f64)
            },
        ),
        ("bridge_tokens".into(), Json::num(rep.bridges as f64)),
        ("syntax_run_max".into(), Json::num(rep.syntax_run_max as f64)),
        (
            "syntax_bridges_density".into(),
            Json::num(rep.syntax_bridges_density),
        ),
        ("reinforced".into(), Json::num(rep.reinforced as f64)),
        (
            "focus".into(),
            Json::Obj(vec![
                ("attractor_arcs".into(), Json::num(rep.attractor_arcs as f64)),
                ("radius".into(), Json::num(rep.focus_radius as f64)),
                ("active".into(), Json::Bool(rep.focus_active)),
                ("focused_steps".into(), Json::num(rep.focused_steps as f64)),
                ("wander_steps".into(), Json::num(rep.wander_steps as f64)),
                ("relevance".into(), Json::num(rep.relevance)),
            ]),
        ),
        ("skipped_unseen".into(), Json::num(rep.skipped_unseen as f64)),
        ("converged".into(), Json::Bool(rep.converged)),
        ("cycled".into(), Json::Bool(rep.cycled)),
        (
            "elapsed_us".into(),
            Json::num(rep.elapsed.as_micros() as f64),
        ),
    ];
    if let Some(l) = learned {
        obj.push(("memory".into(), l.clone()));
    }
    obj
}

/// Печать человеческого отчёта генерации.
fn print_generation_human(rep: &GenerationReport, step_mode: bool) {
    if step_mode {
        if let Some(s) = rep.steps.first() {
            let src = match s.source {
                pqc::generate::TicketSource::Flow => "русло вперёд",
                pqc::generate::TicketSource::Backtrack => "возврат по руслу",
                pqc::generate::TicketSource::Archetype => "архетипический мост (⊗_ε)",
                pqc::generate::TicketSource::Kinetic => "кинетика (внутренний голос)",
                pqc::generate::TicketSource::Bridge => "грамматический мост (синтаксис)",
            };
            println!("квант авторегрессии:");
            println!(
                "  лотерея : {} билетов ({src}), выпала координата {}",
                s.tickets, s.coord
            );
            println!(
                "  декод   : {} → «{}»{}",
                s.coord,
                s.token,
                if s.morpheme { " (AOT-морфема)" } else { "" }
            );
            println!(
                "  born    : бит {} (полюс {}), транспорт: {} перещёлкиваний, θ-сдвиг {:.3}",
                if s.born_bit { 1 } else { 0 },
                if s.born_bit { "−1" } else { "+1" },
                s.moved,
                s.theta_shift
            );
        } else {
            println!("квант авторегрессии: волна иссякла (сходимость ĤΨ = 0)");
        }
        return;
    }
    for (k, s) in rep.steps.iter().enumerate() {
        let src = match s.source {
            pqc::generate::TicketSource::Flow => "→",
            pqc::generate::TicketSource::Backtrack => "←",
            pqc::generate::TicketSource::Archetype => "⊗",
            pqc::generate::TicketSource::Kinetic => "∘",
            pqc::generate::TicketSource::Bridge => "≡",
        };
        println!(
            "  #{:<3} [ {:<5} ] «{}» {} (билетов {:>3}, born {}, moved {:>2})",
            k + 1,
            s.coord,
            s.token,
            src,
            s.tickets,
            if s.born_bit { 1 } else { 0 },
            s.moved
        );
    }
    if rep.steps.is_empty() {
        println!("  (молчание: живых русел из промпта нет)");
    }
    let morphemes = rep.steps.iter().filter(|s| s.morpheme).count();
    println!(
        "\nтекст    : {}",
        if rep.text.is_empty() { "—" } else { &rep.text }
    );
    println!(
        "статистика: слов {} (морфем {}, мостов {} — {:.1}/100 слов{}), пик облака {}, пропусков {}, сходимость: {}, цикл: {}, {:.1} мс",
        rep.steps.len(),
        morphemes,
        rep.bridges,
        rep.syntax_bridges_density,
        if rep.reinforced > 0 {
            format!(", укреплено {}", rep.reinforced)
        } else {
            String::new()
        },
        rep.syntax_run_max,
        rep.skipped_unseen,
        if rep.converged { "да (ĤΨ = 0)" } else { "нет" },
        if rep.cycled { "вырожденный" } else { "нет" },
        rep.elapsed.as_secs_f64() * 1000.0
    );
    // RQ22: фокус волны и релевантность ответа.
    if rep.attractor_arcs > 0 {
        if rep.focus_active {
            println!(
                "фокус    : аттрактор {} дуг, радиус {} — {} шагов в фокусе, {} блужданий, релевантность {:.1}%",
                rep.attractor_arcs,
                rep.focus_radius,
                rep.focused_steps,
                rep.wander_steps,
                rep.relevance * 100.0
            );
        } else {
            println!(
                "фокус    : выключен — свободная речь держится темы на {:.1}% (аттрактор {} дуг)",
                rep.relevance * 100.0,
                rep.attractor_arcs
            );
        }
    }
}

/// `pqc archetype`: нелинейная алгебра архетипов RQ18 — интроспекция
/// оператора `a ⊗_ε b`. Произведение мозга с промптом (`--prompt`)
/// или с другим мозгом (`--with`): структурный изоморфизм, резонанс,
/// конфликт, энергия пересечения.
fn cmd_archetype(args: &[String]) -> i32 {
    let mut brain: Option<String> = None;
    let mut with: Option<String> = None;
    let mut prompt = String::new();
    let mut eps = DEFAULT_BRIDGE_EPS;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        let mut val = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match a.as_str() {
            "--brain" => brain = Some(val("--brain").unwrap_or_default()),
            "--with" => with = Some(val("--with").unwrap_or_default()),
            "--prompt" => prompt = val("--prompt").unwrap_or_default(),
            "--eps" => match val("--eps").map_err(|_| ()).and_then(|v| v.parse::<f64>().map_err(|_| ())) {
                Ok(e) if e.is_finite() && e > 0.0 && e <= 1.0 => eps = e,
                _ => return arch_usage_err("--eps: порог гейта ∈ (0, 1]"),
            },
            "--json" => json = true,
            _ if !a.starts_with("--") && prompt.is_empty() => prompt = a,
            _ => return arch_usage_err(&format!("неизвестная опция: {a}")),
        }
        i += 1;
    }
    let err = |m: &str| -> i32 {
        eprintln!("pqc archetype: {m}");
        2
    };
    let Some(path) = brain else {
        return err("нужен --brain F (контейнер-мозг v2/v3/v4)");
    };
    if with.is_some() && !prompt.is_empty() {
        return err("--with и --prompt взаимно исключают друг друга");
    }
    // Мозг A.
    let src = match load(&path) {
        Ok(s) => s,
        Err(e) => return err(&format!("{path}: {e}")),
    };
    let reader = match pqw::PqwReader::from_bytes(src.as_slice()) {
        Ok(r) => r,
        Err(e) => return err(&format!("{path}: {e}")),
    };
    if reader.encoding() != pqw::phase::TritEncoding::Packed4 {
        return err(&format!("{path}: нужен контейнер v2/v3/v4 (Packed4)"));
    }
    let window = reader
        .gyro()
        .map(|g| g.window().max(1) as usize)
        .unwrap_or(GEN_INLINE_WINDOW);
    let engine_eps = reader.hyperparams().epsilon_threshold;
    let mut engine = match QuantizedGyroCurriculum::new(
        reader.d_pol(),
        engine_eps,
        42,
        window,
    ) {
        Ok(e) => e,
        Err(e) => return err(&e.to_string()),
    };
    if let Err(e) = engine.resume_from_reader(&reader) {
        return err(&format!("{path}: {e}"));
    }
    let d = engine.d_pol() as usize;
    let lattice = engine.lattice().to_vec();
    let nnz_a = engine.nnz();

    // Сомножитель B: архетип второго мозга или промпта.
    let (name_b, arch_b) = if let Some(p2) = &with {
        let src2 = match load(p2) {
            Ok(s) => s,
            Err(e) => return err(&format!("{p2}: {e}")),
        };
        let r2 = match pqw::PqwReader::from_bytes(src2.as_slice()) {
            Ok(r) => r,
            Err(e) => return err(&format!("{p2}: {e}")),
        };
        if r2.encoding() != pqw::phase::TritEncoding::Packed4 {
            return err(&format!("{p2}: нужен контейнер v2/v3/v4 (Packed4)"));
        }
        if r2.d_pol() != engine.d_pol() {
            return err(&format!(
                "размерности решёток различаются: {path} d_pol={}, {p2} d_pol={} \
                 (⊗_ε требует общее фазовое пространство)",
                engine.d_pol(),
                r2.d_pol()
            ));
        }
        // Чистые фазы второго мозга — без поднятия его русел.
        let mut other = vec![0u8; lattice.len()];
        for i in 0..d {
            let t = pqw::trit_bloch::trit_at(&r2.phase_bytes(), i);
            let code = pqw::phase::pack_trit2(t);
            other[i / 4] |= code << (2 * (i % 4));
        }
        (format!("мозг {p2}"), other)
    } else {
        (format!("промпт «{prompt}»"), engine.prompt_archetype(&prompt))
    };

    // c = a ⊗_ε b — ядро RQ18.
    let mut prod = vec![0u8; lattice.len()];
    let st = match archetype_product_packed4(&lattice, &arch_b, d, eps, &mut prod) {
        Ok(s) => s,
        Err(e) => return err(&e.to_string()),
    };
    let nnz_prod = st.resonant.then_some(st.nnz_out);

    if json {
        let obj = Json::Obj(vec![
            (
                "brain".into(),
                Json::Obj(vec![
                    ("path".into(), Json::str(&path)),
                    ("d_pol".into(), Json::num(d as f64)),
                    ("nnz".into(), Json::num(nnz_a as f64)),
                ]),
            ),
            (
                "operand".into(),
                Json::Obj(vec![
                    ("name".into(), Json::str(&name_b)),
                    ("nnz".into(), Json::num(st.nnz_b as f64)),
                ]),
            ),
            (
                "product".into(),
                Json::Obj(vec![
                    ("nnz".into(), Json::num(st.nnz_out as f64)),
                    ("co_support".into(), Json::num(st.co_support as f64)),
                    ("resonance".into(), Json::num(st.resonance as f64)),
                    ("conflict".into(), Json::num(st.conflict as f64)),
                    ("energy".into(), Json::num(st.energy)),
                    ("resonant".into(), Json::Bool(st.resonant)),
                ]),
            ),
            ("eps".into(), Json::num(eps)),
        ]);
        println!("{}", obj.to_string());
        return 0;
    }

    println!("POLER Quantum Core — Archetype Algebra (RQ18): c = a ⊗_ε b");
    println!("мозг A   : {path} (d_pol={d}, носитель {nnz_a} дуг)");
    println!("мозг B   : {name_b} (носитель {} дуг)", st.nnz_b);
    println!(
        "гейт ε   : {eps} — энергия E = co/min(nnz) = {:.3} → {}",
        st.energy,
        if st.resonant { "РЕЗОНАНС (изоморфизм найден)" } else { "ЗАПЕРТ (архетипы ортогональны)" }
    );
    println!(
        "пересечение: {} дуг — согласие {} (конструктивная интерференция), \
         конфликт {} (аннигиляция в суперпозицию)",
        st.co_support, st.resonance, st.conflict
    );
    match nnz_prod {
        Some(n) => println!(
            "продукт  : {n} дуг — новый устойчивый смысл (Π_Λ-проекция, \
             идемпотентен: c ⊗_ε c = c)"
        ),
        None => println!(
            "продукт  : Zero — ложных ассоциаций не существует (E < ε)"
        ),
    }
    0
}

// ============================================================================
// RQ19: pqc learn — целенаправленный интернет-ингест
// ============================================================================

fn learn_usage_err(msg: &str) -> i32 {
    eprintln!("pqc learn: {msg}");
    eprintln!("формат: pqc learn \"ТЕМА\" --brain F [--pages N] [--rounds N]");
    2
}

/// Offline-источник: одна «страница» из --text (тема = заголовок).
struct OfflineSource {
    topic: String,
    text: String,
}

impl pqc::learn_net::TextSource for OfflineSource {
    fn search(&mut self, query: &str, _limit: usize) -> Result<Vec<(String, String)>, String> {
        if query == self.topic {
            Ok(vec![(self.topic.clone(), self.text.chars().take(60).collect())])
        } else {
            Ok(Vec::new())
        }
    }
    fn extracts(&mut self, titles: &[String], _intro: bool) -> Result<Vec<(String, String)>, String> {
        Ok(titles
            .iter()
            .filter(|t| t.as_str() == self.topic)
            .map(|t| (t.clone(), self.text.clone()))
            .collect())
    }
}

fn cmd_learn(args: &[String]) -> i32 {
    let mut topic = String::new();
    let mut brain: Option<String> = None;
    let mut pages = 5usize;
    let mut rounds = 2usize;
    let mut lang = String::from("auto");
    let mut full = false;
    let mut ask: Option<String> = None;
    let mut seed = 42u64;
    let mut dim = 4096u32;
    let mut text: Option<String> = None;
    let mut docs: Vec<String> = Vec::new();
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        let mut val = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("нет значения у {name}"))
        };
        match a.as_str() {
            "--brain" => match val("--brain") {
                Ok(v) => brain = Some(v),
                Err(e) => return learn_usage_err(&e),
            },
            "--pages" => match val("--pages").unwrap_or_default().parse::<usize>() {
                Ok(n) if n >= 1 && n <= 50 => pages = n,
                _ => return learn_usage_err("--pages: целое 1..=50"),
            },
            "--rounds" => match val("--rounds").unwrap_or_default().parse::<usize>() {
                Ok(n) if n >= 1 && n <= 10 => rounds = n,
                _ => return learn_usage_err("--rounds: целое 1..=10"),
            },
            "--lang" => match val("--lang").unwrap_or_default() {
                v if v == "auto" || v == "ru" || v == "en" => lang = v,
                _ => return learn_usage_err("--lang: auto | ru | en"),
            },
            "--full" => full = true,
            "--ask" => match val("--ask") {
                Ok(v) => ask = Some(v),
                Err(e) => return learn_usage_err(&e),
            },
            "--seed" => match val("--seed").unwrap_or_default().parse::<u64>() {
                Ok(s) => seed = s,
                _ => return learn_usage_err("--seed: целое u64"),
            },
            "--dim" => match val("--dim").unwrap_or_default().parse::<u32>() {
                Ok(d) if (64..=16384).contains(&d) => dim = d,
                _ => return learn_usage_err("--dim: 64..=16384"),
            },
            "--text" => match val("--text") {
                Ok(v) => text = Some(v),
                Err(e) => return learn_usage_err(&e),
            },
            "--docs" => match val("--docs") {
                Ok(v) => docs.push(v),
                Err(e) => return learn_usage_err(&e),
            },
            "--json" => json = true,
            _ if !a.starts_with("--") && topic.is_empty() => topic = a,
            _ => return learn_usage_err(&format!("неизвестная опция: {a}")),
        }
        i += 1;
    }
    let err = |m: &str| -> i32 {
        eprintln!("pqc learn: {m}");
        2
    };
    if topic.trim().is_empty() {
        return err("нужна тема: pqc learn \"квантовая механика\" --brain brain.pqw");
    }
    let Some(path) = brain else {
        return err("нужен --brain F (мозг создаётся/расширяется и сохраняется)");
    };
    let cfg = pqc::learn_net::LearnConfig {
        topic: topic.trim().to_string(),
        pages: if docs.is_empty() { pages } else { docs.len().max(1) },
        rounds: if docs.is_empty() { rounds } else { 1 },
        full,
        seed,
        dim,
        ask,
        ..pqc::learn_net::LearnConfig::default()
    };

    if !json {
        println!("POLER Quantum Core — Learn (RQ19/RQ22): целенаправленный ингест знаний");
        if let Some(t) = &text {
            println!("режим   : offline (--text, {} Б без сети)", t.len());
        } else if !docs.is_empty() {
            println!("источник: документации/markdown через HTTPS (RQ22, {} файлов)", docs.len());
            for d in docs.iter().take(5) {
                println!("         • {d}");
            }
        } else {
            println!("источник: {}.wikipedia.org API (TLS 1.3 zero-dep)", {
                if lang == "auto" {
                    pqc::wikisrc::WikiSource::detect_language(&cfg.topic)
                } else {
                    lang.as_str()
                }
            });
        }
        println!("мозг    : {path} (d_pol={dim}, раундов={}, страниц/раунд={})", cfg.rounds, cfg.pages);
    }

    // Существующий мозг — расширяем; отсутствующий — создаём.
    let brain_bytes: Option<Vec<u8>> = match std::fs::read(&path) {
        Ok(b) => Some(b),
        Err(_) => None,
    };
    let brain_existed = brain_bytes.is_some();
    let (engine, report) = match (&text, docs.is_empty()) {
        (Some(t), _) => {
            let mut src = OfflineSource { topic: cfg.topic.clone(), text: t.clone() };
            match pqc::learn_net::learn(&cfg, &mut src, brain_bytes.as_deref()) {
                Ok(x) => x,
                Err(e) => return err(&e),
            }
        }
        (None, false) => {
            // RQ22: источник документаций — markdown/текст по HTTPS.
            let mut src = pqc::docsrc::DocsSource::new(&docs);
            match pqc::learn_net::learn(&cfg, &mut src, brain_bytes.as_deref()) {
                Ok(x) => x,
                Err(e) => return err(&format!("{e} (документы/HTTPS)")),
            }
        }
        (None, true) => {
            let lang = if lang == "auto" {
                pqc::wikisrc::WikiSource::detect_language(&cfg.topic).to_string()
            } else {
                lang.clone()
            };
            let mut src = pqc::wikisrc::WikiSource::new(&lang);
            match pqc::learn_net::learn(&cfg, &mut src, brain_bytes.as_deref()) {
                Ok(x) => x,
                Err(e) => return err(&format!("{e} (сеть/Wikipedia API)")),
            }
        }
    };

    // Сохранение контейнера v4.
    let bytes = match engine.checkpoint() {
        Ok(b) => b,
        Err(e) => return err(&format!("чекпоинт: {e}")),
    };
    if let Err(e) = std::fs::write(&path, &bytes) {
        return err(&format!("запись {path}: {e}"));
    }

    if json {
        let rounds_json: Vec<Json> = report
            .rounds
            .iter()
            .map(|r| {
                Json::Obj(vec![
                    ("query".into(), Json::str(&r.query)),
                    ("titles".into(), Json::Arr(r.titles.iter().map(|t| Json::str(t)).collect())),
                    ("ingested".into(), Json::num(r.ingested as f64)),
                    ("chars".into(), Json::num(r.chars as f64)),
                    ("moved".into(), Json::num(r.moved as f64)),
                ])
            })
            .collect();
        let mut j = vec![
            ("brain".into(), Json::Obj(vec![
                ("path".into(), Json::str(&path)),
                ("existed".into(), Json::Bool(brain_existed)),
                ("d_pol".into(), Json::num(engine.d_pol() as f64)),
                ("channels".into(), Json::num(report.channels_after as f64)),
                ("lexicon".into(), Json::num(report.lexicon_after as f64)),
                ("bytes".into(), Json::num(report.brain_bytes as f64)),
            ])),
            ("topic".into(), Json::str(&cfg.topic)),
            ("pages".into(), Json::num(report.pages as f64)),
            ("chars".into(), Json::num(report.chars as f64)),
            ("moved_total".into(), Json::num(report.moved_total() as f64)),
            ("lexicon".into(), Json::Obj(vec![
                ("before".into(), Json::num(report.lexicon_before as f64)),
                ("after".into(), Json::num(report.lexicon_after as f64)),
            ])),
            ("channels".into(), Json::Obj(vec![
                ("before".into(), Json::num(report.channels_before as f64)),
                ("after".into(), Json::num(report.channels_after as f64)),
            ])),
            ("lattice_nnz".into(), Json::num(report.lattice_nnz as f64)),
            ("rounds".into(), Json::Arr(rounds_json)),
        ];
        if let Some(ans) = &report.answer {
            j.push(("ask".into(), Json::Obj(vec![
                ("question".into(), Json::str(cfg.ask.as_deref().unwrap_or(""))),
                ("answer".into(), Json::str(&ans.text)),
                ("tokens".into(), Json::num(ans.steps.len() as f64)),
            ])));
        }
        println!("{}", Json::Obj(j).to_string());
    } else {
        for r in &report.rounds {
            println!(
                "\nраунд  : «{}» → {} стр. ({} Б)",
                r.query,
                r.titles.len(),
                r.chars
            );
            for t in r.titles.iter().take(5) {
                println!("         • {t}");
            }
        }
        println!(
            "\nингест : {} страниц, {} Б текста, {} born-перещёлкиваний",
            report.pages,
            report.chars,
            report.moved_total()
        );
        println!(
            "решётка: nnz={}, русла J {} → {}, лексикон {} → {} слов",
            report.lattice_nnz,
            report.channels_before,
            report.channels_after,
            report.lexicon_before,
            report.lexicon_after
        );
        println!("сохранено: {path} → v4 контейнер ({} Б)", report.brain_bytes);
        if let Some(ans) = &report.answer {
            println!(
                "\nвопрос : {}",
                cfg.ask.as_deref().unwrap_or("")
            );
            println!("ответ  : {}", if ans.text.is_empty() { "(молчание)" } else { &ans.text });
        }
        println!("\nпроверка: pqc ask \"{}\" --brain {path}", cfg.topic);
    }
    0
}

// ============================================================================
// RQ20: pqc merge — архетипическое слияние мозгов
// ============================================================================

fn merge_usage_err(msg: &str) -> i32 {
    eprintln!("pqc merge: {msg}");
    eprintln!("формат: pqc merge BRAIN_A.pqw BRAIN_B.pqw --out MERGED.pqw [--ask Q]");
    2
}

/// `pqc merge A.pqw B.pqw --out M.pqw`: слияние знаний двух мозгов —
/// фазы `c = a ⊗_ε b` (таблица интерференции без гейта: прозрачность
/// объединяет, конфликты аннигилируют), русла `J = Π_Λ(J_A + J_B)`
/// (кососимметричность по построению), лексиконы LEXI консолидируются
/// (A — база, B заполняет пустые координаты). Born-консолидация:
/// продукт пишется бит-в-бит в контейнер v4/v3/v2.
/// Живая проверка слитого мозга: ответ на вопрос без записи в
/// контейнер (байты слияния остаются детерминированными).
fn merge_ask(merged: &[u8], question: &str, seed: u64) -> Result<String, String> {
    let reader = pqw::PqwReader::from_bytes(merged).map_err(|e| format!("слитый мозг: {e}"))?;
    let window = reader
        .gyro()
        .map(|g| g.window().max(1) as usize)
        .unwrap_or(GEN_INLINE_WINDOW);
    let eps = reader.hyperparams().epsilon_threshold;
    let mut engine =
        QuantizedGyroCurriculum::new(reader.d_pol(), eps, seed, window).map_err(|e| e.to_string())?;
    engine
        .resume_from_reader(&reader)
        .map_err(|e| format!("resume: {e}"))?;
    let gcfg = GeneratorConfig {
        think_steps: 4,
        max_tokens: 24,
        window: engine.gyro_window(),
        seed,
        syntax: true,
        focus_radius: DEFAULT_FOCUS_RADIUS,
        reinforce: true,
        ..GeneratorConfig::default()
    };
    let mut gen = L5Generator::new(&mut engine, gcfg).map_err(|e| e.to_string())?;
    gen.generate(question).map(|rep| rep.text).map_err(|e| e.to_string())
}

fn cmd_merge(args: &[String]) -> i32 {
    let mut brains: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut eps = DEFAULT_BRIDGE_EPS;
    let mut ask: Option<String> = None;
    let mut seed = 42u64;
    let mut settle: Option<usize> = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        let mut val = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("нет значения у {name}"))
        };
        match a.as_str() {
            "--out" => match val("--out") {
                Ok(v) => out = Some(v),
                Err(e) => return merge_usage_err(&e),
            },
            "--eps" => match val("--eps")
                .map_err(|_| ())
                .and_then(|v| v.parse::<f64>().map_err(|_| ()))
            {
                Ok(e) if e.is_finite() && e > 0.0 && e <= 1.0 => eps = e,
                _ => return merge_usage_err("--eps: порог диагностики изоморфизма ∈ (0, 1]"),
            },
            "--settle" => {
                // Значение опционально: --settle → 3 такта (ТЗ 2–4),
                // --settle N → явно (следующий аргумент — число не-флаг).
                let explicit = args.get(i + 1).filter(|v| !v.starts_with("--"));
                match explicit {
                    Some(v) => match v.parse::<usize>() {
                        Ok(n) if (1..=pqc::merge::SETTLE_TICKS_MAX).contains(&n) => {
                            settle = Some(n);
                            i += 1;
                        }
                        _ => {
                            return merge_usage_err(&format!(
                                "--settle: целое 1..={} (ТЗ RQ21: 2–4)",
                                pqc::merge::SETTLE_TICKS_MAX
                            ))
                        }
                    },
                    None => settle = Some(3),
                }
            }
            "--ask" => match val("--ask") {
                Ok(v) => ask = Some(v),
                Err(e) => return merge_usage_err(&e),
            },
            "--seed" => match val("--seed").unwrap_or_default().parse::<u64>() {
                Ok(s) => seed = s,
                _ => return merge_usage_err("--seed: целое u64"),
            },
            "--json" => json = true,
            _ if !a.starts_with("--") && brains.len() < 2 => brains.push(a),
            _ => return merge_usage_err(&format!("неизвестная опция: {a}")),
        }
        i += 1;
    }
    let err = |m: &str| -> i32 {
        eprintln!("pqc merge: {m}");
        2
    };
    if brains.len() != 2 {
        return err("нужно два мозга: pqc merge A.pqw B.pqw --out M.pqw");
    }
    let Some(out_path) = out else {
        return err("нужен --out M.pqw (контейнер слитого мозга)");
    };

    // Мозги.
    let (path_a, path_b) = (&brains[0], &brains[1]);
    let src_a = match load(path_a) {
        Ok(s) => s,
        Err(e) => return err(&format!("{path_a}: {e}")),
    };
    let src_b = match load(path_b) {
        Ok(s) => s,
        Err(e) => return err(&format!("{path_b}: {e}")),
    };

    // Слияние: c = a ⊗_ε b, J = Π_Λ(J_A + J_B), LEXI-консолидация.
    let cfg = MergeConfig { eps };
    let (merged, report) =
        match merge_brains(src_a.as_slice(), src_b.as_slice(), &cfg) {
            Ok(x) => x,
            Err(e) => return err(&e),
        };

    // RQ21: сеттлинг — консолидация волной. 2–4 такта авторегрессии
    // Π_Λ(e^{Δt·J} p) поверх слитых русел: русла доменов прорастают
    // общими связями, система оседает к стационару ĤΨ = 0.
    let (merged, settle_report) = match settle {
        Some(ticks) => {
            match pqc::merge::settle_brain(&merged, &pqc::merge::SettleConfig { ticks }) {
                Ok((bytes, srep)) => (bytes, Some(srep)),
                Err(e) => return err(&e),
            }
        }
        None => (merged, None),
    };
    if let Err(e) = std::fs::write(&out_path, &merged) {
        return err(&format!("запись {out_path}: {e}"));
    }

    // Живая проверка мульти-доменного мышления (без записи в мозг:
    // байты слияния детерминированы только входами A и B).
    let answer = match ask.as_deref().filter(|q| !q.trim().is_empty()) {
        Some(q) => match merge_ask(&merged, q, seed) {
            Ok(text) => Some((q.to_string(), text)),
            Err(e) => return err(&e),
        },
        None => None,
    };

    if json {
        let mut j = vec![
            (
                "brain_a".into(),
                Json::Obj(vec![
                    ("path".into(), Json::str(path_a)),
                    ("d_pol".into(), Json::num(report.d_pol as f64)),
                    ("nnz".into(), Json::num(report.nnz_a as f64)),
                    ("channels".into(), Json::num(report.channels_a as f64)),
                    ("lexicon".into(), Json::num(report.lexicon_a as f64)),
                ]),
            ),
            (
                "brain_b".into(),
                Json::Obj(vec![
                    ("path".into(), Json::str(path_b)),
                    ("nnz".into(), Json::num(report.nnz_b as f64)),
                    ("channels".into(), Json::num(report.channels_b as f64)),
                    ("lexicon".into(), Json::num(report.lexicon_b as f64)),
                ]),
            ),
            (
                "phases".into(),
                Json::Obj(vec![
                    ("co_support".into(), Json::num(report.co_support as f64)),
                    ("resonance".into(), Json::num(report.resonance as f64)),
                    ("conflict".into(), Json::num(report.conflict as f64)),
                    ("energy".into(), Json::num(report.energy)),
                    ("isomorphic".into(), Json::Bool(report.isomorphic)),
                    ("nnz_merged".into(), Json::num(report.nnz_merged as f64)),
                ]),
            ),
            (
                "channels".into(),
                Json::Obj(vec![
                    ("merged".into(), Json::num(report.channels_merged as f64)),
                    ("shared".into(), Json::num(report.channels_shared as f64)),
                    (
                        "annihilated".into(),
                        Json::num(report.channels_annihilated as f64),
                    ),
                    ("window".into(), Json::num(report.window as f64)),
                    ("ticks".into(), Json::num(report.ticks_merged as f64)),
                ]),
            ),
            (
                "lexicon".into(),
                Json::Obj(vec![
                    ("merged".into(), Json::num(report.lexicon_merged as f64)),
                    (
                        "collisions".into(),
                        Json::num(report.lexicon_collisions as f64),
                    ),
                ]),
            ),
            (
                "container".into(),
                Json::Obj(vec![
                    ("version".into(), Json::num(report.container as f64)),
                    ("path".into(), Json::str(&out_path)),
                    ("bytes".into(), Json::num(report.brain_bytes as f64)),
                ]),
            ),
            ("eps".into(), Json::num(eps)),
        ];
        if let Some(srep) = &settle_report {
            j.push((
                "settle".into(),
                Json::Obj(vec![
                    ("ticks_requested".into(), Json::num(srep.ticks_requested as f64)),
                    ("ticks_run".into(), Json::num(srep.ticks_run as f64)),
                    (
                        "moved_per_tick".into(),
                        Json::Arr(
                            srep.moved_per_tick
                                .iter()
                                .map(|m| Json::num(*m as f64))
                                .collect(),
                        ),
                    ),
                    ("theta_shift_total".into(), Json::num(srep.theta_shift_total)),
                    ("channels_before".into(), Json::num(srep.channels_before as f64)),
                    ("channels_after".into(), Json::num(srep.channels_after as f64)),
                    ("channels_grown".into(), Json::num(srep.channels_grown as f64)),
                    ("observed_events".into(), Json::num(srep.observed_events as f64)),
                    ("lattice_flips".into(), Json::num(srep.lattice_flips as f64)),
                    ("stationary".into(), Json::Bool(srep.stationary)),
                ]),
            ));
        }
        if let Some((q, text)) = &answer {
            j.push((
                "ask".into(),
                Json::Obj(vec![
                    ("question".into(), Json::str(q)),
                    ("answer".into(), Json::str(text)),
                ]),
            ));
        }
        println!("{}", Json::Obj(j).to_string());
        return 0;
    }

    println!("POLER Quantum Core — Merge (RQ20): merged = A ⊗_ε B — рождение единого разума");
    println!(
        "мозг A  : {path_a} (d_pol={}, носитель {} дуг, русла {}, слов {})",
        report.d_pol, report.nnz_a, report.channels_a, report.lexicon_a
    );
    println!(
        "мозг B  : {path_b} (носитель {} дуг, русла {}, слов {})",
        report.nnz_b, report.channels_b, report.lexicon_b
    );
    println!(
        "фазы    : c = a ⊗_ε b — резонанс {}, конфликт {} (аннигиляция в открытый вопрос), \
         прозрачный проход {} (уникальное каждого)",
        report.resonance, report.conflict, report.nnz_merged - report.resonance
    );
    println!(
        "          носитель слитого мозга: {} дуг — объединение знаний",
        report.nnz_merged
    );
    println!(
        "изоморфизм: E = co/min(nnz) = {:.3} {} {} — {}",
        report.energy,
        if report.energy >= eps { "≥" } else { "<" },
        eps,
        if report.isomorphic {
            "мозги структурно изоморфны (глубокая общность)"
        } else {
            "домены почти ортогональны (слияние всё равно объединяет)"
        }
    );
    println!(
        "русла   : J = Π_Λ(J_A + J_B) — {} каналов (общих пар {}, встречной циркуляции погашено {}), \
         кососимметричность сохранена",
        report.channels_merged, report.channels_shared, report.channels_annihilated
    );
    println!(
        "лексикон: {} слов (коллизий доминант {} — победа мозга A), окно W={}, тактов {}",
        report.lexicon_merged, report.lexicon_collisions, report.window, report.ticks_merged
    );
    if let Some(srep) = &settle_report {
        println!(
            "сеттлинг: {} такта Π_Λ(e^{{Δt·J}} p) — флипы {:?}, русла {} → {} (проросло {:+}), \
             свидетельств волны {}, стационар ĤΨ = 0: {}",
            srep.ticks_run,
            srep.moved_per_tick,
            srep.channels_before,
            srep.channels_after,
            srep.channels_grown,
            srep.observed_events,
            if srep.stationary { "достигнут" } else { "не достигнут (потолок тактов)" }
        );
    }
    println!(
        "сохранено: {out_path} → v{} контейнер ({} Б)",
        report.container, report.brain_bytes
    );
    if let Some((q, text)) = &answer {
        println!("\nвопрос  : {q}");
        println!(
            "ответ   : {}",
            if text.is_empty() { "(молчание)" } else { text }
        );
    }
    println!(
        "\nпроверка: pqc ask \"вопрос на стыке дисциплин\" --brain {out_path}"
    );
    0
}

fn arch_usage_err(msg: &str) -> i32 {
    eprintln!("pqc archetype: {msg}");
    eprintln!("см. pqc --help: ARCHETYPE OPTIONS");
    2
}

/// `pqc generate` / `pqc step`: генерация из промпта (без записи памяти).
fn cmd_generate(args: &[String], step_mode: bool) -> i32 {
    let mut cfg = GenConfig::default();
    if step_mode {
        cfg.max_tokens = 1;
        cfg.think = 1;
    }
    if let Err(e) = parse_gen_args(args, &mut cfg) {
        return gen_usage_err(&e);
    }
    let prompt = if cfg.prompt.is_empty() {
        cfg.question.clone().unwrap_or_default()
    } else {
        cfg.prompt.clone()
    };
    let mut source = match load_gen_engine(&cfg) {
        Ok(s) => s,
        Err(e) => return gen_usage_err(&e),
    };
    let engine = match &mut source {
        BrainSource::File { engine, .. } => engine,
        BrainSource::Inline(engine) => engine,
    };
    let gcfg = GeneratorConfig {
        think_steps: cfg.think,
        max_tokens: if step_mode { 1 } else { cfg.max_tokens },
        window: cfg.window,
        seed: cfg.seed,
        free: cfg.free,
        morphemes: cfg.morphemes,
        repeat_veto: cfg.repeat_veto,
        bridge: cfg.bridge,
        bridge_eps: cfg.bridge_eps,
        syntax: cfg.syntax,
        focus_radius: cfg.focus_radius,
        reinforce: cfg.reinforce,
        autobiographical: false,
    };
    let mut gen = match L5Generator::new(engine, gcfg) {
        Ok(g) => g,
        Err(e) => return gen_usage_err(&e.to_string()),
    };
    let rep = match gen.generate(&prompt) {
        Ok(r) => r,
        Err(e) => return gen_usage_err(&e.to_string()),
    };
    if cfg.json {
        println!(
            "{}",
            Json::Obj(generation_json_pairs(&rep, None)).to_string()
        );
    } else {
        let kind = if step_mode { "квант" } else { "речь" };
        let src_desc = if cfg.brain.is_some() {
            format!("brain {}", cfg.brain.as_deref().unwrap_or("?"))
        } else {
            "corpus (inline)".to_string()
        };
        println!(
            "POLER Quantum Core — L5 Generator (RQ17): Born-блуждание по руслам J"
        );
        println!("источник : {src_desc}");
        println!(
            "промпт   : «{}» ({} токенов, дуг {}, зажиганий {})",
            if prompt.is_empty() { "— свободная речь" } else { &prompt },
            rep.prompt_tokens,
            rep.prompt_arcs,
            rep.ignited
        );
        println!(
            "мышление : {} шагов, перещёлкиваний {}, θ-сдвиг {:.3}{}",
            cfg.think,
            rep.think_moved,
            rep.think_theta_shift,
            if rep.auto_free { " [ворота сняты: нулевая кинетика]" } else { "" }
        );
        if cfg.bridge {
            if rep.bridge_resonant {
                println!(
                    "мост ⊗_ε  : энергия {:.3} ≥ {} — резонанс: пересечение {} дуг \
                     (согласие {}, конфликт {})",
                    rep.bridge_energy, cfg.bridge_eps, rep.bridge_co,
                    rep.bridge_resonance, rep.bridge_conflict
                );
            } else {
                println!(
                    "мост ⊗_ε  : энергия {:.3} < {} — архетипы ортогональны, \
                     мост молчит (нет ложных ассоциаций)",
                    rep.bridge_energy, cfg.bridge_eps
                );
            }
        }
        if !step_mode {
            println!("{kind}:");
        }
        print_generation_human(&rep, step_mode);
    }
    0
}

/// `pqc ask` / `pqc chat`: диалог с памятью — вопрос и ответ
/// перещёлкивают фазы решётки через born-шаг, мозг дописывается.
fn cmd_ask(args: &[String], chat_mode: bool) -> i32 {
    let mut cfg = GenConfig::default();
    if let Err(e) = parse_gen_args(args, &mut cfg) {
        return gen_usage_err(&e);
    }
    if cfg.brain.is_none() {
        return gen_usage_err(
            "ask/chat требует --brain F: память диалога живёт в контейнере-мозге",
        );
    }
    let question = match cfg.question.clone().or(Some(cfg.prompt.clone())) {
        Some(q) if !q.is_empty() => q,
        _ if !chat_mode => {
            return gen_usage_err("нужен вопрос: pqc ask \"что такое энтропия?\" --brain F")
        }
        _ => String::new(),
    };

    let (path, mut engine) = match load_gen_engine(&cfg) {
        Ok(BrainSource::File { path, engine }) => (path, engine),
        Ok(_) => return gen_usage_err("ask/chat: только --brain"),
        Err(e) => return gen_usage_err(&e),
    };

    // RQ23: автобиографическая память — имя собеседника и нить
    // разговора переживают рестарт (контекст-рефлекс W из секции REFL
    // v5-контейнера; кольцо гироскопа уже восстановлено в resume).
    let refl = engine.reflex().clone();
    if let Some(name) = &cfg.name {
        engine.reflex_mut().set_interlocutor(name);
    }
    let autobiographical = !cfg.fresh;
    let mut reflex_json = None;
    let mut autobio_line = String::new();
    if !refl.is_empty() {
        let thread: Vec<&str> = refl.thread_words(12, |c| engine.lexicon_token(c));
        if cfg.json {
            reflex_json = Some(Json::Obj(vec![
                ("interlocutor".into(), Json::str(refl.interlocutor())),
                ("turns".into(), Json::num(refl.turns() as f64)),
                ("trail_events".into(), Json::num(refl.trail().len() as f64)),
                (
                    "thread".into(),
                    Json::Arr(thread.iter().map(|&t| Json::str(t)).collect()),
                ),
            ]));
        } else {
            let name_disp = if refl.interlocutor().is_empty() {
                "аноним"
            } else {
                refl.interlocutor()
            };
            let thread_disp = if thread.is_empty() {
                "(координаты без слов лексикона)".to_string()
            } else {
                thread.join(" ")
            };
            autobio_line = format!(
                "автобио  : собеседник {name_disp}, {} реплик — помню нить: {thread_disp} …",
                refl.turns()
            );
        }
    }

    if !cfg.json {
        println!(
            "POLER Quantum Core — L5 Dialog (RQ17): диалог с памятью, H^Ψ = 0"
        );
        println!("мозг     : {path} (d_pol={}, каналы J={}, лексикон {} слов)", engine.d_pol(), engine.channel_count(), engine.lexicon_len());
        if !autobio_line.is_empty() {
            println!("{autobio_line}");
        }
        if chat_mode {
            println!("режим    : REPL — вводите вопросы, Ctrl-D завершает и сохраняет");
        }
    }

    // Цикл вопросов: одношаговый ask или многошаговый chat.
    let mut current_question = question;
    let mut first = true;
    loop {
        if chat_mode && current_question.is_empty() {
            print!("вопрос > ");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
                break; // EOF — выход с сохранением
            }
            current_question = line.trim().to_string();
            if current_question.is_empty() {
                continue;
            }
        }

        let gcfg = GeneratorConfig {
            think_steps: cfg.think,
            max_tokens: cfg.max_tokens,
            window: cfg.window,
            seed: cfg.seed,
            free: cfg.free,
            morphemes: cfg.morphemes,
            repeat_veto: cfg.repeat_veto,
            bridge: cfg.bridge,
            bridge_eps: cfg.bridge_eps,
            syntax: cfg.syntax,
            focus_radius: cfg.focus_radius,
            reinforce: cfg.reinforce,
            autobiographical,
        };
        let report = {
            let mut gen = match L5Generator::new(&mut engine, gcfg) {
                Ok(g) => g,
                Err(e) => return gen_usage_err(&e.to_string()),
            };
            match gen.generate(&current_question) {
                Ok(r) => r,
                Err(e) => return gen_usage_err(&e.to_string()),
            }
        };

        // Память диалога: вопрос + ответ перещёлкивают фазы born-шагом —
        // модель обогащается на лету (мнение кристаллизуется, русла
        // вопроса↔ответа копятся). RQ23: реплика уходит и в
        // контекст-рефлекс W — нить диалога переживает рестарт.
        let mut memory_json = None;
        let mut learned_desc = String::new();
        if cfg.learn {
            // RQ23: след W — вопрос + эмиссии ответа с Born-полярностями.
            let answer_events: Vec<(u32, i8)> = report
                .steps
                .iter()
                .map(|s| (s.coord, if s.born_bit { -1 } else { 1 }))
                .collect();
            engine.observe_dialog_turn(&current_question, &answer_events);
            let dialog = format!("{current_question} {}", report.text);
            match engine.ingest(&dialog, 1) {
                Ok(rep) => {
                    learned_desc = format!(
                        "+{} дуг born, +{} каналов, лексикон {} слов",
                        rep.moved,
                        rep.channels,
                        engine.lexicon_len()
                    );
                    memory_json = Some(Json::Obj(vec![
                        ("ingested".into(), Json::Bool(true)),
                        ("born_moved".into(), Json::num(rep.moved as f64)),
                        ("channels".into(), Json::num(rep.channels as f64)),
                        ("lexicon".into(), Json::num(engine.lexicon_len() as f64)),
                        (
                            "reflex_turns".into(),
                            Json::num(engine.reflex().turns() as f64),
                        ),
                    ]));
                }
                Err(e) => {
                    eprintln!("pqc ask: память диалога: {e}");
                }
            }
        }

        if cfg.json {
            let mut j = if first {
                let mut brain = vec![
                    ("path".into(), Json::str(&path)),
                    ("d_pol".into(), Json::num(engine.d_pol() as f64)),
                    ("channels".into(), Json::num(engine.channel_count() as f64)),
                    ("lexicon".into(), Json::num(engine.lexicon_len() as f64)),
                ];
                if let Some(rj) = &reflex_json {
                    brain.push(("reflex".into(), rj.clone()));
                }
                vec![
                    ("brain".into(), Json::Obj(brain)),
                    ("question".into(), Json::str(&current_question)),
                ]
            } else {
                vec![("question".into(), Json::str(&current_question))]
            };
            j.extend(generation_json_pairs(&report, memory_json.as_ref()));
            println!("{}", Json::Obj(j).to_string());
        } else {
            println!(
                "\nвопрос : {}",
                if current_question.is_empty() { "— свободная речь" } else { &current_question }
            );
            println!(
                "ответ  : {}",
                if report.text.is_empty() {
                    if engine.lexicon_len() == 0 {
                        "(молчание: мозг без лексикона — перезапустите обучение v0.7.0+)"
                    } else {
                        "(молчание: живых русел из вопроса нет)"
                    }
                } else {
                    &report.text
                }
            );
            if cfg.learn && !learned_desc.is_empty() {
                println!("память : {learned_desc}");
            }
        }

        first = false;
        if !chat_mode {
            break;
        }
        current_question = String::new();
    }

    // Обратная запись мозга: v5 при живом рефлексе (фазы + русла +
    // лексикон + контекст-рефлекс W), иначе v4 (RQ23).
    if cfg.learn {
        match engine.checkpoint() {
            Ok(bytes) => match std::fs::write(&path, &bytes) {
                Ok(()) => {
                    if !cfg.json {
                        let has_reflex = !engine.reflex().is_empty()
                            && engine.reflex().to_data(engine.d_pol()).is_some();
                        if has_reflex {
                            println!(
                                "\nсохранено: {path} → v5 контейнер ({} Б: фазы + русла J + \
                                 лексикон {} слов + контекст-рефлекс W: {} реплик)",
                                bytes.len(),
                                engine.lexicon_len(),
                                engine.reflex().turns()
                            );
                        } else {
                            println!(
                                "\nсохранено: {path} → v4 контейнер ({} Б: фазы + русла J + лексикон {} слов)",
                                bytes.len(),
                                engine.lexicon_len()
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("pqc ask: запись мозга {path}: {e}");
                    return 1;
                }
            },
            Err(e) => {
                eprintln!("pqc ask: чекпоинт: {e}");
                return 1;
            }
        }
    }
    0
}

// ==========================================================================
// qpc — POLER Quantum PC (идеальные кубиты, v0.44.0)
// ==========================================================================

fn arg_num<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> Result<T, String> {
    let mut it = args.iter().enumerate();
    while let Some((i, a)) = it.next() {
        if a == flag {
            let v = args.get(i + 1).ok_or_else(|| format!("missing value for {flag}"))?;
            return v
                .parse::<T>()
                .map_err(|_| format!("bad value for {flag}: {v}"));
        }
    }
    Ok(default)
}

fn arg_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn bits_string(outcome: u64, n: usize) -> String {
    let mut s = String::with_capacity(n + 2);
    s.push('|');
    for q in (0..n).rev() {
        s.push(if outcome >> q & 1 == 1 { '1' } else { '0' });
    }
    s.push('⟩');
    s
}

/// `pqc qc <file.qc> [--shots N] [--seed S] [--top K] [--amplitudes]
///                 [--exact] [--json] [--probs]`
fn cmd_qc(args: &[String]) -> i32 {
    let path = match args.first() {
        Some(p) if !p.starts_with("--") => p.clone(),
        _ => {
            eprintln!("pqc qc: circuit file required\n\n{USAGE}");
            return 2;
        }
    };
    let shots: u64 = match arg_num(args, "--shots", 1024u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qc: {e}");
            return 2;
        }
    };
    let seed: u64 = match arg_num(args, "--seed", 42u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qc: {e}");
            return 2;
        }
    };
    let top: usize = match arg_num(args, "--top", 20usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qc: {e}");
            return 2;
        }
    };
    let want_amplitudes = arg_flag(args, "--amplitudes");
    let want_exact = arg_flag(args, "--exact");
    let want_probs = arg_flag(args, "--probs");
    let as_json = arg_flag(args, "--json");

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("pqc qc: cannot read {path}: {e}");
            return 1;
        }
    };
    let circuit = match pqc::qpc::Circuit::parse(&text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pqc qc: {e}");
            return 1;
        }
    };

    if want_exact {
        match pqc::exact::run_exact(&circuit) {
            Ok(rep) => {
                if as_json {
                    let mut obj: Vec<(String, pqc::Json)> = vec![
                        ("engine".into(), pqc::Json::str("qpc-exact")),
                        ("n_qubits".into(), pqc::Json::num(rep.n_qubits as f64)),
                    ];
                    obj.push(("probabilities".into(), pqc::Json::num_arr(rep.probs_f64.clone())));
                    obj.push(("norm_residual".into(), pqc::Json::num(rep.norm_residual)));
                    let comps: Vec<pqc::Json> = rep
                        .exact_probs
                        .iter()
                        .map(|p| pqc::Json::Arr(vec![pqc::Json::num(p.a as f64), pqc::Json::num(p.b as f64), pqc::Json::num(p.k as f64)]))
                        .collect();
                    obj.push(("exact_probs".into(), pqc::Json::Arr(comps)));
                    let amps: Vec<pqc::Json> = rep
                        .amplitudes
                        .iter()
                        .map(|z| {
                            pqc::Json::Arr(vec![
                                pqc::Json::num(z.a as f64),
                                pqc::Json::num(z.b as f64),
                                pqc::Json::num(z.c as f64),
                                pqc::Json::num(z.d as f64),
                                pqc::Json::num(z.k as f64),
                            ])
                        })
                        .collect();
                    obj.push(("exact_amplitudes".into(), pqc::Json::Arr(amps)));
                    let strs: Vec<pqc::Json> = rep.exact_probs.iter().map(|p| pqc::Json::str(format!("{p}"))).collect();
                    obj.push(("exact_probs_pretty".into(), pqc::Json::Arr(strs)));
                    println!("{}", pqc::Json::Obj(obj).to_string());
                    return 0;
                }
                println!("POLER Quantum PC — EXACT RING Z[1/sqrt(2), i]");
                println!("file: {path}");
                println!("qubits: {}, norm residual: {:.2e}", rep.n_qubits, rep.norm_residual);
                println!();
                for (i, (p, ex)) in rep.probs_f64.iter().zip(rep.exact_probs.iter()).enumerate() {
                    if *p > 0.0 || want_probs {
                        println!("  {}  {:.6}   = {}", bits_string(i as u64, rep.n_qubits), p, ex);
                    }
                }
                if want_amplitudes || rep.n_qubits <= 6 {
                    println!("\namplitudes (exact):");
                    for (i, z) in rep.amplitudes.iter().enumerate() {
                        let (re, im) = z.to_f64();
                        println!("  {}  ({:+.6} {:+.6}i)  = {}", bits_string(i as u64, rep.n_qubits), re, im, z.display());
                    }
                }
                return 0;
            }
            Err(e) => {
                eprintln!("pqc qc --exact: {e}");
                return 1;
            }
        }
    }

    match pqc::qpc::run(&circuit, shots, seed) {
        Ok(rep) => {
            if as_json {
                let counts: Vec<pqc::Json> = rep
                    .counts
                    .iter()
                    .map(|(o, c)| pqc::Json::Arr(vec![pqc::Json::num(*o as f64), pqc::Json::num(*c as f64)]))
                    .collect();
                let obj = pqc::Json::Obj(vec![
                    ("engine".into(), pqc::Json::str("qpc")),
                    ("n_qubits".into(), pqc::Json::num(rep.n_qubits as f64)),
                    ("gate_count".into(), pqc::Json::num(rep.gate_count as f64)),
                    ("per_shot".into(), pqc::Json::Bool(rep.per_shot)),
                    ("shots".into(), pqc::Json::num(rep.shots as f64)),
                    ("norm".into(), pqc::Json::num(rep.norm)),
                    ("entropy_bits".into(), pqc::Json::num(rep.entropy_bits)),
                    ("landauer_j".into(), pqc::Json::num(rep.landauer_j)),
                    ("probabilities".into(), pqc::Json::num_arr(rep.probabilities.clone())),
                    ("marginals".into(), pqc::Json::num_arr(rep.marginals.clone())),
                    ("counts".into(), pqc::Json::Arr(counts)),
                ]);
                println!("{}", obj.to_string());
                return 0;
            }
            println!("POLER Quantum PC — ideal qubit substrate");
            println!("file: {path}");
            println!(
                "qubits: {}, gates: {}, shots: {}, mode: {}",
                rep.n_qubits,
                rep.gate_count,
                rep.shots,
                if rep.per_shot { "per-shot collapse" } else { "statevector" }
            );
            println!("norm: {:.16}", rep.norm);
            println!(
                "entropy: {:.4} bits | Landauer floor @300K: {:.3e} J",
                rep.entropy_bits, rep.landauer_j
            );
            if !rep.counts.is_empty() {
                println!("\ntop outcomes:");
                for (o, c) in rep.counts.iter().take(top) {
                    println!(
                        "  {}  {:>8}  p̂ = {:.4}",
                        bits_string(*o, rep.n_qubits),
                        c,
                        *c as f64 / rep.shots as f64
                    );
                }
            }
            if want_probs {
                println!("\nBorn distribution |<x|psi>|^2:");
                for (i, p) in rep.probabilities.iter().enumerate() {
                    if *p > 1e-12 {
                        println!("  {}  {:.6}", bits_string(i as u64, rep.n_qubits), p);
                    }
                }
            }
            if want_amplitudes && rep.n_qubits <= 20 {
                println!("\namplitudes:");
                for (i, a) in rep.final_state.amplitudes().iter().enumerate() {
                    if a.norm_sq() > 1e-24 {
                        println!("  {}  ({:+.6} {:+.6}i)", bits_string(i as u64, rep.n_qubits), a.re, a.im);
                    }
                }
            }
            println!("\nmarginals P(b_q = 1):");
            for (q, m) in rep.marginals.iter().enumerate() {
                println!("  q{q}: {m:.6}");
            }
            0
        }
        Err(e) => {
            eprintln!("pqc qc: {e}");
            1
        }
    }
}

/// `pqc algo <bell|ghz|qft|iqft|grover|bv|dj> [options]`
fn cmd_algo(args: &[String]) -> i32 {
    let name = match args.first() {
        Some(n) if !n.starts_with("--") => n.as_str(),
        _ => {
            eprintln!("pqc algo: algorithm name required (bell|ghz|qft|iqft|grover|bv|dj|period|teleport)\n\n{USAGE}");
            return 2;
        }
    };
    let n: usize = match arg_num(args, "--n", 4usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let shots: u64 = match arg_num(args, "--shots", 1024u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let seed: u64 = match arg_num(args, "--seed", 42u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let as_json = arg_flag(args, "--json");
    let want_probs = arg_flag(args, "--probs");
    let secret: u64 = match arg_num(args, "--secret", 0b1011u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let marks_str: String = match arg_num(args, "--marks", "22".to_string()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let marks: Vec<usize> = match marks_str
        .split(',')
        .map(|t| t.trim().parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(_) => {
            eprintln!("pqc algo: bad --marks \"{marks_str}\" (expected comma-separated integers)");
            return 2;
        }
    };
    // period: параметры гребёнки (теоретико-числовой субстрат).
    let period: usize = match arg_num(args, "--period", 0usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let offset: usize = match arg_num(args, "--offset", 0usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };

    // Цикл P: единый реестр схем (та же точка правды, что у shell-команды
    // `quantum` и MCP-инструмента `poler_quantum`).
    use pqc::algorithms as alg;
    let theta: f64 = match arg_num(args, "--theta", 0.7f64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 2;
        }
    };
    let params = alg::AlgoParams {
        n,
        marks,
        secret,
        period,
        offset,
        theta,
    };
    let circuit_result = alg::build(name, &params);
    let (circuit, iterations) = match circuit_result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc algo: {e}");
            return 1;
        }
    };

    match pqc::qpc::run(&circuit, shots, seed) {
        Ok(rep) => {
            if as_json {
                let counts: Vec<pqc::Json> = rep
                    .counts
                    .iter()
                    .map(|(o, c)| pqc::Json::Arr(vec![pqc::Json::num(*o as f64), pqc::Json::num(*c as f64)]))
                    .collect();
            let mut obj_fields = vec![
                ("engine".into(), pqc::Json::str("qpc-algo")),
                ("algorithm".into(), pqc::Json::str(name)),
                ("n_qubits".into(), pqc::Json::num(rep.n_qubits as f64)),
                ("gate_count".into(), pqc::Json::num(rep.gate_count as f64)),
                ("iterations".into(), pqc::Json::num(iterations as f64)),
                ("shots".into(), pqc::Json::num(rep.shots as f64)),
                ("norm".into(), pqc::Json::num(rep.norm)),
                ("entropy_bits".into(), pqc::Json::num(rep.entropy_bits)),
                ("landauer_j".into(), pqc::Json::num(rep.landauer_j)),
                ("probabilities".into(), pqc::Json::num_arr(rep.probabilities.clone())),
                ("marginals".into(), pqc::Json::num_arr(rep.marginals.clone())),
                ("counts".into(), pqc::Json::Arr(counts)),
            ];
            if name == "period" {
                // Пики → восстановление периода (min-q правило целостности).
                let max_cnt = rep
                    .counts
                    .iter()
                    .map(|(_, c)| *c)
                    .max()
                    .unwrap_or(0);
                let thr = ((max_cnt as f64) * 0.3) as u64;
                let peaks: Vec<usize> = rep
                    .counts
                    .iter()
                    .filter(|&&(o, cnt)| o != 0 && cnt >= thr.max(6))
                    .map(|&(o, _)| o as usize)
                    .collect();
                let recovered = pqc::algorithms::recover_period(&peaks, rep.n_qubits);
                let candidates: Vec<pqc::Json> = peaks
                    .iter()
                    .map(|&k| {
                        let cs = pqc::algorithms::period_candidates(k, rep.n_qubits);
                        pqc::Json::Arr(vec![
                            pqc::Json::num(k as f64),
                            pqc::Json::Arr(cs.into_iter().map(|q| pqc::Json::num(q as f64)).collect()),
                        ])
                    })
                    .collect();
                obj_fields.push(("peaks".into(), pqc::Json::Arr(
                    peaks.iter().map(|&k| pqc::Json::num(k as f64)).collect(),
                )));
                obj_fields.push(("candidates".into(), pqc::Json::Arr(candidates)));
                obj_fields.push((
                    "recovered_period".into(),
                    match recovered {
                        Some(r) => pqc::Json::num(r as f64),
                        None => pqc::Json::str("unresolved"),
                    },
                ));
                obj_fields.push(("true_period".into(), pqc::Json::num(period as f64)));
            }
            let obj = pqc::Json::Obj(obj_fields);
            println!("{}", obj.to_string());
            return 0;
        }
            println!("POLER Quantum PC — algorithm: {}", name);
            println!(
                "qubits: {}, gates: {}, iterations: {}, shots: {}",
                rep.n_qubits, rep.gate_count, iterations, rep.shots
            );
            println!("entropy: {:.4} bits", rep.entropy_bits);
            if !rep.counts.is_empty() {
                println!("\ntop outcomes:");
                for (o, c) in rep.counts.iter().take(12) {
                    println!(
                        "  {}  {:>8}  p̂ = {:.4}",
                        bits_string(*o, rep.n_qubits),
                        c,
                        *c as f64 / rep.shots as f64
                    );
                }
            }
            if want_probs {
                println!("\nBorn distribution:");
                for (i, p) in rep.probabilities.iter().enumerate() {
                    if *p > 1e-9 {
                        println!("  {}  {:.6}", bits_string(i as u64, rep.n_qubits), p);
                    }
                }
            }
            if name == "period" && !rep.counts.is_empty() {
                let max_cnt = rep.counts.iter().map(|(_, c)| *c).max().unwrap_or(0);
                let thr = ((max_cnt as f64) * 0.3) as u64;
                let peaks: Vec<usize> = rep
                    .counts
                    .iter()
                    .filter(|&&(o, cnt)| o != 0 && cnt >= thr.max(6))
                    .map(|&(o, _)| o as usize)
                    .collect();
                println!("\nperiod-finding report:");
                println!("  peaks (main lobes): {peaks:?}");
                for &k in peaks.iter().take(8) {
                    let cs = pqc::algorithms::period_candidates(k, rep.n_qubits);
                    println!("  k = {k:>6}  convergent candidates: {cs:?}");
                }
                match pqc::algorithms::recover_period(&peaks, rep.n_qubits) {
                    Some(r) => {
                        println!("  recovered period: r = {r} (true {period})");
                        if r == period {
                            println!("  ✅ MATCH");
                        } else {
                            println!("  ❌ MISMATCH");
                        }
                    }
                    None => println!("  recovered period: unresolved"),
                }
                println!(
                    "  сертификация a^r ≡ 1 (mod N) — SMT: tools/verifiers/verify_number_theory_smt.py"
                );
            }
            if name == "teleport" {
                // Цикл P: канал под контролем фиделити.
                let want_exact = arg_flag(args, "--exact");
                let tr = if want_exact {
                    alg::run_teleport_exact()
                } else {
                    alg::run_teleport(theta)
                };
                match tr {
                    Ok(t) => {
                        println!("\nteleport report:");
                        println!("  канал: 6 Клиффорд-вентилей, когерентные коррекции");
                        if want_exact {
                            println!("  режим: ТОЧНО — кольцо Z[1/sqrt(2), i], структурное равенство");
                        }
                        println!(
                            "  препарат q0: (α={:+.6}, β={:+.6})  Ry({:.4})|0⟩",
                            t.psi_in[0].0, t.psi_in[1].0, t.theta
                        );
                        println!(
                            "  выход   q2: (α={:+.6}, β={:+.6})",
                            t.psi_out[0].0, t.psi_out[1].0
                        );
                        println!(
                            "  фиделити |⟨ψ_in|ψ_out⟩|² = {}",
                            if t.exact { "1 (точно)".to_string() } else { format!("{:.15}", t.fidelity) }
                        );
                        println!(
                            "  расхождение веток: {:.3e}{}",
                            t.branch_deviation,
                            if t.branch_deviation < 1e-12 { "  ✓ канал идеален" } else { "" }
                        );
                        println!(
                            "  Блох in : (x={:+.4}, y={:+.4}, z={:+.4})",
                            t.bloch_in[0], t.bloch_in[1], t.bloch_in[2]
                        );
                        println!(
                            "  Блох out: (x={:+.4}, y={:+.4}, z={:+.4})",
                            t.bloch_out[0], t.bloch_out[1], t.bloch_out[2]
                        );
                    }
                    Err(e) => println!("  teleport report: {e}"),
                }
            }
            0
        }
        Err(e) => {
            eprintln!("pqc algo: {e}");
            1
        }
    }
}

/// Позиционные аргументы `pqc verify` (значения флагов вроде `--n 3`
/// пропускаются вместе с самим флагом).
fn verify_positionals(args: &[String]) -> impl Iterator<Item = &String> {
    let mut skip_next = false;
    args.iter().filter(move |a| {
        if skip_next {
            skip_next = false;
            return false;
        }
        if a.starts_with("--") {
            skip_next = *a == "--n";
            return false;
        }
        true
    })
}

/// `pqc verify <unitary|equiv|teleport> [opts]` — цикл P: формальный
/// верификатор квантовых схем.
///
/// Три слоя в духе SMT: структурный линт → фальсификация на случайных
/// состояниях → полное доказательство (кольцо Z[1/sqrt(2), i] — точно;
/// f64 — с допуском). Вердикты градуированы честно: ProvedExact /
/// VerifiedNumeric / VerifiedSampling / Refuted.
fn cmd_verify(args: &[String]) -> i32 {
    let mode = match args.first() {
        Some(m) if !m.starts_with("--") => m.as_str(),
        _ => {
            eprintln!(
                "pqc verify: mode required (unitary <algo> | equiv <A> <B> | teleport)\n\n{USAGE}"
            );
            return 2;
        }
    };
    let as_json = arg_flag(args, "--json");
    let n: usize = match arg_num(args, "--n", 4usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc verify: {e}");
            return 2;
        }
    };

    use pqc::algorithms as alg;
    use pqc::verify::{self as ver, Verdict};

    let build_algo = |name: &str| -> Result<pqc::qpc::Circuit, String> {
        let p = alg::AlgoParams {
            n,
            marks: vec![22 % (1usize << n.max(1))],
            secret: 0b1011,
            period: 0,
            offset: 0,
            theta: 0.7,
        };
        alg::build(name, &p).map(|(c, _)| c).map_err(|e| format!("{e:?}"))
    };

    let report = match mode {
        "unitary" => {
            // Второй позиционный аргумент (первый — сам режим).
            let algo = verify_positionals(args).nth(1).cloned();
            let algo = match algo {
                Some(a) => a,
                None => {
                    eprintln!("pqc verify unitary: algorithm name required (bell|ghz|qft|…)");
                    return 2;
                }
            };
            let circuit = match build_algo(&algo) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("pqc verify: {e}");
                    return 1;
                }
            };
            ver::verify_unitary(&circuit).map_err(|e| format!("{e:?}"))
        }
        "equiv" => {
            // Позиционные после режима: ровно два алгоритма.
            let names: Vec<String> = verify_positionals(args).skip(1).cloned().collect();
            if names.len() != 2 {
                eprintln!("pqc verify equiv: два алгоритма обязательны: equiv <A> <B> [--n N] [--phase]");
                return 2;
            }
            let (a, b) = match (build_algo(&names[0]), build_algo(&names[1])) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("pqc verify: {e}");
                    return 1;
                }
            };
            let up_to_phase = arg_flag(args, "--phase");
            ver::verify_equivalence(&a, &b, up_to_phase).map_err(|e| format!("{e:?}"))
        }
        "teleport" => ver::verify_teleport_channel().map_err(|e| format!("{e:?}")),
        other => {
            eprintln!("pqc verify: неизвестный режим `{other}` (unitary|equiv|teleport)");
            return 2;
        }
    };
    let report = match report {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc verify: {e}");
            return 1;
        }
    };

    if as_json {
        let verdict = match report.verdict {
            Verdict::ProvedExact => pqc::Json::str("proved_exact"),
            Verdict::VerifiedNumeric(tol) => pqc::Json::Arr(vec![
                pqc::Json::str("verified_numeric"),
                pqc::Json::num(tol),
            ]),
            Verdict::VerifiedSampling(tol) => pqc::Json::Arr(vec![
                pqc::Json::str("verified_sampling"),
                pqc::Json::num(tol),
            ]),
            Verdict::Refuted(d) => {
                pqc::Json::Arr(vec![pqc::Json::str("refuted"), pqc::Json::num(d)])
            }
        };
        let obj = pqc::Json::Obj(vec![
            ("engine".into(), pqc::Json::str("pqc-verify")),
            ("property".into(), pqc::Json::str(report.property)),
            ("subject".into(), pqc::Json::str(&report.subject)),
            ("n_qubits".into(), pqc::Json::num(report.n_qubits as f64)),
            ("dim".into(), pqc::Json::num(report.dim as f64)),
            ("gate_count".into(), pqc::Json::num(report.gate_count as f64)),
            ("method".into(), pqc::Json::str(report.method)),
            ("verdict".into(), verdict),
            ("max_deviation".into(), pqc::Json::num(report.max_deviation)),
            (
                "checks".into(),
                pqc::Json::Arr(
                    report
                        .checks
                        .iter()
                        .map(|(name, ok, dev)| {
                            pqc::Json::Arr(vec![
                                pqc::Json::str(*name),
                                pqc::Json::num(if *ok { 1.0 } else { 0.0 }),
                                pqc::Json::num(*dev),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "notes".into(),
                pqc::Json::Arr(
                    report
                        .notes
                        .iter()
                        .map(|s| pqc::Json::str(s))
                        .collect(),
                ),
            ),
        ]);
        println!("{}", obj.to_string());
        return match report.verdict {
            Verdict::Refuted(_) => 1,
            _ => 0,
        };
    }

    println!("POLER Quantum PC — формальная верификация (цикл P)");
    println!("свойство: {}", report.property);
    println!("субъект:  {}", report.subject);
    println!(
        "схема:    {} кубитов, {} унитарных операций, dim {}",
        report.n_qubits, report.gate_count, report.dim
    );
    println!("метод:    {}", report.method);
    println!("вердикт:  {}", report.verdict_line());
    for (name, ok, dev) in &report.checks {
        println!("  [{}] {:<24} невязка {:.3e}", if *ok { "✓" } else { "✗" }, name, dev);
    }
    for note in &report.notes {
        println!("  · {note}");
    }
    match report.verdict {
        Verdict::Refuted(_) => 1,
        _ => 0,
    }
}

/// `pqc stab [--n N] [--preset ghz|cluster|random] [--depth D] [--shots S]
///          [--seed S] [--json]`
///
/// Gottesman–Knill: стабилизаторный симулятор за пределами 26 кубитов
/// (память n·n/4 байт вместо 2^n·16; GHZ-2048 и случайные Клиффорды
/// на 1024 кубитах — за секунды). Идеальные кубиты: исходы точны по
/// построению, ноль ошибок гейтов/зчитывания/декогеренции.
fn cmd_stab(args: &[String]) -> i32 {
    let n: usize = match arg_num(args, "--n", 64usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc stab: {e}");
            return 2;
        }
    };
    let preset = args
        .iter()
        .position(|a| a == "--preset")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "ghz".to_string());
    let depth: usize = match arg_num(args, "--depth", 20usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc stab: {e}");
            return 2;
        }
    };
    let shots: u64 = match arg_num(args, "--shots", 32u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc stab: {e}");
            return 2;
        }
    };
    let seed: u64 = match arg_num(args, "--seed", 42u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc stab: {e}");
            return 2;
        }
    };
    let as_json = arg_flag(args, "--json");
    // --expect "0,1,3": точное ⟨Z_S⟩ (спектральная теорема: 0 или ±1;
    // "null" = 0 — оператор не в стабилизаторной группе). Можно повторять.
    let expect_sets: Vec<Vec<usize>> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == "--expect")
        .filter_map(|(i, _)| args.get(i + 1))
        .map(|v| {
            v.split(',')
                .filter_map(|t| t.trim().parse::<usize>().ok())
                .collect::<Vec<usize>>()
        })
        .collect();

    use pqc::rng::Rng;
    use pqc::stabilizer::StabilizerState;
    use std::time::Instant;

    let t0 = Instant::now();
    let build = |rng: &mut Rng| -> std::io::Result<StabilizerState> {
        let mut st = StabilizerState::new(n).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}"))
        })?;
        match preset.as_str() {
            "ghz" => {
                st.h(0);
                for q in 1..n {
                    st.cx(0, q);
                }
            }
            "cluster" => {
                // линейный кластер: H везде, CZ по рёбрам
                for q in 0..n {
                    st.h(q);
                }
                for q in 0..n.saturating_sub(1) {
                    st.cz(q, q + 1);
                }
            }
            "random" => {
                for _ in 0..depth * n / 4 {
                    let g = (rng.next_u64() as usize) % 4;
                    let a = rng.next_u64() as usize % n;
                    let b = (a + 1 + rng.next_u64() as usize % 8.min(n - 1)) % n;
                    match g {
                        0 => st.h(a),
                        1 => st.s(a),
                        2 => st.cx(a, b),
                        _ => st.cz(a, b),
                    }
                }
            }
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown preset `{other}` (ghz|cluster|random)"),
                ))
            }
        }
        Ok(st)
    };
    let mut rng = Rng::seed_from_u64(seed);
    let mut st = match build(&mut rng) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc stab: {e}");
            return 2;
        }
    };
    let build_dt = t0.elapsed();

    // Точные ожидания (без измерений): до всяких мутаций measure_all.
    let expects: Vec<(Vec<usize>, Option<f64>)> = expect_sets
        .iter()
        .map(|subset| {
            let mut s = build(&mut Rng::seed_from_u64(seed)).unwrap();
            let v = s.expect_z_product(subset);
            (subset.clone(), v)
        })
        .collect();

    // Прогоны измерений: гистограмма чётности/энтропия исходов.
    let t1 = Instant::now();
    let mut outcome_counts: std::collections::BTreeMap<Vec<u8>, u64> =
        std::collections::BTreeMap::new();
    let mut random_events_total = 0usize;
    let mut first_outcomes: Vec<u8> = Vec::new();
    for shot in 0..shots {
        let mut s = if shot == 0 {
            std::mem::replace(&mut st, build(&mut rng).unwrap())
        } else {
            build(&mut rng).unwrap()
        };
        let (out, rnd) = s.measure_all(&mut rng);
        random_events_total += rnd;
        if shot == 0 {
            first_outcomes = out.clone();
        }
        *outcome_counts.entry(out).or_insert(0) += 1;
    }
    let meas_dt = t1.elapsed();
    let total_dt = t0.elapsed();

    // Энтропия распределения исходов (по битовой строке — грубая,
    // для GHZ она = 1: два исхода).
    let entropy = {
        let tot = shots as f64;
        -outcome_counts
            .values()
            .map(|c| {
                let p = *c as f64 / tot;
                p * p.log2()
            })
            .sum::<f64>()
    };
    let hamming: Vec<f64> = (0..n)
        .map(|q| {
            let ones: u64 = outcome_counts
                .keys()
                .zip(outcome_counts.values())
                .map(|(k, c)| u64::from(k.get(q).copied().unwrap_or(0)) * c)
                .sum();
            ones as f64 / shots as f64
        })
        .collect();

    if as_json {
        let counts_json: Vec<pqc::Json> = outcome_counts
            .iter()
            .map(|(k, c)| {
                let bits: Vec<pqc::Json> =
                    k.iter().map(|b| pqc::Json::num(f64::from(*b))).collect();
                pqc::Json::Arr(vec![
                    pqc::Json::Arr(bits),
                    pqc::Json::num(*c as f64),
                ])
            })
            .collect();
        let expect_json: Vec<pqc::Json> = expects
            .iter()
            .map(|(subset, v)| {
                let bits: Vec<pqc::Json> =
                    subset.iter().map(|q| pqc::Json::num(*q as f64)).collect();
                let val = match v {
                    Some(x) => pqc::Json::num(*x),
                    None => pqc::Json::str("null"),
                };
                pqc::Json::Arr(vec![pqc::Json::Arr(bits), val])
            })
            .collect();
        let obj = pqc::Json::Obj(vec![
            ("engine".into(), pqc::Json::str("stabilizer-gottesman-knill")),
            ("preset".into(), pqc::Json::str(&preset)),
            ("n_qubits".into(), pqc::Json::num(n as f64)),
            ("shots".into(), pqc::Json::num(shots as f64)),
            ("expect_z".into(), pqc::Json::Arr(expect_json)),
            ("random_events_total".into(), pqc::Json::num(random_events_total as f64)),
            ("outcome_entropy_bits".into(), pqc::Json::num(entropy)),
            ("marginals_p1".into(), pqc::Json::num_arr(hamming.clone())),
            ("distinct_outcomes".into(), pqc::Json::num(outcome_counts.len() as f64)),
            ("build_ms".into(), pqc::Json::num(build_dt.as_secs_f64() * 1e3)),
            ("measure_ms".into(), pqc::Json::num(meas_dt.as_secs_f64() * 1e3)),
            ("total_ms".into(), pqc::Json::num(total_dt.as_secs_f64() * 1e3)),
            ("counts".into(), pqc::Json::Arr(counts_json)),
        ]);
        println!("{}", obj.to_string());
        return 0;
    }

    println!("POLER Quantum PC — стабилизаторный движок (Gottesman–Knill)");
    println!("preset: {preset}, qubits: {n}, shots: {shots}");
    println!(
        "память таблицы: ~{} КБ (statevector потребовал бы {} ГБ)",
        n * n / 4 / 1024,
        {
            let bytes = 16u128 << n.min(80);
            (bytes / (1u128 << 30)) as u64
        }
    );
    println!(
        "случайных событий Борна: {} (точная вероятность ½ каждое)",
        random_events_total
    );
    println!("исходов в гистограмме: {}, энтропия: {:.4} бит", outcome_counts.len(), entropy);
    println!(
        "время: построение {:.1} мс + измерения {:.1} мс = {:.1} мс",
        build_dt.as_secs_f64() * 1e3,
        meas_dt.as_secs_f64() * 1e3,
        total_dt.as_secs_f64() * 1e3
    );
    if !expects.is_empty() {
        println!("\nточные ожидания ⟨Z_S⟩ (спектральная теорема: 0 или ±1):");
        for (subset, v) in &expects {
            let vs = match v {
                Some(x) => format!("{x:+.1}"),
                None => "0 (null — вне группы)".to_string(),
            };
            println!("  Z_{{{}}}: {}", subset.iter().map(|q| q.to_string()).collect::<Vec<_>>().join(","), vs);
        }
    }
    if !first_outcomes.is_empty() && n <= 64 {
        let s: String = first_outcomes.iter().map(|b| char::from(b + b'0')).collect();
        println!("первый исход: {s}");
    }
    if preset == "ghz" && !first_outcomes.is_empty() {
        let all_eq = first_outcomes.iter().all(|b| *b == first_outcomes[0]);
        println!("GHZ-корреляция (все биты равны): {}", if all_eq { "✅" } else { "❌" });
    }
    0
}

/// `pqc noise <bell|ghz|grover|bv|qft|dj> [--n N] [--preset P|--p1q V --p2q V
///           --pread V --t1 V --t2 V] [--shots S] [--compare] [--json]`
///
/// «Идеал vs железо»: та же схема на идеальном субстрате и на
/// калиброванном шуме (квантовые траектории: деполяризация, T1/T2,
/// чтение). Пресеты: ideal|ibm-heron|google-willow|noisy-90s.
fn cmd_noise(args: &[String]) -> i32 {
    let name = match args.first() {
        Some(n) if !n.starts_with("--") => n.as_str(),
        _ => {
            eprintln!("pqc noise: algorithm required (bell|ghz|grover|bv|qft|dj)\n\n{USAGE}");
            return 2;
        }
    };
    let n: usize = match arg_num(args, "--n", 6usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc noise: {e}");
            return 2;
        }
    };
    let shots: u64 = match arg_num(args, "--shots", 20_000u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc noise: {e}");
            return 2;
        }
    };
    let seed: u64 = match arg_num(args, "--seed", 42u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc noise: {e}");
            return 2;
        }
    };
    let as_json = arg_flag(args, "--json");
    let compare = arg_flag(args, "--compare");
    let preset = args
        .iter()
        .position(|a| a == "--preset")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "ibm-heron".to_string());

    use pqc::algorithms as alg;
    use pqc::noise::{run_noisy, NoiseModel};
    let circuit = match name {
        "bell" => alg::bell().unwrap(),
        "ghz" => alg::ghz(n).unwrap(),
        "grover" => alg::grover(n, &[22]).unwrap().0,
        "bv" => alg::bernstein_vazirani(n, 0b1011).unwrap(),
        "dj" => alg::deutsch_jozsa(n, &(0..1usize << n.min(4)).filter(|x| x & 1 == 1).collect::<Vec<_>>()).unwrap(),
        "qft" => {
            let mut c = alg::qft(n, false).unwrap();
            c.push(pqc::qpc::Op::MeasureAll);
            c
        }
        other => {
            eprintln!("pqc noise: unknown algorithm `{other}`");
            return 2;
        }
    };

    // кастомные параметры шума (переопределяют пресет)
    let has_custom = args.iter().any(|a| ["--p1q", "--p2q", "--pread", "--t1", "--t2", "--tg1", "--tg2"].contains(&a.as_str()));
    let farg = |flag: &str, def: f64| -> f64 {
        arg_num(args, flag, def.to_string())
            .ok()
            .and_then(|v: String| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(def)
    };
    let custom_model = NoiseModel {
        p1q: farg("--p1q", 0.0),
        p2q: farg("--p2q", 0.0),
        p_read: farg("--pread", 0.0),
        t1_us: farg("--t1", f64::INFINITY),
        t2_us: farg("--t2", f64::INFINITY),
        gate1_ns: farg("--tg1", 0.0),
        gate2_ns: farg("--tg2", 0.0),
    };

    let presets: Vec<&str> = if compare {
        vec!["ideal", "google-willow", "ibm-heron", "noisy-90s"]
    } else if has_custom {
        vec!["custom"]
    } else {
        if NoiseModel::preset(&preset).is_none() {
            eprintln!("pqc noise: unknown preset `{preset}` (ideal|ibm-heron|google-willow|noisy-90s)");
            return 2;
        }
        vec![preset.as_str()]
    };

    let mut rows: Vec<(String, f64, f64, f64, f64)> = Vec::new(); // (name, ideal_peak, noisy_peak, tvd, fidelity)
    for p in &presets {
        let m = if *p == "custom" {
            custom_model.clone()
        } else {
            NoiseModel::preset(p).unwrap()
        };
        let rep = run_noisy(&circuit, &m, shots, seed).unwrap();
        rows.push((p.to_string(), rep.ideal_peak, rep.noisy_peak, rep.tvd, rep.classical_fidelity));
    }

    if as_json {
        let arr: Vec<pqc::Json> = rows
            .iter()
            .map(|(p, ip, np, tvd, f)| {
                pqc::Json::Obj(vec![
                    ("preset".into(), pqc::Json::str(p)),
                    ("ideal_peak".into(), pqc::Json::num(*ip)),
                    ("noisy_peak".into(), pqc::Json::num(*np)),
                    ("tvd".into(), pqc::Json::num(*tvd)),
                    ("classical_fidelity".into(), pqc::Json::num(*f)),
                ])
            })
            .collect();
        let obj = pqc::Json::Obj(vec![
            ("engine".into(), pqc::Json::str("noise-mcwf")),
            ("algorithm".into(), pqc::Json::str(name)),
            ("n_qubits".into(), pqc::Json::num(circuit.n_qubits() as f64)),
            ("shots".into(), pqc::Json::num(shots as f64)),
            ("results".into(), pqc::Json::Arr(arr)),
        ]);
        println!("{}", obj.to_string());
        return 0;
    }

    println!("POLER Quantum PC — идеал vs железо (Monte Carlo траектории)");
    println!("схема: {name}, кубитов: {}, выстрелов: {shots}", circuit.n_qubits());
    println!(
        "\n{:<14} {:>12} {:>12} {:>10} {:>10}",
        "субстрат", "пик идеал", "пик железо", "TVD", "F_класс"
    );
    for (p, ip, np, tvd, f) in &rows {
        println!("{:<14} {:>12.4} {:>12.4} {:>10.4} {:>10.4}", p, ip, np, tvd, f);
    }
    println!(
        "\nидеальный субстрат: нуль ошибок гейтов/чтения/декогеренции;\
         физическое железо платит на каждом шаге (T1/T2, деполяризация, чтение)"
    );
    0
}

/// `pqc qaoa [--edges "0-1,1-2"] [--n K] [--p P] [--sweeps W] [--restarts R]
///           [--shots M] [--seed S] [--json]` — цикл Q: MaxCut-ансатц QAOA
/// с классической оптимизацией углов на идеальном субстрате.
fn cmd_qaoa(args: &[String]) -> i32 {
    let edges_str: String = arg_num(args, "--edges", String::new())
        .ok()
        .and_then(|v: String| v.parse::<String>().ok())
        .unwrap_or_default();
    let p: usize = match arg_num(args, "--p", 2usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qaoa: {e}");
            return 2;
        }
    };
    let sweeps: usize = match arg_num(args, "--sweeps", 12usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qaoa: {e}");
            return 2;
        }
    };
    let restarts: usize = match arg_num(args, "--restarts", 3usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qaoa: {e}");
            return 2;
        }
    };
    let shots: u64 = match arg_num(args, "--shots", 1024u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qaoa: {e}");
            return 2;
        }
    };
    let seed: u64 = match arg_num(args, "--seed", 42u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc qaoa: {e}");
            return 2;
        }
    };
    let as_json = arg_flag(args, "--json");

    let problem = if edges_str.trim().is_empty() {
        pqc::qaoa::MaxCut::demo()
    } else {
        let edges = match pqc::qaoa::MaxCut::parse_edges(&edges_str) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("pqc qaoa: {e:?}");
                return 2;
            }
        };
        let max_v = edges.iter().map(|&(i, j)| i.max(j)).max().unwrap_or(0);
        let n: usize = arg_num(args, "--n", 0usize).unwrap_or(0);
        let n = if n == 0 { max_v + 1 } else { n };
        match pqc::qaoa::MaxCut::new(n, edges) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("pqc qaoa: {e:?}");
                return 2;
            }
        }
    };

    let cfg = pqc::qaoa::QaoaConfig {
        p,
        sweeps,
        restarts,
        shots,
        seed,
    };
    let rep = match pqc::qaoa::run_qaoa(&problem, &cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc qaoa: {e:?}");
            return 1;
        }
    };

    let bits = |o: u64| -> String {
        let mut s = String::new();
        for q in (0..rep.n_qubits).rev() {
            s.push(if o >> q & 1 == 1 { '1' } else { '0' });
        }
        s
    };

    if as_json {
        let counts: Vec<pqc::Json> = rep
            .counts
            .iter()
            .map(|(o, c)| pqc::Json::Arr(vec![pqc::Json::num(*o as f64), pqc::Json::num(*c as f64)]))
            .collect();
        let edges: Vec<pqc::Json> = rep
            .edges
            .iter()
            .map(|(i, j)| pqc::Json::Arr(vec![pqc::Json::num(*i as f64), pqc::Json::num(*j as f64)]))
            .collect();
        let gammas: Vec<pqc::Json> = rep.params[..rep.p].iter().map(|x| pqc::Json::num(*x)).collect();
        let betas: Vec<pqc::Json> = rep.params[rep.p..].iter().map(|x| pqc::Json::num(*x)).collect();
        let obj = pqc::Json::Obj(vec![
            ("engine".into(), pqc::Json::str("qpc-qaoa")),
            ("problem".into(), pqc::Json::str("maxcut")),
            ("n_qubits".into(), pqc::Json::num(rep.n_qubits as f64)),
            ("edges".into(), pqc::Json::Arr(edges)),
            ("p".into(), pqc::Json::num(rep.p as f64)),
            ("gammas".into(), pqc::Json::Arr(gammas)),
            ("betas".into(), pqc::Json::Arr(betas)),
            ("expected_cut".into(), pqc::Json::num(rep.expected_cut)),
            ("expected_cut_init".into(), pqc::Json::num(rep.expected_cut_init)),
            ("best_bits".into(), pqc::Json::str(bits(rep.best_bits))),
            ("best_cut".into(), pqc::Json::num(rep.best_cut as f64)),
            (
                "optimum".into(),
                match rep.optimum {
                    Some(o) => pqc::Json::num(o as f64),
                    None => pqc::Json::Null,
                },
            ),
            (
                "approx_ratio".into(),
                match rep.approx_ratio {
                    Some(r) => pqc::Json::num(r),
                    None => pqc::Json::Null,
                },
            ),
            ("evals".into(), pqc::Json::num(rep.evals as f64)),
            ("gate_count".into(), pqc::Json::num(rep.gate_count as f64)),
            ("shots".into(), pqc::Json::num(rep.shots as f64)),
            ("norm".into(), pqc::Json::num(rep.norm)),
            ("counts".into(), pqc::Json::Arr(counts)),
        ]);
        println!("{}", obj.to_string());
        return 0;
    }

    println!("POLER Quantum PC — QAOA MaxCut (цикл Q)");
    println!(
        "граф: {} вершин, {} рёбер; анзац p = {}, вычислений E[cut]: {}",
        rep.n_qubits,
        rep.edges.len(),
        rep.p,
        rep.evals
    );
    println!(
        "E[cut]: {:.4} → {:.4}",
        rep.expected_cut_init, rep.expected_cut
    );
    println!(
        "лучший битстринг |{}⟩ — разрез {} из {}",
        bits(rep.best_bits),
        rep.best_cut,
        rep.optimum.map_or_else(|| "?".to_string(), |o| o.to_string())
    );
    if let (Some(_), Some(r)) = (rep.optimum, rep.approx_ratio) {
        println!("аппроксимационное отношение: {:.1}%", 100.0 * r);
    } else {
        println!("переборный оптимум: n > 20 — честная граница");
    }
    println!("норма: {:.16}", rep.norm);
    0
}

/// `pqc substrate [--dim N] [--steps N] [--eta H] [--gamma G] [--mu M]
///               [--fill N] [--scf U] [--seed S] [--trace N] [--json]`
fn cmd_substrate(args: &[String]) -> i32 {
    let dim: usize = match arg_num(args, "--dim", 4usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let steps: usize = match arg_num(args, "--steps", 400usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let eta: f64 = match arg_num(args, "--eta", 0.05f64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let gamma: f64 = match arg_num(args, "--gamma", 0.0f64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let mu: f64 = match arg_num(args, "--mu", 1.0f64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let fill: usize = match arg_num(args, "--fill", 2usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let scf_u: f64 = match arg_num(args, "--scf", 0.0f64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let seed: u64 = match arg_num(args, "--seed", 42u64) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let trace_every: usize = match arg_num(args, "--trace", 25usize) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 2;
        }
    };
    let as_json = arg_flag(args, "--json");

    let cfg = pqc::substrate::SubstrateConfig {
        eta,
        gamma,
        mu,
        particles: fill,
        steps,
        scf_u,
    };
    let rep = match pqc::substrate::run_random(dim, seed, cfg, trace_every) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pqc substrate: {e}");
            return 1;
        }
    };

    if as_json {
        let mat_json = |m: &pqc::substrate::CMat| -> pqc::Json {
            let rows: Vec<pqc::Json> = (0..m.n())
                .map(|i| {
                    let row: Vec<pqc::Json> = (0..m.n())
                        .map(|j| {
                            let v = m.at(i, j);
                            pqc::Json::Arr(vec![pqc::Json::num(v.re), pqc::Json::num(v.im)])
                        })
                        .collect();
                    pqc::Json::Arr(row)
                })
                .collect();
            pqc::Json::Arr(rows)
        };
        let pts: Vec<pqc::Json> = rep
            .points
            .iter()
            .map(|p| {
                pqc::Json::Obj(vec![
                    ("step".into(), pqc::Json::num(p.step as f64)),
                    ("trace".into(), pqc::Json::num(p.trace)),
                    ("purity".into(), pqc::Json::num(p.purity)),
                    ("energy".into(), pqc::Json::num(p.energy)),
                    ("lyapunov".into(), pqc::Json::num(p.lyapunov)),
                    ("defect".into(), pqc::Json::num(p.defect)),
                    ("rotor_work".into(), pqc::Json::num(p.rotor_work)),
                    ("precession_speed".into(), pqc::Json::num(p.precession_speed)),
                ])
            })
            .collect();
        let obj = pqc::Json::Obj(vec![
            ("engine".into(), pqc::Json::str("qpc-substrate")),
            ("dim".into(), pqc::Json::num(rep.dim as f64)),
            ("seed".into(), pqc::Json::num(seed as f64)),
            ("config".into(), pqc::Json::Obj(vec![
                ("eta".into(), pqc::Json::num(cfg.eta)),
                ("gamma".into(), pqc::Json::num(cfg.gamma)),
                ("mu".into(), pqc::Json::num(cfg.mu)),
                ("particles".into(), pqc::Json::num(cfg.particles as f64)),
                ("steps".into(), pqc::Json::num(cfg.steps as f64)),
                ("scf_u".into(), pqc::Json::num(cfg.scf_u)),
            ])),
            ("hamiltonian".into(), mat_json(&rep.hamiltonian)),
            ("p0".into(), mat_json(&rep.p0)),
            ("points".into(), pqc::Json::Arr(pts)),
            ("energy_violations".into(), pqc::Json::num(rep.energy_violations as f64)),
            ("rotor_work_total".into(), pqc::Json::num(rep.rotor_work_total)),
            ("max_trace_drift".into(), pqc::Json::num(rep.max_trace_drift)),
            ("final_defect".into(), pqc::Json::num(rep.final_defect)),
            ("final_commutator".into(), pqc::Json::num(rep.final_commutator)),
            ("scf_residual".into(), pqc::Json::num(rep.scf_residual)),
        ]);
        println!("{}", obj.to_string());
        return 0;
    }

    println!("POLER Quantum PC — UDE substrate flow (cycle G, Vol. VII §2.2)");
    println!(
        "dim: {}, particles: {}, steps: {}, eta: {}, gamma: {}, mu: {}, scf_u: {}",
        rep.dim, cfg.particles, cfg.steps, cfg.eta, cfg.gamma, cfg.mu, cfg.scf_u
    );
    println!("\ntrajectory (every {trace_every} steps):");
    println!("  {:>6}  {:>12}  {:>12}  {:>14}  {:>12}  {:>12}", "step", "Tr P", "Tr P^2", "E = Tr HP", "||[H,P]||^2", "||P^2-P||");
    for p in &rep.points {
        println!(
            "  {:>6}  {:>12.6}  {:>12.6}  {:>14.8}  {:>12.3e}  {:>12.3e}",
            p.step, p.trace, p.purity, p.energy, p.lyapunov, p.defect
        );
    }
    println!("\nsummary:");
    println!("  energy violations (>1e-12 up-steps): {}", rep.energy_violations);
    println!("  rotor |work| total:            {:.3e}", rep.rotor_work_total);
    println!("  max trace drift in flight:     {:.3e}", rep.max_trace_drift);
    println!("  final idempotency defect:      {:.3e}", rep.final_defect);
    println!("  final ||[F,P]||_HS:            {:.3e}", rep.final_commutator);
    println!("  SCF residual ||F-F_prev||:     {:.3e}", rep.scf_residual);
    0
}
