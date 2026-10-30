# Используем Alpine в качестве базового образа для сборки
FROM alpine:latest AS builder

# Устанавливаем зависимости (wget, xz для распаковки)
RUN apk add --no-cache wget xz

# Указываем версию Zig
ENV ZIG_VERSION=0.13.0

# Скачиваем и распаковываем Zig
RUN wget https://ziglang.org/download/${ZIG_VERSION}/zig-linux-x86_64-${ZIG_VERSION}.tar.xz && \
tar -xJf zig-linux-x86_64-${ZIG_VERSION}.tar.xz -C /usr/local && \
mv /usr/local/zig-linux-x86_64-${ZIG_VERSION} /usr/local/zig

# Добавляем Zig в PATH
ENV PATH="/usr/local/zig:$PATH"

# Финальный минимальный образ (scratch) для запуска
FROM scratch AS final
COPY --from=builder /path/to/your/compiled/binary /binary
ENTRYPOINT ["/binary"]
