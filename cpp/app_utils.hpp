#pragma once

#include <QCoreApplication>
#include <QGuiApplication>
#include <QString>
#include <QTranslator>
#include <QLocale>
#include <QQmlEngine>
#include <QList>
#include <QtQml>
#include <QWindow>
#include <string>
#include "rust/cxx.h"

inline void set_quit_on_last_window_closed() {
    QGuiApplication::setQuitOnLastWindowClosed(false);
}

inline QString translate(rust::Str context, rust::Str sourceText) {
    return QCoreApplication::translate(
        std::string(context.data(), context.size()).c_str(),
        std::string(sourceText.data(), sourceText.size()).c_str());
}

inline void install_translator(rust::Str localeName) {
    static QTranslator* translator = nullptr;
    if (translator) {
        QCoreApplication::removeTranslator(translator);
        delete translator;
        translator = nullptr;
    }
    translator = new QTranslator(QCoreApplication::instance());
    const QString i18nDir = QStringLiteral(":/resources/i18n/");
    QString name = QString::fromUtf8(localeName.data(), (int)localeName.size());
    QLocale locale = (name == QStringLiteral("System") || name.isEmpty()) ? QLocale::system() : QLocale(name);
    if (translator->load(locale, QString(), QString(), i18nDir)) {
        QCoreApplication::installTranslator(translator);
    } else {
        delete translator;
        translator = nullptr;
    }
}

inline void retranslate_all() {
    QList<QQmlEngine *> engines;
    for (auto *window : QGuiApplication::allWindows()) {
        if (auto *engine = qmlEngine(static_cast<QObject *>(window))) {
            if (!engines.contains(engine)) {
                engines.append(engine);
            }
        }
    }
    if (auto *app = QCoreApplication::instance()) {
        const auto foundEngines = app->findChildren<QQmlEngine *>();
        for (auto *engine : foundEngines) {
            if (!engines.contains(engine)) {
                engines.append(engine);
            }
        }
    }
    for (auto *engine : engines) {
        engine->retranslate();
    }
}
