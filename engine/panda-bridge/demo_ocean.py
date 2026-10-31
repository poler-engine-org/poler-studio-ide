#!/usr/bin/env python3
"""
demo_ocean.py — ПОЛИГОН «ЭФИРНЫЙ ОКЕАН»: POLER ENGINE × Panda3D.

Цикл W «Симбиоз» (v0.59.0). Вода — НЕ анимация и НЕ сеточный решатель
Panda3D: это живая спектральная гидродинамика GF(3) из libpoler_ffi.so
(тот же движок, что и `poler-engine --exec "game water"`):
  • состояние моря 1–2 КБ (2048 байт против 1 МБ f32-сетки);
  • целочисленные фазы 3^(p+m) — ноль f32-дрейфа за миллионы шагов;
  • дисперсия ω=√(gk+γk³), равновесие Пирсона–Московица.

Оптика (GLSL):
  • Френель — зеркало неба под скользящими углами;
  • солнечный блик — Blinn-Phong по аналитическим нормалям;
  • пена на гребнях — крутизна волны;
  • глубинный цвет — закон Бера–Ламберта;
  • туман к горизонту.

Кадр = ОДИН вызов C-ABI (polerf_water_geometries_f32: тик физики +
FFT-синтез + аналитические нормали) + одна memcpy в GeomVertexData.

Управление: мышь — обзор, колесо — зум, ESC — выход.
Установка: pip install panda3d numpy
Запуск:    python3 demo_ocean.py [ветер_м/с] [сетка]
Пример:    python3 demo_ocean.py 10 128
"""
import math
import sys
import time

sys.path.insert(0, __file__.rsplit("/", 1)[0])

import numpy as np

from poler_panda import Water, version

WIND = float(sys.argv[1]) if len(sys.argv) > 1 else 8.0
GRID = int(sys.argv[2]) if len(sys.argv) > 2 else 128
GRID = 1 << max(4, min(10, int(math.log2(GRID))))
DOMAIN = 140.0  # метров

from direct.showbase.ShowBase import ShowBase  # noqa: E402
from panda3d.core import (  # noqa: E402
    AmbientLight,
    DirectionalLight,
    Geom,
    GeomNode,
    GeomTriangles,
    GeomVertexData,
    GeomVertexFormat,
    TextNode,
    Vec3,
    Vec4,
    loadPrcFileData,
)

loadPrcFileData(
    "",
    """
window-title POLER ENGINE × Panda3D — Эфирный Океан (GF(3))
sync-video #t
show-frame-rate-meter #t
""",
)

# =============================================================================
# GLSL: Френель + солнце + пена + Беер–Ламберт + туман
# =============================================================================
WATER_VERTEX = """
#version 130
uniform mat4 p3d_ModelViewProjectionMatrix;

in vec4 p3d_Vertex;
in vec3 p3d_Normal;

out vec3 v_normal_w;
out vec3 v_pos_w;
out vec2 v_uv;
out float v_steep;

void main() {
    v_normal_w = p3d_Normal;
    v_pos_w = p3d_Vertex.xyz;
    v_uv = p3d_Vertex.xy * 0.12;
    v_steep = length(p3d_Normal.xz);
    gl_Position = p3d_ModelViewProjectionMatrix * p3d_Vertex;
}
"""

WATER_FRAGMENT = """
#version 130
uniform vec3 sun_dir;
uniform vec3 cam_pos;
uniform vec3 sky_color;
uniform vec3 deep_color;
uniform sampler2D normal_map;
uniform sampler2D foam_tex;
uniform sampler2D sky_tex;
uniform float os_time;

in vec3 v_normal_w;
in vec3 v_pos_w;
in vec2 v_uv;
in float v_steep;

out vec4 frag;

void main() {
    // --- 1. Двухскоростной каскад нормалей микро-волн (PBR) ---
    vec2 uv1 = v_uv * 1.8 + vec2(os_time * 0.035, os_time * 0.015);
    vec2 uv2 = v_uv * 3.7 - vec2(os_time * 0.025, -os_time * 0.045);
    vec3 n1 = texture2D(normal_map, uv1).xyz * 2.0 - 1.0;
    vec3 n2 = texture2D(normal_map, uv2).xyz * 2.0 - 1.0;
    vec3 micro_n = normalize(n1 + n2);

    // Смешивание спектральной макро-нормали (GF(3) кремний) с PBR-рябью
    vec3 N = normalize(v_normal_w + micro_n * 0.35);
    vec3 V = normalize(cam_pos - v_pos_w);
    if (dot(N, V) < 0.0) N = -N;

    // --- 2. Честный Френель (Schlick) ---
    float cos_theta = max(dot(N, V), 0.0);
    float fres = pow(1.0 - cos_theta, 5.0);
    float kr = mix(0.035, 0.98, fres);

    // --- 3. Отражение неба (HDR Panorama Lookup) ---
    vec3 R = reflect(-V, N);
    float sky_pitch = clamp(R.z * 0.5 + 0.5, 0.01, 0.99);
    vec3 sky_pbr = texture2D(sky_tex, vec2(0.5, sky_pitch)).rgb;
    vec3 sky = mix(sky_color, sky_pbr * 1.25, 0.85);

    // --- 4. Солнечный блик Blinn-Phong (анизотропный двойной лепесток) ---
    vec3 H = normalize(V + sun_dir);
    float ndh = max(dot(N, H), 0.0);
    float spec = pow(ndh, 380.0) * 3.5 + pow(ndh, 45.0) * 0.45;
    vec3 sun_spec = vec3(1.0, 0.97, 0.88) * spec;

    // --- 5. Подповерхностное рассеивание (Subsurface Scattering / Тонкий гребень) ---
    float sss = pow(clamp(dot(V, -sun_dir), 0.0, 1.0), 3.0) * clamp(v_steep * 1.8, 0.0, 1.0);
    vec3 sss_color = vec3(0.0, 0.65, 0.55) * sss * 0.8;

    // --- 6. Глубинный цвет Беера–Ламберта ---
    vec3 water_deep = mix(deep_color, vec3(0.01, 0.22, 0.32), clamp(v_steep * 1.4, 0.0, 1.0));
    vec3 water = water_deep + sss_color;

    // --- 7. Органическая пена на гребнях (PBR Foam Layering) ---
    vec4 foam_map = texture2D(foam_tex, v_uv * 1.2 + vec2(os_time * 0.01, 0.0));
    float foam_mask = smoothstep(0.25, 0.55, v_steep + foam_map.r * 0.15);
    vec3 foam_color = vec3(0.96, 0.98, 1.0) * (0.85 + 0.15 * foam_map.r);

    // Финальный композитинг
    vec3 col = mix(water, sky, kr) + sun_spec;
    col = mix(col, foam_color, foam_mask * 0.92);

    // --- 8. Атмосферный туман горизонта ---
    float fog = clamp(length(cam_pos - v_pos_w) / 480.0, 0.0, 0.88);
    col = mix(col, sky_pbr, fog);

    frag = vec4(col, 1.0);
}
"""


class OceanApp(ShowBase):
    def __init__(self):
        super().__init__()
        self.disableMouse()

        print(f"POLER FFI : {version()}")

        # --- море POLER (вся физика — в Rust) ---
        self.sea = Water(n=GRID, modes=160, seed=42, wind=WIND, domain=DOMAIN)
        self.cell = DOMAIN / (GRID - 1)
        print(
            f"Море      : сетка {GRID}×{GRID}, домен {DOMAIN:.0f} м, "
            f"ветер {WIND} м/с → H_s = {self.sea.significant_height:.2f} м "
            f"(Пирсон–Московиц {0.21 * WIND ** 2 / 9.81:.2f})"
        )

        # --- геометрия: динамическая плоскость GRID×GRID (V3n3) ---
        fmt = GeomVertexFormat.getV3n3()
        self.vdata = GeomVertexData("poler-ocean", fmt, Geom.UHDynamic)
        self.vdata.setNumRows(GRID * GRID)

        # статичные x, y + динамичные z и нормали
        idx = np.arange(GRID * GRID, dtype=np.float32)
        self._geom = np.zeros((GRID * GRID, 6), dtype=np.float32)
        self._geom[:, 0] = (idx % GRID) * self.cell - DOMAIN / 2
        self._geom[:, 1] = (idx // GRID) * self.cell - DOMAIN / 2
        self.vdata.modifyArray(0).modifyHandle().setData(self._geom.tobytes())

        tris = GeomTriangles(Geom.UHStatic)
        quads = np.arange(GRID * GRID, dtype=np.uint32).reshape(GRID, GRID)[:-1, :-1]
        a = quads.reshape(-1)
        b = a + 1
        c = a + GRID
        d = c + 1
        tri_idx = np.empty((a.size, 6), dtype=np.uint32)
        tri_idx[:, 0] = a
        tri_idx[:, 1] = c
        tri_idx[:, 2] = b
        tri_idx[:, 3] = b
        tri_idx[:, 4] = c
        tri_idx[:, 5] = d
        tris.setIndexType(Geom.NTUint32)
        tris.addNextVertices(a.size * 2)
        # индексный буфер одной записью (numpy → байты)
        tris.modifyVertices().modifyHandle().setData(tri_idx.tobytes())

        geom = Geom(self.vdata)
        geom.addPrimitive(tris)
        node = GeomNode("poler-ocean")
        node.addGeom(geom)
        self.ocean = self.render.attachNewNode(node)

        # --- шейдер оптики ---
        import os
        from panda3d.core import Shader, Texture, SamplerState

        shader = Shader.make(Shader.SL_GLSL, vertex=WATER_VERTEX, fragment=WATER_FRAGMENT)
        self.setBackgroundColor(0.36, 0.55, 0.78, 1.0)
        self.ocean.setShader(shader)
        self.ocean.setShaderInput("sun_dir", Vec3(0.45, 0.5, 0.74).normalized())
        self.ocean.setShaderInput("sky_color", Vec3(0.36, 0.55, 0.78))
        self.ocean.setShaderInput("deep_color", Vec3(0.015, 0.08, 0.16))
        self.ocean.setShaderInput("os_time", 0.0)

        # --- текстуры PBR (микро-нормали, пена и HDR-небо) ---
        assets_dir = os.path.join(os.path.dirname(__file__), "assets")
        pbr_dir = os.path.join(assets_dir, "pbr")
        normal_tex_path = os.path.join(pbr_dir, "water_micro_normal.png")
        foam_tex_path = os.path.join(pbr_dir, "foam_realistic.png")
        sky_tex_path = os.path.join(pbr_dir, "sky_hdr.png")

        if os.path.exists(normal_tex_path):
            tex_n = self.loader.loadTexture(normal_tex_path)
            tex_n.setWrapU(SamplerState.WM_repeat)
            tex_n.setWrapV(SamplerState.WM_repeat)
            self.ocean.setShaderInput("normal_map", tex_n)

        if os.path.exists(foam_tex_path):
            tex_f = self.loader.loadTexture(foam_tex_path)
            tex_f.setWrapU(SamplerState.WM_repeat)
            tex_f.setWrapV(SamplerState.WM_repeat)
            self.ocean.setShaderInput("foam_tex", tex_f)

        if os.path.exists(sky_tex_path):
            tex_s = self.loader.loadTexture(sky_tex_path)
            tex_s.setWrapU(SamplerState.WM_clamp)
            tex_s.setWrapV(SamplerState.WM_clamp)
            self.ocean.setShaderInput("sky_tex", tex_s)

        # --- звук (настоящий прибой + резонанс) ---
        sound_path = os.path.join(assets_dir, "ocean_real.ogg")
        if not os.path.exists(sound_path):
            sound_path = os.path.join(assets_dir, "ocean_ambient.wav")
        if os.path.exists(sound_path):
            try:
                self.ambient_sound = self.loader.loadSfx(sound_path)
                self.ambient_sound.setLoop(True)
                self.ambient_sound.setVolume(0.85)
                self.ambient_sound.play()
            except Exception as e:
                print(f"Звук: {e}")

        # --- камера и свет ---
        self.camera.setPos(0, -55, 18)
        self.camera.setHpr(-8, -12, 0)
        self.camLens.setFov(62)
        self.orbit = {"h": -8.0, "p": -12.0, "r": 60.0}
        self.accept("escape", sys.exit)
        self.accept("wheel_up", self._zoom, [-6])
        self.accept("wheel_down", self._zoom, [6])
        self.taskMgr.add(self._camera_task, "camera")

        alight = AmbientLight("ambient")
        alight.setColor(Vec4(0.35, 0.38, 0.42, 1))
        self.render.setLight(self.render.attachNewNode(alight))
        dlight = DirectionalLight("sun")
        dlight.setColor(Vec4(1.0, 0.96, 0.88, 1))
        dlnp = self.render.attachNewNode(dlight)
        dlnp.setHpr(35, -55, 0)
        self.render.setLight(dlnp)

        # --- HUD ---
        self.hud = self.a2dBottomLeft.attachNewNode(TextNode("hud"))
        self.hud.node().setText("POLER × Panda3D")
        self.hud.node().setTextScale(0.045)
        self.hud.setScale(1.4)
        self.hud.setPos(0.05, 0, 0.12)

        self.frames = 0
        self.t0 = time.perf_counter()
        self.tphys = 0.0
        self.taskMgr.add(self._physics_task, "poler-physics")

    # ------------------------------------------------------------------
    def _zoom(self, dr):
        self.orbit["r"] = max(18.0, min(240.0, self.orbit["r"] + dr))

    def _camera_task(self, task):
        md = self.mouseWatcherNode
        if md.hasMouse():
            x, y = md.getMouseX(), md.getMouseY()
            self.orbit["h"] = x * 70.0
            self.orbit["p"] = max(-72.0, min(-2.0, y * 55.0 - 8.0))
        r = self.orbit["r"]
        p = math.radians(self.orbit["p"])
        h = math.radians(self.orbit["h"])
        self.camera.setPos(
            math.sin(h) * r * math.cos(p),
            -math.cos(h) * r * math.cos(p),
            abs(math.sin(p)) * r + 4.0,
        )
        self.camera.setHpr(self.orbit["h"], self.orbit["p"], 0)
        # позиция камеры для шейдера (Френель считает виды из eye-точки)
        cp = self.camera.getPos()
        self.ocean.setShaderInput("cam_pos", cp)
        return task.cont

    def _physics_task(self, task):
        # ОДИН вызов C-ABI на кадр: тик + FFT-синтез + аналитические нормали
        t0 = time.perf_counter()
        heights, normals = self.sea.geometries(1.0 / 60.0)
        self.tphys = time.perf_counter() - t0

        n = self.sea.n
        self._geom[:, 2] = np.frombuffer(heights, dtype=np.float32)
        self._geom[:, 3:] = np.frombuffer(normals, dtype=np.float32).reshape(n * n, 3)
        self.ocean.setShaderInput("os_time", task.time)
        self.vdata.modifyArray(0).modifyHandle().setData(self._geom.tobytes())

        self.frames += 1
        if self.frames % 30 == 0:
            now = time.perf_counter()
            fps = 30 / max(now - self.t0, 1e-9)
            self.t0 = now
            self.hud.node().setText(
                f"POLER × Panda3D | {WIND} м/с | H_s {self.sea.significant_height:.2f} м"
                f" | {fps:.0f} FPS | физика {self.tphys * 1000:.1f} мс"
                f" | hash 0x{self.sea.state_hash:08X}"
            )
        return task.cont


def main():
    app = OceanApp()
    app.run()


if __name__ == "__main__":
    main()
