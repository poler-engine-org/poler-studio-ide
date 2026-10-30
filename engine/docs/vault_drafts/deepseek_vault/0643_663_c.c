// Вместо одного массива — динамическое добавление
typedef struct {
Link* links;
int link_count;
int link_capacity;  // ← добавить
} System;

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
ln->buffer = calloc(delay, sizeof(float));
}
