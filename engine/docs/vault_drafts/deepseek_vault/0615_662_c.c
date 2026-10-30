float interaction_strength = (src->archetype == dst->archetype) ? -1.0f : 1.0f;
interaction_strength *= expf(-distance / (src->width + dst->width));
🧩 3. НОВАЯ СТРУКТУРА СИСТЕМЫ (SystemV3)
c
