import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.platform
import QtCore
import com.lortunate.minnow
import "../../../components"

Item {
    id: root

    property var screenCapture

    function stripFileProtocol(path) {
        if (path.startsWith("file://")) {
            return path.substring(7);
        }
        return path;
    }

    FolderDialog {
        id: folderDialog
        currentFolder: Config.savePath !== "" ? "file://" + Config.savePath : StandardPaths.standardLocations(StandardPaths.PicturesLocation)[0]
        title: qsTr("Select Save Directory")

        onAccepted: {
            var path = stripFileProtocol(folderDialog.folder.toString());
            Config.updateSavePath(path);
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            width: parent.width
            spacing: AppTheme.spacingMedium

            SettingCard {
                SettingItem {
                    title: qsTr("Language")
                    description: qsTr("Choose the display language of the application.")

                    control: StyledComboBox {
                        id: langCombo

                        property var values: Config.getSupportedLanguages()

                        currentIndex: values.indexOf(Config.language)
                        model: [qsTr("Follow System"), "简体中文", "English (US)"]

                        onActivated: index => Config.updateLanguage(values[index])
                    }
                }

                Divider {}

                SettingItem {
                    title: qsTr("Theme")
                    description: qsTr("Choose the appearance of the application.")

                    control: StyledComboBox {
                        id: themeCombo

                        property var values: ["System", "Light", "Dark"]

                        currentIndex: values.indexOf(Config.theme)
                        model: [qsTr("Follow System"), qsTr("Light"), qsTr("Dark")]

                        onActivated: index => Config.updateTheme(values[index])
                    }
                }

                Divider {}

                SettingItem {
                    title: qsTr("App Font")
                    description: qsTr("Choose the font family for the application.")

                    control: StyledComboBox {
                        id: fontCombo
                        property var fontList: Config.getSystemFonts()

                        currentIndex: fontList.indexOf(Config.fontFamily)
                        model: fontList
                        previewFonts: true

                        onActivated: index => {
                            if (index >= 0 && index < fontList.length) {
                                Config.updateFontFamily(fontList[index]);
                            }
                        }
                    }
                }
            }
            SettingCard {
                SettingItem {
                    title: qsTr("Save Directory")
                    description: Config.savePath !== "" ? Config.savePath : Config.getDefaultSavePath()

                    control: StyledButton {
                        text: qsTr("Browse")
                        variant: StyledButton.Variant.Outline
                        onClicked: folderDialog.open()
                    }
                }

                Divider {}

                SettingItem {
                    title: qsTr("Image Compression")
                    description: qsTr("Optimize saved images using Oxipng. Disabling this improves saving speed but increases file size.")

                    control: Switch {
                        checked: Config.oxipngEnabled
                        onCheckedChanged: Config.updateOxipngEnabled(checked)
                    }
                }
            }
        }
    }
}
