import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow
import "../../../components"

Item {
    id: root

    OcrManager {
        id: ocrManager
        Component.onCompleted: init()
    }

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
                    title: qsTr("Enable OCR")
                    description: qsTr("Enable Optical Character Recognition to detect text in screenshots.")

                    control: Switch {
                        checked: ocrManager.enabled
                        onCheckedChanged: ocrManager.setOcrEnabledPersist(checked)
                    }
                }
            }

            SettingCard {
                visible: ocrManager.enabled
                Layout.fillWidth: true

                SettingItem {
                    title: qsTr("OCR Models")
                    description: ocrManager.statusMessage

                    control: StyledButton {
                        text: ocrManager.isDownloading ? qsTr("Downloading...") : (ocrManager.isModelReady ? qsTr("Redownload") : qsTr("Download"))
                        variant: StyledButton.Variant.Outline
                        enabled: !ocrManager.isDownloading
                        onClicked: ocrManager.downloadModels()
                    }
                }
            }

            // Info text
            Text {
                Layout.fillWidth: true
                Layout.leftMargin: AppTheme.spacingSmall
                Layout.rightMargin: AppTheme.spacingSmall
                color: AppTheme.subText
                font.family: AppTheme.fontFamily
                font.pixelSize: AppTheme.fontSizeSmall
                wrapMode: Text.WordWrap
                text: qsTr("Note: OCR runs locally on your device. The 'Mobile' models are used by default for performance.")
                visible: ocrManager.enabled
            }
        }
    }
}
