#pragma once

#include <QCoreApplication>
#include <QGuiApplication>
#include <QString>
#include <QTranslator>
#include <QLocale>
#include <QQmlEngine>
#include <QList>
#include <QtQml>
#include <QIcon>
#include <QWindow>
#include <QCursor>
#include <QScreen>
#include <QRect>
#include <QPoint>
#include <string>
#include "rust/cxx.h"


inline void quit_app() {
    QCoreApplication::quit();
}

inline void set_window_icon() {
    QIcon icon(":/resources/logo.png");
    QGuiApplication::setWindowIcon(icon);
}

inline void set_quit_on_last_window_closed() {
    QGuiApplication::setQuitOnLastWindowClosed(false);
}

inline QString translate(rust::Str context, rust::Str sourceText) {
    return QCoreApplication::translate(
        std::string(context.data(), context.size()).c_str(),
        std::string(sourceText.data(), sourceText.size()).c_str());
}

inline int cursor_x() {
    return QCursor::pos().x();
}

inline int cursor_y() {
    return QCursor::pos().y();
}

inline QScreen* screen_at(const QPoint& point) {
    if (QScreen* screen = QGuiApplication::screenAt(point)) {
        return screen;
    }
    if (QScreen* primary = QGuiApplication::primaryScreen()) {
        return primary;
    }
    const QList<QScreen*> screens = QGuiApplication::screens();
    return screens.isEmpty() ? nullptr : screens.first();
}

inline QRect screen_geometry_at(const QPoint& point) {
    if (QScreen* screen = screen_at(point)) {
        return screen->geometry();
    }
    return QRect();
}

inline int cursor_screen_x_at(int x, int y) {
    return screen_geometry_at(QPoint(x, y)).x();
}

inline int cursor_screen_y_at(int x, int y) {
    return screen_geometry_at(QPoint(x, y)).y();
}

inline int cursor_screen_width_at(int x, int y) {
    return screen_geometry_at(QPoint(x, y)).width();
}

inline int cursor_screen_height_at(int x, int y) {
    return screen_geometry_at(QPoint(x, y)).height();
}

inline double cursor_screen_scale_at(int x, int y) {
    if (QScreen* screen = screen_at(QPoint(x, y))) {
        return screen->devicePixelRatio();
    }
    return 1.0;
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
