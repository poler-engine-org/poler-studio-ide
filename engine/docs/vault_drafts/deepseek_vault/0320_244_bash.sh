# Сборка в сырой бинарник (без заголовков ELF)
nasm -f bin boot.asm -o boot.bin

# Запуск в QEMU (без эмуляции диска, грузим напрямую)
qemu-system-x86_64 -drive format=raw,file=boot.bin
