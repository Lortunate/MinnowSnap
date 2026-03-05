#pragma once

#include <QKeySequence>
#include <QKeyCombination>
#include <QString>

namespace ShortcutUtils {
    inline QString getKeySequence(int key, int modifiers) {
        Qt::Key qtKey = static_cast<Qt::Key>(key);
        Qt::KeyboardModifiers qtMods = static_cast<Qt::KeyboardModifiers>(modifiers);

        if (qtKey == Qt::Key_unknown ||
            qtKey == Qt::Key_Shift ||
            qtKey == Qt::Key_Control ||
            qtKey == Qt::Key_Alt ||
            qtKey == Qt::Key_Meta ||
            qtKey == Qt::Key_Super_L ||
            qtKey == Qt::Key_Super_R) {
            return QString();
        }

        if (qtMods & Qt::ShiftModifier) {
            switch (qtKey) {
                case Qt::Key_Exclam: qtKey = Qt::Key_1; break;
                case Qt::Key_At: qtKey = Qt::Key_2; break;
                case Qt::Key_NumberSign: qtKey = Qt::Key_3; break;
                case Qt::Key_Dollar: qtKey = Qt::Key_4; break;
                case Qt::Key_Percent: qtKey = Qt::Key_5; break;
                case Qt::Key_AsciiCircum: qtKey = Qt::Key_6; break;
                case Qt::Key_Ampersand: qtKey = Qt::Key_7; break;
                case Qt::Key_Asterisk: qtKey = Qt::Key_8; break;
                case Qt::Key_ParenLeft: qtKey = Qt::Key_9; break;
                case Qt::Key_ParenRight: qtKey = Qt::Key_0; break;
                case Qt::Key_Underscore: qtKey = Qt::Key_Minus; break;
                case Qt::Key_Plus: qtKey = Qt::Key_Equal; break;
                case Qt::Key_BraceLeft: qtKey = Qt::Key_BracketLeft; break;
                case Qt::Key_BraceRight: qtKey = Qt::Key_BracketRight; break;
                case Qt::Key_Bar: qtKey = Qt::Key_Backslash; break;
                case Qt::Key_Colon: qtKey = Qt::Key_Semicolon; break;
                case Qt::Key_QuoteDbl: qtKey = Qt::Key_Apostrophe; break;
                case Qt::Key_Less: qtKey = Qt::Key_Comma; break;
                case Qt::Key_Greater: qtKey = Qt::Key_Period; break;
                case Qt::Key_Question: qtKey = Qt::Key_Slash; break;
                case Qt::Key_AsciiTilde: qtKey = Qt::Key_QuoteLeft; break;
                default: break;
            }
        }

        QKeyCombination combination(qtMods, qtKey);
        QString seq = QKeySequence(combination).toString(QKeySequence::PortableText);
        seq.replace(QStringLiteral("Meta+"), QStringLiteral("Super+"));

        return seq;
    }
}
