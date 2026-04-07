#include "MinnowSnap/src/bridge/app.cxx.h"

#include <QByteArray>
#include <QMessageLogContext>
#include <QString>
#include <QtGlobal>

namespace {
QtMessageHandler previous_message_handler = nullptr;
thread_local bool forwarding_qt_message = false;

rust::Str to_rust_str(const QByteArray& value) {
    return rust::Str(value.constData(), static_cast<std::size_t>(value.size()));
}

void qt_message_forwarder(QtMsgType type, const QMessageLogContext& context, const QString& message) {
    if (forwarding_qt_message) {
        if (previous_message_handler != nullptr) {
            previous_message_handler(type, context, message);
        }
        return;
    }

    forwarding_qt_message = true;

    const QByteArray category_utf8(context.category != nullptr ? context.category : "");
    const QByteArray message_utf8 = message.toUtf8();
    const QByteArray file_utf8(context.file != nullptr ? context.file : "");

    push_qt_log(
        static_cast<::std::int32_t>(type),
        to_rust_str(category_utf8),
        to_rust_str(message_utf8),
        to_rust_str(file_utf8),
        static_cast<::std::int32_t>(context.line));

    if (previous_message_handler != nullptr && previous_message_handler != qt_message_forwarder) {
        previous_message_handler(type, context, message);
    }

    forwarding_qt_message = false;
}
} // namespace

void install_qt_message_handler() {
    previous_message_handler = qInstallMessageHandler(qt_message_forwarder);
}
