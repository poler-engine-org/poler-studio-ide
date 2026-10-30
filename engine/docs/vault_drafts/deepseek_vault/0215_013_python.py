# Простая интеграция - по очереди
class SimpleHybridBrain:
def __init__(self):
self.vortex = SynapticVortexV3(...)
self.poler = IntegratedPolerCore(...)
self.bridge = SimpleBridge(...)

def think_step(self, input_text):
# 1. Текст → POLER
poler_state = self.poler.text_to_poler(input_text)

# 2. POLER → Vortex (стимуляция)
vortex_pattern = self.bridge.to_vortex(poler_state)
self.vortex.stimulate(vortex_pattern)

# 3. Vortex работает 10 циклов
for _ in range(10):
self.vortex.cycle()

# 4. Vortex → POLER (обратная связь)
vortex_activity = self.vortex.get_activations()
new_poler_state = self.bridge.to_poler(vortex_activity)

# 5. Обновление POLER
self.poler.integrate(new_poler_state)

return self.poler.generate_response()
