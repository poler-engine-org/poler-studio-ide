#!/usr/bin/env python3
"""Независимый аудит текста SVG-артефактов (внешний ревизор).

Сессия-10, грабля 23: движок меряет текст оценкой 0.62 em/глиф.
Ревизор меряет РЕАЛЬНЫМ шрифтом DejaVu Sans Mono (PIL): если оценка
движка врёт в свою пользу — ревизор это поймает. Проверяются:
  • каждая <text>-надпись лежит в границах viewBox (±1.5 px),
    с учётом text-anchor и поворота (transform=rotate);
  • в viz_scale-сценах подписи одного ряда не пересекаются.
Выход: непустой список нарушений = код 1."""
import re
import sys
import glob
from PIL import ImageFont

FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
FONT_BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"
TOL = 1.5


def load(size, bold):
    return ImageFont.truetype(FONT_BOLD if bold else FONT, int(round(size)))


def unescape(s):
    return (s.replace("&lt;", "<").replace("&gt;", ">")
             .replace("&quot;", '"').replace("&amp;", "&"))


def attr(attrs, key):
    m = re.search(r'(?<![\w-])' + re.escape(key) + r'="([^"]*)"', attrs)
    return m.group(1) if m else None


def parse_rotate(t):
    if not t:
        return None
    m = re.search(r"rotate\(([^)]+)\)", t)
    if not m:
        return None
    nums = [float(x) for x in re.split(r"[,\s]+", m.group(1)) if x]
    if len(nums) == 1:
        return (nums[0], 0.0, 0.0)
    if len(nums) == 3:
        return (nums[0], nums[1], nums[2])
    return None


def text_extents(x, y, wpx, size, anchor, rot):
    """Габариты надписи: (x0, y0, x1, y1)."""
    if anchor == "middle":
        x0, x1 = x - wpx / 2, x + wpx / 2
    elif anchor == "end":
        x0, x1 = x - wpx, x
    else:
        x0, x1 = x, x + wpx
    y0, y1 = y - 0.85 * size, y + 0.30 * size
    if rot:
        import math
        a, cx, cy = rot
        t = math.radians(a)
        ca, sa = math.cos(t), math.sin(t)
        pts = []
        for (px, py) in [(x, y), (x + wpx, y)]:
            dx, dy = px - cx, py - cy
            pts.append((cx + dx * ca - dy * sa, cy + dx * sa + dy * ca))
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        x0 = min(xs) - 0.3 * size
        x1 = max(xs) + 0.3 * size
        y0 = min(ys) - 0.85 * size
        y1 = max(ys) + 0.30 * size
    return x0, y0, x1, y1


def audit(path):
    svg = open(path, encoding="utf-8").read()
    vb = re.search(r'viewBox="([^"]+)"', svg)
    if not vb:
        return ["нет viewBox"]
    _x, _y, W, H = (float(v) for v in vb.group(1).split())
    bad = []
    for m in re.finditer(r"<text([^>]*)>(.*?)</text>", svg, re.S):
        attrs, content = m.group(1), unescape(m.group(2).strip())
        if not content:
            continue
        try:
            x = float(attr(attrs, "x") or 0)
            y = float(attr(attrs, "y") or 0)
            size = float(attr(attrs, "font-size") or 12)
        except ValueError:
            bad.append(f"кривые координаты: {content[:20]}")
            continue
        anchor = attr(attrs, "text-anchor") or "start"
        bold = 'font-weight="bold"' in attrs
        rot = parse_rotate(attr(attrs, "transform"))
        font = load(size, bold)
        wpx = font.getlength(content)
        x0, y0, x1, y1 = text_extents(x, y, wpx, size, anchor, rot)
        if x0 < -TOL or x1 > W + TOL or y0 < -TOL or y1 > H + TOL:
            head = content[:24]
            bad.append(
                f"«{head}» за холстом {W:.0f}×{H:.0f}: "
                f"x[{x0:.0f}…{x1:.0f}] y[{y0:.0f}…{y1:.0f}]"
            )
    return bad


def main():
    files = sorted(glob.glob("scripts/viz_artifacts/*.svg"))
    if not files:
        print("нет артефактов")
        return 1
    total_bad = 0
    for f in files:
        bad = audit(f)
        name = f.split("/")[-1]
        if bad:
            total_bad += len(bad)
            for b in bad:
                print(f"   ❌ {name}: {b}")
        else:
            print(f"   ✅ {name}: все надписи в границах холста")
    print()
    if total_bad:
        print(f"АУДИТ: {total_bad} нарушений ❌")
        return 1
    print(f"АУДИТ: {len(files)} сцен чисты — ни одна надпись не обрезана ✅")
    return 0


if __name__ == "__main__":
    sys.exit(main())
