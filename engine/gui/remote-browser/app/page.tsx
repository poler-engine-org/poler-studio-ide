/* eslint-disable @typescript-eslint/no-explicit-any */
"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { io } from "socket.io-client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  ArrowLeft,
  ArrowRight,
  RotateCw,
  Link2,
  Trash2,
  Globe,
  Keyboard,
  Loader2,
} from "lucide-react";

const VW = 1280;
const VH = 800;
const SERVICE_PORT = 3031;

const SPECIAL_KEYS = new Set([
  "Enter",
  "Backspace",
  "Delete",
  "Tab",
  "Escape",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

export default function Home() {
  const socketRef = useRef<any>(null);
  const viewRef = useRef<HTMLDivElement>(null);

  const [frame, setFrame] = useState<string>("");
  const [frameNo, setFrameNo] = useState(0);
  const [meta, setMeta] = useState({ u: "", ti: "" });
  const [age, setAge] = useState<number>(0);
  const [connected, setConnected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [urlInput, setUrlInput] = useState("");
  const [urlSync, setUrlSync] = useState(true);
  const [focused, setFocused] = useState(false);
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);
  const [driveMsgs, setDriveMsgs] = useState<string[]>([]);
  const [quality, setQuality] = useState("58");

  const pushDrive = useCallback((m: string) => {
    setDriveMsgs((prev) => [...prev.slice(-4), m]);
  }, []);

  useEffect(() => {
    const s = io(`/?XTransformPort=${SERVICE_PORT}`, {
      transports: ["websocket", "polling"],
    });
    socketRef.current = s;
    s.on("connect", () => setConnected(true));
    s.on("disconnect", () => setConnected(false));
    s.on("hello", (d: any) => {
      if (d && d.u) setMeta({ u: d.u, ti: d.ti || "" });
    });
    s.on("frame", (f: any) => {
      if (!f || !f.d) return;
      setFrameNo((n) => n + 1);
      setFrame(f.d);
      setMeta({ u: f.u || "", ti: f.ti || "" });
      setAge(Math.max(0, Date.now() - (f.t || Date.now())));
      setBusy(false);
      if (urlSync) setUrlInput((f.u || "").split("://")[1] || f.u || "");
    });
    s.on("nav", (d: any) => {
      if (d && d.u) setMeta((m) => ({ ...m, u: d.u }));
    });
    s.on("nav_err", (d: any) => pushDrive("⚠️ " + ((d && d.m) || "навигация не удалась")));
    s.on("drive", (d: any) => pushDrive((d && d.m) || ""));
    return () => {
      s.close();
    };
  }, [pushDrive, urlSync]);

  // wheel — не пассивный слушатель, чтобы страница не скроллилась сама
  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      socketRef.current?.emit("wheel", { dx: e.deltaX, dy: e.deltaY });
      setBusy(true);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  useEffect(() => {
    socketRef.current?.emit("quality", { q: parseInt(quality, 10) });
  }, [quality]);

  const toPageCoords = (e: React.PointerEvent | React.MouseEvent) => {
    const el = e.currentTarget as HTMLElement;
    const r = el.getBoundingClientRect();
    const x = Math.round(((e.clientX - r.left) / r.width) * VW);
    const y = Math.round(((e.clientY - r.top) / r.height) * VH);
    return { x: Math.min(VW - 1, Math.max(0, x)), y: Math.min(VH - 1, Math.max(0, y)) };
  };

  const onPointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    viewRef.current?.focus();
    const { x, y } = toPageCoords(e);
    socketRef.current?.emit("click", { x, y, b: "left" });
    setBusy(true);
  };

  const onDouble = (e: React.MouseEvent) => {
    e.preventDefault();
    const { x, y } = toPageCoords(e);
    socketRef.current?.emit("dblclick", { x, y });
    setBusy(true);
  };

  const onContext = (e: React.MouseEvent) => {
    e.preventDefault();
    const { x, y } = toPageCoords(e);
    socketRef.current?.emit("click", { x, y, b: "right" });
    setBusy(true);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const t = e.target as HTMLElement;
    if (t && t.closest && t.closest("input,textarea,select,button")) return;
    if (e.key === "F5" || (e.key === "r" && (e.ctrlKey || e.metaKey) && !e.shiftKey)) return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v") return; // paste придёт своим событием
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      socketRef.current?.emit("key", { k: `Control+${k}` });
    } else if (SPECIAL_KEYS.has(e.key)) {
      socketRef.current?.emit("key", { k: e.key });
    } else if (e.key.length === 1) {
      socketRef.current?.emit("text", { t: e.key });
    }
    setBusy(true);
  };

  const onPaste = (e: React.ClipboardEvent) => {
    const t = e.clipboardData.getData("text");
    if (t) {
      socketRef.current?.emit("text", { t });
      setBusy(true);
    }
  };

  const navigate = (u: string) => {
    const v = u.trim();
    if (!v) return;
    socketRef.current?.emit("navigate", { url: v });
    setBusy(true);
  };

  const act = (name: string) => {
    socketRef.current?.emit(name);
    setBusy(true);
  };

  const title = meta.ti || meta.u || "—";
  const host = (() => {
    try {
      return meta.u ? new URL(meta.u).hostname : "";
    } catch {
      return "";
    }
  })();

  return (
    <div className="min-h-screen flex flex-col bg-zinc-950 text-zinc-100">
      {/* Шапка */}
      <header className="border-b border-zinc-800/80 bg-zinc-950/95 sticky top-0 z-20">
        <div className="mx-auto max-w-7xl px-4 py-3 sm:px-6 lg:px-8 flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2">
            <Globe className="h-5 w-5 text-emerald-400" />
            <h1 className="text-base font-semibold tracking-tight">
              POLER · Удалённый браузер
            </h1>
          </div>
          <span
            className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${
              connected
                ? "bg-emerald-500/10 text-emerald-400 border border-emerald-500/30"
                : "bg-red-500/10 text-red-400 border border-red-500/30"
            }`}
          >
            {connected ? "на связи" : "нет связи"}
          </span>
          {busy && (
            <span className="inline-flex items-center gap-1.5 text-xs text-amber-400">
              <Loader2 className="h-3.5 w-3.5 animate-spin" /> обновляю кадр…
            </span>
          )}
          <div className="ml-auto flex items-center gap-2">
            <Select value={quality} onValueChange={setQuality}>
              <SelectTrigger className="w-[120px] h-9 bg-zinc-900 border-zinc-700 text-xs">
                <SelectValue placeholder="качество" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="40">быстро</SelectItem>
                <SelectItem value="58">обычно</SelectItem>
                <SelectItem value="75">резко</SelectItem>
              </SelectContent>
            </Select>
            <Button
              variant="outline"
              size="sm"
              className="h-9 bg-zinc-900 border-emerald-700/60 text-emerald-300 hover:bg-emerald-900/30 hover:text-emerald-200"
              onClick={() => {
                socketRef.current?.emit("drive_auth");
                pushDrive("🔗 Привязка запрошена…");
              }}
            >
              <Link2 className="mr-1.5 h-4 w-4" /> Привязать Диск
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-9 bg-zinc-900 border-zinc-700 text-zinc-300 hover:bg-zinc-800"
              onClick={() => {
                if (confirm("Стереть все куки и логины этого браузера?")) {
                  socketRef.current?.emit("forget");
                }
              }}
            >
              <Trash2 className="mr-1.5 h-4 w-4" /> Забыть сессию
            </Button>
          </div>
        </div>
        {/* Адресная строка */}
        <div className="mx-auto max-w-7xl px-4 pb-3 sm:px-6 lg:px-8 flex items-center gap-2">
          <Button
            variant="outline"
            size="icon"
            className="h-9 w-9 bg-zinc-900 border-zinc-700"
            onClick={() => act("back")}
            aria-label="Назад"
          >
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            className="h-9 w-9 bg-zinc-900 border-zinc-700"
            onClick={() => act("forward")}
            aria-label="Вперёд"
          >
            <ArrowRight className="h-4 w-4" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            className="h-9 w-9 bg-zinc-900 border-zinc-700"
            onClick={() => act("reload")}
            aria-label="Обновить"
          >
            <RotateCw className="h-4 w-4" />
          </Button>
          <form
            className="flex-1 flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              navigate(urlInput);
              setUrlSync(true);
            }}
          >
            <Input
              value={urlInput}
              onChange={(e) => {
                setUrlInput(e.target.value);
                setUrlSync(false);
              }}
              placeholder="адрес: drive.google.com …"
              className="h-9 bg-zinc-900 border-zinc-700 font-mono text-sm"
              aria-label="Адрес страницы"
            />
            <Button type="submit" size="sm" className="h-9 bg-emerald-600 hover:bg-emerald-500 text-white">
              Перейти
            </Button>
          </form>
        </div>
      </header>

      {/* Рабочая область */}
      <main className="flex-1 mx-auto w-full max-w-7xl px-4 py-4 sm:px-6 lg:px-8 grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
        {/* Окно удалённого браузера */}
        <section aria-label="Окно удалённого браузера">
          <div
            ref={viewRef}
            role="application"
            aria-label="Экран удалённого браузера. Клик — мышка, печатаешь — клавиатура, Ctrl+V — вставка."
            tabIndex={0}
            onPointerDown={onPointerDown}
            onDoubleClick={onDouble}
            onContextMenu={onContext}
            onKeyDown={onKeyDown}
            onPaste={onPaste}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            onPointerMove={(e) => {
              const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
              setCursor({ x: e.clientX - r.left, y: e.clientY - r.top });
            }}
            onPointerLeave={() => setCursor(null)}
            className="relative w-full overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 outline-none focus:ring-2 focus:ring-emerald-500/70 select-none"
            style={{ aspectRatio: `${VW} / ${VH}`, cursor: "none" }}
          >
            {frame ? (
              <img
                src={`data:image/jpeg;base64,${frame}`}
                alt={`Экран удалённого браузера: ${title}`}
                draggable={false}
                className="absolute inset-0 h-full w-full object-contain pointer-events-none"
              />
            ) : (
              <div className="absolute inset-0 grid place-items-center text-zinc-500 text-sm">
                <Loader2 className="mr-2 h-5 w-5 animate-spin inline" /> жду первый кадр…
              </div>
            )}

            {/* локальный курсор-точка */}
            {cursor && (
              <div
                className="absolute h-3.5 w-3.5 rounded-full border-2 border-emerald-400 bg-emerald-400/30 pointer-events-none z-10"
                style={{ left: cursor.x - 7, top: cursor.y - 7 }}
              />
            )}

            {/* подсказка про фокус */}
            {!focused && (
              <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-10 pointer-events-none rounded-lg bg-zinc-950/80 border border-zinc-700 px-4 py-2 text-xs text-zinc-300 flex items-center gap-2">
                <Keyboard className="h-4 w-4 text-emerald-400" />
                кликни по экрану — и печатай / Ctrl+V прямо сюда
              </div>
            )}

            {/* статусная плашка */}
            <div className="absolute bottom-0 inset-x-0 z-10 flex items-center gap-2 px-3 py-1.5 text-[11px] font-mono text-zinc-400 bg-zinc-950/70 border-t border-zinc-800 pointer-events-none truncate">
              <span className="text-emerald-400">{host || "—"}</span>
              <span className="truncate">{title}</span>
              <span className="ml-auto shrink-0">
                {frame ? `кадр ${frameNo} · ${age > 9000 ? "старый" : age + " мс"}` : "…"}
              </span>
            </div>
          </div>
          <p className="mt-2 text-xs text-zinc-500">
            Это настоящий Chromium в песочнице POLER: колёсико — скролл, двойной клик — двойной,
            правый клик — контекстное меню сайта. Пароли не сохраняю в чате — только сюда.
          </p>
        </section>

        {/* Боковая панель: привязка + инструкция */}
        <aside className="flex flex-col gap-4 min-w-0">
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/60 p-4">
            <h2 className="text-sm font-semibold text-emerald-300 mb-2 flex items-center gap-2">
              <Link2 className="h-4 w-4" /> Привязка Google
            </h2>
            <ol className="list-decimal list-inside space-y-1.5 text-xs text-zinc-300">
              <li>Нажми «Привязать Диск» сверху.</li>
              <li>В окне слева откроется вход Google — войди (2FA если спросят).</li>
              <li>Нажми «Разрешить» — токен запишется сам.</li>
              <li>Скажи ассистенту в чате «готово» — он проверит и начнёт выгрузку.</li>
            </ol>
            {driveMsgs.length > 0 && (
              <div className="mt-3 max-h-40 overflow-y-auto rounded-lg border border-zinc-800 bg-zinc-950 p-2 space-y-1">
                {driveMsgs.map((m, i) => (
                  <p key={i} className="text-[11px] leading-relaxed text-zinc-400 break-words">
                    {m}
                  </p>
                ))}
              </div>
            )}
          </div>

          <div className="rounded-xl border border-zinc-800 bg-zinc-900/60 p-4">
            <h2 className="text-sm font-semibold text-zinc-200 mb-2">Что это даёт</h2>
            <ul className="space-y-1.5 text-xs text-zinc-400">
              <li>• Один вход = доступ к Диску и Gemini-перепискам через твою же сессию.</li>
              <li>• OAuth-редиректы на 127.0.0.1 попадают куда надо — в песочнице.</li>
              <li>• Colab открывается прямо здесь же (colab.research.google.com).</li>
              <li>• «Забыть сессию» одним кликом — куки стираются целиком.</li>
            </ul>
            <p className="mt-3 text-[11px] text-zinc-500 border-t border-zinc-800 pt-2">
              Ссылка превью — личная, но не оставляй её открытой на чужом экране.
            </p>
          </div>
        </aside>
      </main>

      <footer className="mt-auto border-t border-zinc-800/80 bg-zinc-950">
        <div className="mx-auto max-w-7xl px-4 py-4 text-xs text-zinc-500 sm:px-6 lg:px-8 flex flex-wrap gap-2 justify-between">
          <span>Удалённый браузер POLER · Chromium 153 · песочница ассистента</span>
          <span className="font-mono">порт 3031 · стриминг JPEG · куки в профиле песочницы</span>
        </div>
      </footer>
    </div>
  );
}
