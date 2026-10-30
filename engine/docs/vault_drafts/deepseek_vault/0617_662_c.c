void sys_v3_step(SystemV3* sys) {
// 1. Диссипация (затухание) на всех уровнях
for (int lvl = 0; lvl < sys->level_count; lvl++) {
level_dissipate(sys, lvl);
}

// 2. Применение внешнего поля (входной уровень)
apply_external_field(sys);

// 3. Восходящая композиция: снизу вверх
for (int lvl = 0; lvl < sys->level_count - 1; lvl++) {
// Взаимодействия от lvl к lvl+1
propagate_interactions(sys, lvl, lvl+1);
// Коллапс (активация) на уровне lvl+1
level_collapse(sys, lvl+1);
}

// 4. Нисходящие связи (top-down attention) — опционально
// ...

// 5. Обновление эмерджентного времени
update_emergent_time(sys);

// 6. Адаптация когнитивной метрики
update_metric(sys);
}
