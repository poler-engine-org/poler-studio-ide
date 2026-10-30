// В system_core.h:
typedef struct {
// ... существующие поля ...
    int link_capacity;   // выделенная память
} System;

// В system_core.c:
void sys_init(System* sys, int element_count, int link_count_estimate) {
// ... остальное ...
sys->link_capacity = link_count_estimate * 1.2; // запас 20%
sys->links = (Link*)malloc(sys->link_capacity * sizeof(Link));
    sys->link_count = 0;  // начинаем с нуля
}

void sys_add_link(System* sys, int src, int dst, float weight, int delay) {
if (sys->link_count >= sys->link_capacity) {
sys->link_capacity *= 2;
sys->links = realloc(sys->links, sys->link_capacity * sizeof(Link));
}
Link* ln = &sys->links[sys->link_count++];
ln->src = src;
ln->dst = dst;
ln->weight = weight;
ln->delay = delay;
ln->head = 0;
ln->buffer = (float*)calloc(delay, sizeof(float));
}

Теперь create_links реально заполняет связи:

c
