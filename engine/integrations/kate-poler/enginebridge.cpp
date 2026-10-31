/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#include "enginebridge.h"

#include <QFileInfo>
#include <QProcess>
#include <QStandardPaths>
#include <QTimer>

#include <memory>

EngineBridge::EngineBridge(QObject *parent)
    : QObject(parent)
{
}

EngineBridge::~EngineBridge()
{
    cancel();
}

QString EngineBridge::engineBinary()
{
    // 1. Явное переопределение окружением.
    if (const QString env = qEnvironmentVariable("POLER_ENGINE_BIN"); !env.isEmpty()) {
        return env;
    }
    // 2. Канонические места установки движка (см. docs/skills/poler-engine.md).
    const QStringList candidates = {
        QStringLiteral("%1/.local/bin/poler-engine").arg(qEnvironmentVariable("HOME")),
        QStringLiteral("/usr/local/bin/poler-engine"),
        QStringLiteral("/usr/bin/poler-engine"),
    };
    for (const QString &c : candidates) {
        if (QFileInfo::exists(c) && QFileInfo(c).isExecutable()) {
            return c;
        }
    }
    // 3. PATH.
    return QStandardPaths::findExecutable(QStringLiteral("poler-engine"));
}

bool EngineBridge::available()
{
    const QString bin = engineBinary();
    return !bin.isEmpty() && QFileInfo(bin).isExecutable();
}

void EngineBridge::run(const QStringList &args, ResultCallback callback, int timeoutMs, StderrChunk stderrChunk)
{
    if (m_busy) {
        if (callback) {
            callback(-1, QString(), QStringLiteral("engine busy: previous call in flight"));
        }
        return;
    }
    const QString bin = engineBinary();
    if (bin.isEmpty()) {
        if (callback) {
            callback(127,
                     QString(),
                     QStringLiteral("poler-engine not found — install to ~/.local/bin or set $POLER_ENGINE_BIN"));
        }
        Q_EMIT failed(QStringLiteral("poler-engine not found"));
        return;
    }

    m_busy = true;
    m_process = new QProcess(this);
    m_process->setProgram(bin);
    m_process->setArguments(args);
    // Раздельные каналы: stdout — данные (JSON), stderr — прогресс/диагностика.
    m_process->setProcessChannelMode(QProcess::SeparateChannels);
    m_process->setWorkingDirectory(qEnvironmentVariable("HOME"));

    Q_EMIT started();

    // Один вызов колбэка — по завершении процесса ИЛИ таймауту.
    // shared_ptr: лямбды живут дольше кадра run(), захват по ссылке запрещён.
    auto *proc = m_process;
    auto called = std::make_shared<bool>(false);

    // Живой прогресс (харвестер пишет сводку в stderr).
    if (stderrChunk) {
        connect(proc, &QProcess::readyReadStandardError, this, [proc, stderrChunk] {
            stderrChunk(QString::fromUtf8(proc->readAllStandardError()));
        });
    }

    QTimer *timeout = nullptr;
    if (timeoutMs > 0) {
        timeout = new QTimer(proc);
        timeout->setSingleShot(true);
        connect(timeout, &QTimer::timeout, proc, [proc] {
            // QProcess делает graceful: SIGTERM всей группе, затем SIGKILL.
            proc->terminate();
            QTimer::singleShot(2000, proc, [proc] {
                if (proc->state() != QProcess::NotRunning) {
                    proc->kill();
                }
            });
        });
    }

    connect(proc, &QProcess::errorOccurred, this, [this, proc, callback, called](QProcess::ProcessError err) {
        if (*called) {
            return;
        }
        *called = true;
        m_busy = false;
        const QString msg = QStringLiteral("poler-engine process error: %1").arg(int(err));
        if (callback) {
            callback(-2, QString(), msg);
        }
        Q_EMIT failed(msg);
        proc->deleteLater();
    });

    connect(proc,
            &QProcess::finished,
            this,
            [this, proc, callback, called](int exitCode, QProcess::ExitStatus) {
                if (*called) {
                    return;
                }
                *called = true;
                m_busy = false;
                const QString stdOut = QString::fromUtf8(proc->readAllStandardOutput());
                const QString stdErr = QString::fromUtf8(proc->readAllStandardError());
                if (callback) {
                    callback(exitCode, stdOut, stdErr);
                }
                Q_EMIT finished(exitCode);
                proc->deleteLater();
            });

    if (timeout) {
        timeout->start(timeoutMs);
    }
    m_process->start();
}

void EngineBridge::cancel()
{
    if (m_process && m_process->state() != QProcess::NotRunning) {
        m_process->terminate();
        QTimer::singleShot(2000, m_process, [p = m_process] {
            if (p->state() != QProcess::NotRunning) {
                p->kill();
            }
        });
    }
}

#include "moc_enginebridge.cpp"
