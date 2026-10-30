[bits 16]
[org 0x7C00]

start:
; Отключаем прерывания и устанавливаем сегменты в 0
cli
xor ax, ax
mov ds, ax
mov es, ax
mov ss, ax
mov sp, 0x7C00

; Включаем A20 (линия адреса) - без этого не будет 64-бит
in al, 0x92
or al, 2
out 0x92, al

; Загружаем GDT (временную, чтобы переключиться в защищённый)
lgdt [gdt_desc]
mov eax, cr0
or eax, 1
mov cr0, eax

; Дальний прыжок в 32-битный код
jmp 0x08:pmode

[bits 32]
pmode:
mov ax, 0x10
mov ds, ax
mov ss, ax
mov es, ax
mov fs, ax
mov gs, ax

; Проверяем, поддерживает ли CPU 64-бит (флаг PAE)
mov eax, 0x80000001
cpuid
test edx, (1 << 29)  ; LM-бит (Long Mode)
jz hang

; Включаем PAE и создаём временную таблицу страниц
mov eax, page_table
mov cr3, eax
mov eax, cr4
or eax, (1 << 5)  ; PAE-бит
mov cr4, eax

; Включаем Long Mode (EFER)
mov ecx, 0xC0000080
rdmsr
or eax, (1 << 8)  ; LME
wrmsr

; Включаем paging
mov eax, cr0
or eax, (1 << 31)
mov cr0, eax

; Финальный прыжок в 64-битный сегмент
jmp 0x08:kmain

[bits 64]
kmain:
; Пишем прямо в видеопамять (0xB8000)
    mov qword [0xB8000], 0x2F582F58  ; 'X' белым на зелёном фоне
jmp $

hang:
jmp hang

; ---- GDT с 64-битными дескрипторами ----
gdt_start:
dq 0x0
