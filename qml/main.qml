import QtQuick
import QtQuick.Controls.Basic
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

        onQuickCaptureShortcutTriggered: quickCapture(Qt.rect(0, 0, 0, 0))
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

    TrayMenu {
        id: trayMenu
        onPreferencesRequested: root.showPreferences()
        onScreenCaptureRequested: {
            if (!overlay.visible) {
                screenCapture.prepareCapture();
            }
        }
        onQuickCaptureRequested: screenCapture.quickCapture(Qt.rect(0, 0, 0, 0))
        onQuitRequested: Qt.quit()
    }

    Menu {
        id: nativeMenu
        MenuItem {
            text: trayMenu.controller.preferencesText
            onTriggered: root.showPreferences()
        }
        MenuSeparator {}
        MenuItem {
            text: trayMenu.controller.screenCaptureText
            onTriggered: {
                if (!overlay.visible) {
                    screenCapture.prepareCapture();
                }
            }
        }
        MenuItem {
            text: trayMenu.controller.quickCaptureText
            onTriggered: screenCapture.quickCapture(Qt.rect(0, 0, 0, 0))
        }
        MenuSeparator {}
        MenuItem {
            text: trayMenu.controller.quitText
            onTriggered: Qt.quit()
        }
    }

    SystemTrayIcon {
        id: trayIcon
        icon.mask: Qt.platform.os === "osx"
        icon.source: Qt.platform.os === "osx" ? "qrc:/resources/tray_black.svg" : (AppTheme.systemIsDark ? "qrc:/resources/tray_white.svg" : "qrc:/resources/tray_black.svg")
        tooltip: trayMenu.controller.tooltipText
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
