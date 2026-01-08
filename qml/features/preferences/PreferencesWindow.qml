import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow
import "pages"

Window {
    id: preferencesWindow

    property var screenCapture: null

    // Window setup
    visible: false
    width: 640
    height: 440
    title: qsTr("Preferences")
    color: AppTheme.background

    flags: Qt.Window | Qt.WindowTitleHint | Qt.WindowSystemMenuHint | Qt.WindowCloseButtonHint

    Component.onCompleted: {
        x = (Screen.width - width) / 2;
        y = (Screen.height - height) / 2;
        WindowHelper.setupUnifiedTitlebar(preferencesWindow);
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

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                // Draggable Title Area for Sidebar (Traffic Lights)
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 40

                    DragHandler {
                        target: null
                        onActiveChanged: if (active)
                            preferencesWindow.startSystemMove()
                    }
                }

                ListView {
                    id: sidebarList
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.topMargin: AppTheme.spacingMedium
                    Layout.bottomMargin: AppTheme.spacingMedium
                    spacing: 2
                    clip: true
                    interactive: false

                    model: [qsTr("General"), qsTr("Shortcuts"), qsTr("About")]

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
                        screenCapture: preferencesWindow.screenCapture
                    }

                    ShortcutsPage {
                        screenCapture: preferencesWindow.screenCapture
                    }

                    AboutPage {}
                }
            }
        }
    }
}
