---
name: qemu-rsp-debugger
description: >-
  Автономная отладка QEMU без root и gdb через GDB Remote Serial Protocol (RSP).
  Используй для установки hardware watchpoints/breakpoints на память (0xAAAA poison), чтения регистров CPU и трассировки RIP через сырой TCP-сокет.
---

# 🛰️ QEMU RSP Debugger — Rootless Remote Serial Protocol Client

Навык **qemu-rsp-debugger** обучает агента взаимодействовать со встроенным GDB-стабом QEMU (`-gdb tcp::1234 -S`) напрямую через сырые TCP-сокеты (Python/C/Zig) без необходимости установки `gdb` и без прав `root`.

---

## 🎯 Сценарии применения
1. **Поиск писателя в память (Hardware Watchpoint на 0xAAAA):**
   Отслеживание точного места, где затирается стек или пишется `0xAAAA` (`Z2,<addr>,<len>`).
2. **Снятие состояния регистров без паники ядра:**
   Пакет `g` — получение всех 64-битных регистров (RAX..R15, RIP, RFLAGS, RSP, CR0..CR4).
3. **Пошаговое исполнение (Single Step) после прерывания:**
   Пакет `s` — шаг на 1 инструкцию для верификации выходов `sysretq`/`iretq`.

---

## 🛠️ Минимальный эталонный RSP-клиент (Python/Sockets)

```python
import socket

def checksum(data: str) -> str:
    return f"{sum(data.encode('ascii')) % 256:02x}"

def send_packet(s: socket.socket, cmd: str) -> str:
    pkt = f"${cmd}#{checksum(cmd)}"
    s.sendall(pkt.encode('ascii'))
    ack = s.recv(1) # '+'
    resp = s.recv(4096).decode('latin1')
    s.sendall(b'+')
    return resp

def attach_and_watch(host='127.0.0.1', port=1234, watch_addr=0x4002EED388):
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect((host, port))
    
    # 1. Handshake & Extended Mode
    send_packet(s, "qSupported:multiprocess+;swbreak+;hwbreak+")
    send_packet(s, "?")
    
    # 2. Поставить Hardware Write Watchpoint (Z2,addr,length)
    resp = send_packet(s, f"Z2,{watch_addr:x},8")
    print(f"[*] Watchpoint set on 0x{watch_addr:x}: {resp}")
    
    # 3. Продолжить исполнение (Continue)
    print("[*] Resuming execution until write hit...")
    hit = send_packet(s, "c")
    print(f"[!] WATCHPOINT HIT! Stop packet: {hit}")
    
    # 4. Прочитать регистры (g)
    regs = send_packet(s, "g")
    print(f"[*] Registers raw dump: {regs}")
```
