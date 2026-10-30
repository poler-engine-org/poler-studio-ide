// ИСПРАВЛЕНО в v2.0:
foreach zone:
propagate_signals_from(zone)
generate_spikes_in(next_zone)  // ← Правильный порядок!
