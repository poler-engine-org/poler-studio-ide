"""
poler_toolkit.core — общие утилиты.

Отвечает за:
  - Чтение файлов (.txt / .md / .epub) через POLER's read_file/read_epub
  - Нормализацию текста (BOM, CRLF, whitespace)
  - Авто-detect keyword (самый частый знаменательный токен)
  - Разбиение на главы (regex, авто-подбор паттерна)
  - Подготовку выходных директорий с timestamp
"""

from __future__ import annotations

import re
import sys
import logging
from pathlib import Path
from datetime import datetime
from typing import Optional, List, Tuple, Dict, Any
from collections import Counter

# Импорт POLER (он лежит рядом или доступен через sys.path)
from . import paths as _paths
_paths.ensure_poler_v6_on_path()
import poler_v6 as P  # noqa: E402

from .errors import FileError, UnsupportedFormatError, PolerEngineError

log = logging.getLogger("poler_toolkit.core")


# ============================================================
# Константы
# ============================================================
SUPPORTED_TEXT_EXT = {".txt", ".md", ".markdown"}
SUPPORTED_EPUB_EXT = {".epub"}

# Lazy: resolve through paths.py (env-aware)
DEFAULT_OUTPUT_ROOT = _paths.get_output_root()

# Стоп-слова для авто-detect (короткие общие слова)
_AUTODETECT_STOP = {
    "и", "в", "на", "с", "по", "для", "не", "что", "это", "как", "но", "или",
    "же", "бы", "ли", "быть", "он", "она", "они", "мы", "вы", "я", "это",
    "то", "от", "до", "из", "у", "о", "об", "при", "за", "под", "над",
    "and", "the", "a", "an", "of", "to", "in", "on", "for", "is", "are",
    "was", "were", "be", "been", "with", "as", "by", "that", "this",
}


# ============================================================
# Чтение файлов
# ============================================================
def read_text_file(path: str | Path) -> str:
    """Читает .txt / .md файл, нормализует BOM и CRLF."""
    p = Path(path)
    if not p.exists():
        raise FileError(str(p), "file not found")
    if p.suffix.lower() not in SUPPORTED_TEXT_EXT:
        raise UnsupportedFormatError(str(p), p.suffix.lower())
    try:
        raw = p.read_text(encoding="utf-8-sig", errors="replace")
    except Exception as e:
        raise FileError(str(p), f"read failed: {e}")
    # Нормализация
    raw = raw.replace("\r\n", "\n").replace("\r", "\n")
    # Удалить NUL
    raw = raw.replace("\x00", "")
    return raw


def read_epub_file(path: str | Path) -> str:
    """Читает .epub через POLER's read_epub."""
    p = Path(path)
    if not p.exists():
        raise FileError(str(p), "file not found")
    if p.suffix.lower() not in SUPPORTED_EPUB_EXT:
        raise UnsupportedFormatError(str(p), p.suffix.lower())
    try:
        text = P.read_epub(str(p))
    except Exception as e:
        raise PolerEngineError("read_epub", e)
    if not text:
        raise FileError(str(p), "epub returned empty text")
    return text


def read_any(path: str | Path) -> Tuple[str, str]:
    """
    Читает любой поддерживаемый файл.
    Returns: (text, format) — format ∈ {'txt', 'md', 'epub'}.
    """
    p = Path(path)
    ext = p.suffix.lower()
    if ext in SUPPORTED_TEXT_EXT:
        return read_text_file(p), ext.lstrip(".")
    if ext in SUPPORTED_EPUB_EXT:
        return read_epub_file(p), "epub"
    raise UnsupportedFormatError(str(p), ext)


# ============================================================
# Авто-detect keyword
# ============================================================
def auto_detect_keyword(text: str, top_n: int = 10) -> List[Tuple[str, int]]:
    """
    Находит самые частые знаменательные токены (>3 символа, не stop-word).
    Возвращает [(word, count), ...] отсортированный по убыванию.
    """
    tokens = re.findall(r"[\w’']+", text.lower())
    counter = Counter(tokens)
    # Фильтруем стоп-слова и короткие
    filtered = [
        (w, c) for w, c in counter.most_common(200)
        if len(w) > 3 and w not in _AUTODETECT_STOP
    ]
    return filtered[:top_n]


def pick_keyword(text: str, hint: Optional[str] = None) -> str:
    """
    Выбирает keyword для анализа.
    Если hint задан — использует его.
    Иначе — берёт самый частый знаменательный токен.
    """
    if hint:
        return hint
    top = auto_detect_keyword(text, top_n=1)
    if not top:
        raise PolerEngineError(
            "auto_detect_keyword",
            RuntimeError("no meaningful tokens found"),
        )
    return top[0][0]


# ============================================================
# Разбиение на главы
# ============================================================
# Готовые паттерны под разные языки/форматы
CHAPTER_PATTERNS: Dict[str, str] = {
    # Russian: "Глава 1", "ГЛАВА 1", "Глава 1: Название"
    "ru": r"^\s*(Пролог[:\s].*|Глава\s+\d+[^\n]*|Роздiл\s+\d+[^\n]*)\s*$",
    # Ukrainian: "Розділ 1", "Глава 1"
    "ua": r"^\s*(Пролог[:\s].*|Глава\s+\d+[^\n]*|Роздiл\s+\d+[^\n]*)\s*$",
    # English: "Chapter 1", "CHAPTER 1"
    "en": r"^\s*(Prologue[:\s].*|Chapter\s+\d+[^\n]*)\s*$",
    # Inline (для EPUB без переносов): "ГЛАВА N" где угодно в строке
    "inline_ru": r"ГЛАВА\s+\d+",
    "inline_en": r"CHAPTER\s+\d+",
}


def split_chapters(
    text: str,
    pattern: Optional[str] = None,
    lang: str = "auto",
    min_chapter_chars: int = 500,
) -> List[Dict[str, Any]]:
    """
    Разбивает текст на главы.

    Args:
        text: исходный текст
        pattern: regex pattern (приоритет). Если None — авто-подбор.
        lang: 'ru' / 'ua' / 'en' / 'auto' / 'inline_ru' / 'inline_en'
        min_chapter_chars: отсечка оглавлений (главы < этого размера пропускаются)

    Returns: [{'title': str, 'body': str, 'start': int, 'end': int, 'paragraphs': [str, ...]}, ...]
    """
    # Авто-подбор паттерна: пробуем по очереди
    if pattern is None:
        if lang == "auto":
            candidates = ["ru", "ua", "en", "inline_ru", "inline_en"]
        else:
            candidates = [lang]
        chosen = None
        for cand in candidates:
            pat = CHAPTER_PATTERNS.get(cand)
            if not pat:
                continue
            matches = list(re.finditer(pat, text, re.MULTILINE | re.IGNORECASE))
            if len(matches) >= 2:
                chosen = pat
                log.debug(f"Auto-picked chapter pattern '{cand}': {len(matches)} matches")
                break
        if chosen is None:
            log.warning("No chapter pattern matched; treating whole text as one chapter")
            return [{
                "title": "Whole text",
                "body": text,
                "start": 0,
                "end": len(text),
                "paragraphs": _split_paragraphs(text),
            }]
        pattern = chosen

    matches = list(re.finditer(pattern, text, re.MULTILINE | re.IGNORECASE))
    if not matches:
        log.warning("Pattern matched 0 chapters; treating whole text as one chapter")
        return [{
            "title": "Whole text",
            "body": text,
            "start": 0,
            "end": len(text),
            "paragraphs": _split_paragraphs(text),
        }]

    chapters = []
    for i, m in enumerate(matches):
        title = m.group(1) if m.groups() else m.group(0)
        title = title.strip()
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        body = text[start:end].strip()
        if len(body) < min_chapter_chars:
            log.debug(f"Skipping short section: '{title}' ({len(body)} chars)")
            continue
        chapters.append({
            "title": title,
            "body": body,
            "start": start,
            "end": end,
            "paragraphs": _split_paragraphs(body),
        })
    # Если все главы отфильтрованы — вернуть весь текст как одну главу
    if not chapters:
        log.warning("All matched chapters were below min_chapter_chars; "
                    "treating whole text as one chapter")
        return [{
            "title": "Whole text",
            "body": text,
            "start": 0,
            "end": len(text),
            "paragraphs": _split_paragraphs(text),
        }]
    return chapters


def _split_paragraphs(body: str) -> List[str]:
    """Разбивает тело главы на абзацы."""
    if "\n\n" in body:
        return [p.strip() for p in re.split(r"\n\s*\n", body) if p.strip()]
    # Fallback: группы по 5 предложений
    sentences = re.split(r"(?<=[.!?…])\s+", body)
    paragraphs = []
    for i in range(0, len(sentences), 5):
        p = " ".join(sentences[i:i + 5]).strip()
        if p:
            paragraphs.append(p)
    return paragraphs


# ============================================================
# Выходные директории
# ============================================================
def make_output_dir(
    task_name: str,
    output_root: Optional[Path] = None,
    timestamp: Optional[str] = None,
) -> Path:
    """
    Создаёт директорию вида <root>/<task>_<YYYYMMDD_HHMMSS>/.
    Если output_root не задан — резолвится через paths.get_output_root()
    (env-aware: $POLER_TOOLKIT_OUTPUT / XDG_DATA_HOME / ~/.local/share/...).
    """
    root = output_root or _paths.get_output_root()
    ts = timestamp or datetime.now().strftime("%Y%m%d_%H%M%S")
    # Очистить task_name от небезопасных символов
    safe_task = re.sub(r"[^A-Za-z0-9_\-]", "_", task_name)
    out_dir = root / f"{safe_task}_{ts}"
    out_dir.mkdir(parents=True, exist_ok=True)
    return out_dir


# ============================================================
# Безопасный импорт POLER API
# ============================================================
def get_poler() -> Any:
    """Возвращает модуль poler_v6 для прямого использования в recipes."""
    return P


def safe_call(func_name: str, *args, **kwargs) -> Any:
    """
    Безопасный вызов функции POLER с логированием и обработкой ошибок.
    Бросает PolerEngineError при падении.
    """
    func = getattr(P, func_name, None)
    if func is None:
        raise PolerEngineError(func_name, AttributeError(f"function '{func_name}' not found in poler_v6"))
    try:
        log.debug(f"POLER call: {func_name}(*{args!r}, **{kwargs!r})")
        return func(*args, **kwargs)
    except Exception as e:
        raise PolerEngineError(func_name, e) from e


__all__ = [
    "read_text_file",
    "read_epub_file",
    "read_any",
    "auto_detect_keyword",
    "pick_keyword",
    "split_chapters",
    "make_output_dir",
    "get_poler",
    "safe_call",
    "CHAPTER_PATTERNS",
    "SUPPORTED_TEXT_EXT",
    "SUPPORTED_EPUB_EXT",
    "DEFAULT_OUTPUT_ROOT",
]
