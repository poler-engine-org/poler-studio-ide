// kernel/zig/src/main.zig
const hal = @import("hal.zig");

export fn long_mode_entry() callconv(.C) void {
// Инициализация HAL
hal.init();

// После этого можно использовать Serial для вывода
hal.Serial.puts("Hello from POLER-OS kernel!\n");

// Теперь инициализация PMM (физический менеджер памяти)
// ... будет позже

// Бесконечный цикл (пока нет процессов)
while (true) {
hal.hlt();
}
}
