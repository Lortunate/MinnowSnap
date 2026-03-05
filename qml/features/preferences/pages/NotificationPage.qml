import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow
import "../../../components"

Item {
    id: root

    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            width: parent.width
            spacing: AppTheme.spacingMedium

            SettingCard {
                SettingItem {
                    title: qsTr("Enable Notifications")
                    description: qsTr("Show system notifications for all actions.")

                    control: StyledSwitch {
                        checked: Config.notificationEnabled
                        onCheckedChanged: Config.updateNotificationEnabled(checked)
                    }
                }

                Divider {}

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    
                    enabled: Config.notificationEnabled
                    opacity: Config.notificationEnabled ? 1.0 : 0.5
                    
                    Behavior on opacity {
                        NumberAnimation { duration: 200; easing.type: Easing.InOutQuad }
                    }

                    SettingItem {
                        title: qsTr("Save Notification")
                        description: qsTr("Show notification when image is saved.")

                        control: StyledSwitch {
                            checked: Config.saveNotification
                            onCheckedChanged: Config.updateSaveNotification(checked)
                        }
                    }

                    Divider {}

                    SettingItem {
                        title: qsTr("Copy Notification")
                        description: qsTr("Show notification when content is copied to clipboard.")

                        control: StyledSwitch {
                            checked: Config.copyNotification
                            onCheckedChanged: Config.updateCopyNotification(checked)
                        }
                    }
                }
            }

            SettingCard {
                SettingItem {
                    title: qsTr("Shutter Sound")
                    description: qsTr("Play a sound effect when capturing.")

                    control: StyledSwitch {
                        checked: Config.shutterSound
                        onCheckedChanged: Config.updateShutterSound(checked)
                    }
                }
            }
        }
    }
}
