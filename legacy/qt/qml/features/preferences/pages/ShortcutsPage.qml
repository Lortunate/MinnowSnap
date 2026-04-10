import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow
import "../../../components"

Item {
    id: root
    property var screenCapture

    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            width: parent.width
            spacing: AppTheme.spacingLarge

            SettingCard {
                SettingItem {
                    title: qsTr("Capture")
                    description: qsTr("Select a specific region or window to capture")

                    control: ShortcutInput {
                        id: screenCaptureInput
                        hasConflict: Config.hasShortcutConflicts
                        defaultValue: "F1"
                        sequence: Config.captureShortcut
                        onCommitted: (newShortcut) => {
                            Config.updateCaptureShortcut(newShortcut)
                            if (root.screenCapture) {
                                root.screenCapture.setCaptureShortcut(newShortcut)
                            }
                            Config.checkShortcutConflicts(newShortcut, Config.quickCaptureShortcut)
                        }
                    }
                }

                Divider {}

                SettingItem {
                    title: qsTr("Quick Capture")
                    description: qsTr("Capture the entire visible screen area immediately")

                    control: ShortcutInput {
                        id: quickCaptureInput
                        hasConflict: Config.hasShortcutConflicts
                        defaultValue: "F2"
                        sequence: Config.quickCaptureShortcut
                        onCommitted: (newShortcut) => {
                            Config.updateQuickCaptureShortcut(newShortcut)
                            if (root.screenCapture) {
                                root.screenCapture.setQuickCaptureShortcut(newShortcut)
                            }
                            Config.checkShortcutConflicts(Config.captureShortcut, newShortcut)
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 0

                RowLayout {
                    spacing: AppTheme.spacingTiny
                    visible: Config.hasShortcutConflicts

                    Text {
                        font.pixelSize: AppTheme.fontSizeSmall
                        text: "⚠️"
                    }

                    Text {
                        color: AppTheme.danger
                        font.family: AppTheme.fontFamily
                        font.pixelSize: AppTheme.fontSizeSmall
                        text: Config.shortcutConflictMsg
                    }
                }

                Item {
                    Layout.fillWidth: true
                }

                StyledButton {
                    text: qsTr("Restore Defaults")
                    variant: StyledButton.Variant.Text
                    onClicked: {
                        Config.updateCaptureShortcut("F1")
                        Config.updateQuickCaptureShortcut("F2")
                        if (root.screenCapture) {
                            root.screenCapture.setCaptureShortcut("F1")
                            root.screenCapture.setQuickCaptureShortcut("F2")
                        }
                        Config.checkShortcutConflicts("F1", "F2")
                    }
                }
            }
        }
    }
}
