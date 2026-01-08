import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import com.lortunate.minnow
import "../../../components"

Item {
    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            width: parent.width
            spacing: AppTheme.spacingLarge

            // Header Section
            ColumnLayout {
                Layout.fillWidth: true
                Layout.topMargin: AppTheme.spacingSmall
                spacing: AppTheme.spacingMedium

                RowLayout {
                    Layout.alignment: Qt.AlignLeft
                    spacing: AppTheme.spacingLarge

                    Image {
                        Layout.preferredHeight: 56
                        Layout.preferredWidth: 56
                        fillMode: Image.PreserveAspectFit
                        mipmap: true
                        smooth: true
                        source: "qrc:/resources/logo.png"
                    }

                    ColumnLayout {
                        spacing: AppTheme.spacingTiny

                        Text {
                            color: AppTheme.text
                            font.family: AppTheme.fontFamily
                            font.pixelSize: 22
                            font.weight: Font.DemiBold
                            text: "MinnowSnap"
                        }

                        Text {
                            color: AppTheme.subText
                            font.family: AppTheme.fontFamily
                            font.pixelSize: AppTheme.fontSizeBody
                            text: qsTr("Version %1").arg(Config.version)
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    Layout.maximumWidth: 480
                    color: AppTheme.subText
                    font.family: AppTheme.fontFamily
                    font.pixelSize: AppTheme.fontSizeBody
                    lineHeight: 1.5
                    text: qsTr("A simple and powerful screen capture tool built with Rust and Qt.")
                    wrapMode: Text.WordWrap
                }
            }

            // Links Card
            SettingCard {
                LinkItem {
                    text: qsTr("GitHub Repository")
                    url: "https://github.com/Lortunate/MinnowSnap"
                }

                Divider {}

                LinkItem {
                    text: qsTr("Report an Issue")
                    url: "https://github.com/Lortunate/MinnowSnap/issues"
                }
            }
        }
    }
}
