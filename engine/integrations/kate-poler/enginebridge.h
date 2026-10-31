/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#pragma once

#include <QObject>
#include <QString>
#include <QStringList>

#include <functional>

/**
 * EngineBridge — асинхронный мост к бинарнику poler-engine.
 *
 * Движок остаётся единственным суверенным ядром: плагин НЕ дублирует его
 * алгоритмы, а вызывает CLI (grep-json / ai-json / triune / harvest) через
 * QProcess и парсит готовый JSON. Никакой сети, никаких ключей — всё локально.
 *
 * Гарантии движка наследуются автоматически: O(1) кольцевые буферы вывода,
 * каскадные таймауты SIGTERM→SIGKILL, отсутствие зомби и TTY-дедлоков
 * (см. docs/ENGINE_EXECUTION_PROTOCOL.md в репозитории poler-engine).
 */
class EngineBridge : public QObject
{
    Q_OBJECT

public:
    using ResultCallback = std::function<void(int exitCode, const QString &stdOut, const QString &stdErr)>;
    using StderrChunk = std::function<void(const QString &chunk)>;

    explicit EngineBridge(QObject *parent = nullptr);
    ~EngineBridge() override;

    /**
     * Поиск бинарника движка: $POLER_ENGINE_BIN → ~/.local/bin/poler-engine →
     * /usr/local/bin/poler-engine → /usr/bin/poler-engine → PATH.
     * Возвращает пустую строку, если не найден.
     */
    static QString engineBinary();

    /** Движок найден и исполняем? */
    static bool available();

    /** Идет ли сейчас какой-то вызов (один активный вызов на мост). */
    bool busy() const
    {
        return m_busy;
    }

    /**
     * Запустить движок с аргументами. Колбэк вызывается один раз по завершении
     * (успех, ошибка или таймаут). Аргументы формирует вызывающая сторона —
     * движок сам гарантирует отсутствие шелл-инъекций (argv-массив).
     * @param timeoutMs 0 = без таймаута (движок сам прибьёт по своим правилам)
     * @param stderrChunk опциональный живой приёмник stderr (прогресс харвестера)
     */
    void run(const QStringList &args, ResultCallback callback, int timeoutMs = 0, StderrChunk stderrChunk = nullptr);

    /** Отменить текущий вызов (SIGTERM группе → SIGKILL — делает сам движок/QProcess). */
    void cancel();

Q_SIGNALS:
    void started();
    void finished(int exitCode);
    void failed(const QString &error);

private:
    class QProcess *m_process = nullptr;
    bool m_busy = false;
};
