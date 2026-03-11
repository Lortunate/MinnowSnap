import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow
import "pages"

Window {
    id: preferencesWindow

    property var screenCapture: null

    visible: false
    width: 640
    height: 440
    title: qsTr("Preferences")
    color: (Qt.platform.os === "osx") ? AppTheme.background : "transparent"

    flags: (Qt.platform.os === "osx") ? (Qt.Window | Qt.WindowTitleHint | Qt.WindowSystemMenuHint | Qt.WindowCloseButtonHint) : (Qt.Window | Qt.FramelessWindowHint)

    Component.onCompleted: {
        x = (Screen.width - width) / 2;
        y = (Screen.height - height) / 2;
        WindowHelper.setupUnifiedTitlebar(preferencesWindow);
    }

    Component {
        id: extendedHeader
        RowLayout {
            anchors.fill: parent
            spacing: AppTheme.spacingSmall
            
            Item { width: AppTheme.spacingMedium }

            Image {
                source: "qrc:/resources/logo.png"
                Layout.preferredWidth: 20
                Layout.preferredHeight: 20
                Layout.alignment: Qt.AlignVCenter
                smooth: true
                mipmap: true
                fillMode: Image.PreserveAspectFit
            }

            Label {
                text: qsTr("Preferences")
                font.family: AppTheme.fontFamily
                font.pixelSize: AppTheme.fontSizeBody + 1
                font.weight: Font.DemiBold
                color: AppTheme.text
                Layout.alignment: Qt.AlignVCenter
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }

    onClosing: function (close) {
        close.accepted = false;
        preferencesWindow.hide();
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 160
            color: AppTheme.cardBackground
            radius: (Qt.platform.os === "osx") ? 0 : AppTheme.radiusXLarge

            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.radius
                color: parent.color
                visible: parent.radius > 0
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 40

                    DragHandler {
                        target: null
                        onActiveChanged: if (active)
                            preferencesWindow.startSystemMove()
                    }

                    Loader {
                        anchors.fill: parent
                        sourceComponent: extendedHeader 
                        visible: Qt.platform.os !== "osx"
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: AppTheme.divider
                    opacity: 0.1
                    visible: Qt.platform.os !== "osx"
                }

                ListView {
                    id: sidebarList
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.topMargin: AppTheme.spacingMedium
                    Layout.bottomMargin: AppTheme.spacingMedium
                    spacing: 4
                    clip: true
                    interactive: false

                    model: [qsTr("General"), qsTr("Shortcuts"), qsTr("Notifications"), qsTr("OCR"), qsTr("About")]

                    delegate: ItemDelegate {
                        id: delegateControl

                        required property int index
                        required property string modelData
                        property bool isSelected: sidebarList.currentIndex === index

                        width: sidebarList.width
                        height: AppTheme.sidebarItemHeight
                        padding: 0
                        hoverEnabled: true
                        text: modelData
                        font.family: AppTheme.fontFamily
                        font.pixelSize: AppTheme.fontSizeBody
                        font.weight: isSelected ? Font.Medium : Font.Normal

                        background: Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: AppTheme.spacingSmall
                            anchors.rightMargin: AppTheme.spacingSmall
                            anchors.topMargin: 1
                            anchors.bottomMargin: 1
                            radius: AppTheme.radiusMedium
                            color: {
                                if (delegateControl.isSelected)
                                    return AppTheme.itemSelected;
                                if (delegateControl.down)
                                    return AppTheme.itemPress;
                                if (delegateControl.hovered)
                                    return AppTheme.itemHover;
                                return "transparent";
                            }
                        }

                        contentItem: Text {
                            text: delegateControl.text
                            font: delegateControl.font
                            color: delegateControl.isSelected || delegateControl.hovered ? AppTheme.text : AppTheme.subText
                            verticalAlignment: Text.AlignVCenter
                            leftPadding: AppTheme.spacingLarge
                            rightPadding: AppTheme.spacingSmall
                            elide: Text.ElideRight
                        }

                        onClicked: sidebarList.currentIndex = index
                    }
                }
            }

            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 1
                color: AppTheme.divider
                opacity: 0.2
            }
        }

        Rectangle {
            Layout.fillHeight: true
            Layout.fillWidth: true
            color: AppTheme.formBackground
            radius: (Qt.platform.os === "osx") ? 0 : AppTheme.radiusXLarge

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.radius
                color: parent.color
                visible: parent.radius > 0
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 40

                    DragHandler {
                        target: null
                        onActiveChanged: if (active)
                            preferencesWindow.startSystemMove()
                    }

                    RowLayout {
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        spacing: 0
                        visible: Qt.platform.os !== "osx"

                        Button {
                            Layout.fillHeight: true
                            Layout.preferredWidth: 46
                            display: AbstractButton.IconOnly
                            flat: true

                            icon.source: "qrc:/resources/icons/close.svg"
                            icon.width: 10
                            icon.height: 10
                            icon.color: hovered ? "white" : AppTheme.text

                            background: Rectangle {
                                color: parent.down ? "#B71C1C" : (parent.hovered ? "#E81123" : "transparent")
                                radius: (Qt.platform.os !== "osx" && (parent.hovered || parent.down)) ? AppTheme.radiusXLarge : 0

                                Rectangle {
                                    anchors.left: parent.left
                                    anchors.top: parent.top
                                    anchors.bottom: parent.bottom
                                    width: parent.radius
                                    color: parent.color
                                    visible: parent.radius > 0
                                }

                                Rectangle {
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.bottom: parent.bottom
                                    height: parent.radius
                                    color: parent.color
                                    visible: parent.radius > 0
                                }
                            }

                            onClicked: preferencesWindow.close()
                        }
                    }
                }

                StackLayout {
                    id: pagesStack
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.leftMargin: AppTheme.spacingLarge
                    Layout.rightMargin: AppTheme.spacingLarge
                    Layout.bottomMargin: AppTheme.spacingLarge
                    Layout.topMargin: AppTheme.spacingMedium
                    currentIndex: sidebarList.currentIndex

                    GeneralPage {
                    }

                    ShortcutsPage {
                        screenCapture: preferencesWindow.screenCapture
                    }

                    NotificationPage {}

                    OcrPage {}

                    AboutPage {}
                }
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.color: AppTheme.border
        border.width: 1
        radius: AppTheme.radiusXLarge
        visible: Qt.platform.os !== "osx"
    }
}
