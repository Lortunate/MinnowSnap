#include <QtQml/qqmldebug.h>

extern "C" int minnowsnap_main();

int main() {
#if defined(QT_QML_DEBUG)
    QQmlDebuggingEnabler::enableDebugging(true);
#endif
    return minnowsnap_main();
}
