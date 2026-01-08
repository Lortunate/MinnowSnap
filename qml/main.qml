import QtQuick
import QtQuick.Controls
import Qt.labs.platform
import com.lortunate.minnow
import "features/capture"
import "features/tray"

ApplicationWindow {
    id: root

    height: 0
    visible: false
    width: 0

    function showPreferences() {
        if (preferencesLoader.active) {
            preferencesLoader.item.show();
            preferencesLoader.item.raise();
            preferencesLoader.item.requestActivate();
        } else {
            preferencesLoader.active = true;
        }
    }

    Loader {
        id: preferencesLoader
        active: false
        source: "features/preferences/PreferencesWindow.qml"

        onLoaded: {
            item.screenCapture = screenCapture;
            item.show();
            item.raise();
            item.requestActivate();
        }

        // Unload the window when closed to reset state
        Connections {
            function onVisibleChanged() {
                if (!preferencesLoader.item.visible) {
                    preferencesLoader.active = false;
                }
            }

            ignoreUnknownSignals: true
            target: preferencesLoader.item
        }
    }
    ScreenCapture {
        id: screenCapture

        Component.onCompleted: {
            screenCapture.registerHotkeys();
        }

        onQuickCaptureShortcutTriggered: quickCapture(0, 0, 0, 0)
        onScreenCaptureShortcutTriggered: {
            if (!overlay.visible) {
                prepareCapture();
            }
        }
    }

    CaptureOverlay {
        id: overlay
        screenCapture: screenCapture
    }

    Action {
        id: actPreferences
        text: qsTr("Preferences")
        onTriggered: root.showPreferences()
    }

    Action {
        id: actScreenCapture
        text: qsTr("Capture")
        onTriggered: {
            if (!overlay.visible) {
                screenCapture.prepareCapture();
            }
        }
    }

    Action {
        id: actQuickCapture
        text: qsTr("Quick Capture")
        onTriggered: screenCapture.quickCapture(0, 0, 0, 0)
    }

    Action {
        id: actQuit
        text: qsTr("Exit")
        onTriggered: Qt.quit()
    }

    TrayMenu {
        id: trayMenu
        preferencesAction: actPreferences
        quickCaptureAction: actQuickCapture
        screenCaptureAction: actScreenCapture
        quitAction: actQuit
    }

    Menu {
        id: nativeMenu
        MenuItem {
            text: actPreferences.text
            onTriggered: actPreferences.trigger()
        }
        MenuSeparator {}
        MenuItem {
            text: actScreenCapture.text
            onTriggered: actScreenCapture.trigger()
        }
        MenuItem {
            text: actQuickCapture.text
            onTriggered: actQuickCapture.trigger()
        }
        MenuSeparator {}
        MenuItem {
            text: actQuit.text
            onTriggered: actQuit.trigger()
        }
    }

    SystemTrayIcon {
        id: trayIcon

        icon.mask: true
        icon.source: "qrc:/resources/tray.svg"
        tooltip: qsTr("MinnowSnap - Screen Capture Tool")
        visible: true

        menu: Qt.platform.os === "osx" ? nativeMenu : null

        onActivated: function (reason) {
            if (reason === SystemTrayIcon.DoubleClick) {
                root.showPreferences();
            } else if (Qt.platform.os !== "osx") {
                if (reason === SystemTrayIcon.Trigger || reason === SystemTrayIcon.Context) {
                    trayMenu.popup(trayIcon.geometry);
                }
            }
        }
    }
}
