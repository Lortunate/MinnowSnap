import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow

Rectangle {
    id: root

    property real hue: 0
    property real sat: 1
    property color selectionColor: Qt.hsva(hue, sat, val, 1.0)
    property real val: 1

    signal accepted(color c)
    signal rejected

    function setColor(c) {
        // Qt color object has hsv properties
        root.hue = Math.max(0, c.hsvHue);
        root.sat = c.hsvSaturation;
        root.val = c.hsvValue;
    }

    border.color: AppTheme.border
    border.width: 1
    color: AppTheme.surface
    height: 220
    radius: AppTheme.radiusLarge
    width: 200

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 2
        anchors.topMargin: 2
        color: AppTheme.shadowMedium
        radius: AppTheme.radiusLarge
        z: -1
    }
    MouseArea {
        anchors.fill: parent
    }

    Column {
        anchors.centerIn: parent
        spacing: 12

        Item {
            height: 110
            width: 180

            Rectangle {
                anchors.fill: parent
                color: Qt.hsva(root.hue, 1, 1, 1)
                radius: 4
            }

            Rectangle {
                anchors.fill: parent
                radius: 4

                gradient: Gradient {
                    orientation: Gradient.Horizontal

                    GradientStop {
                        color: "#FFFFFFFF"
                        position: 0.0
                    }
                    GradientStop {
                        color: "#00FFFFFF"
                        position: 1.0
                    }
                }
            }

            Rectangle {
                anchors.fill: parent
                radius: AppTheme.radiusSmall

                gradient: Gradient {
                    orientation: Gradient.Vertical

                    GradientStop {
                        color: "#00000000"
                        position: 0.0
                    }
                    GradientStop {
                        color: "#FF000000"
                        position: 1.0
                    }
                }
            }

            Rectangle {
                border.color: AppTheme.handleFill
                border.width: 1
                color: "transparent"
                height: 12
                radius: 6
                width: 12
                x: Math.max(0, Math.min(parent.width, root.sat * parent.width)) - width / 2
                y: Math.max(0, Math.min(parent.height, (1 - root.val) * parent.height)) - height / 2

                Rectangle {
                    anchors.centerIn: parent
                    border.color: AppTheme.handleInner
                    border.width: 1
                    color: "transparent"
                    height: 10
                    radius: 5
                    width: 10
                }
            }
            MouseArea {
                function update(mouse) {
                    root.sat = Math.max(0, Math.min(1, mouse.x / width));
                    root.val = 1 - Math.max(0, Math.min(1, mouse.y / height));
                }

                anchors.fill: parent

                onPositionChanged: mouse => update(mouse)
                onPressed: mouse => update(mouse)
            }
        }

        Item {
            height: 10
            width: 180

            Rectangle {
                anchors.fill: parent
                radius: 5

                gradient: Gradient {
                    orientation: Gradient.Horizontal

                    GradientStop {
                        color: "#FF0000"
                        position: 0.00
                    }
                    GradientStop {
                        color: "#FFFF00"
                        position: 0.17
                    }
                    GradientStop {
                        color: "#00FF00"
                        position: 0.33
                    }
                    GradientStop {
                        color: "#00FFFF"
                        position: 0.50
                    }
                    GradientStop {
                        color: "#0000FF"
                        position: 0.67
                    }
                    GradientStop {
                        color: "#FF00FF"
                        position: 0.83
                    }
                    GradientStop {
                        color: "#FF0000"
                        position: 1.00
                    }
                }
            }

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                border.color: AppTheme.handleFill
                border.width: 2
                color: Qt.hsva(root.hue, 1, 1, 1)
                height: 14

                layer.enabled: true
                radius: 7
                width: 14
                x: Math.max(0, Math.min(parent.width, root.hue * parent.width)) - width / 2
            }
            MouseArea {
                function update(mouse) {
                    root.hue = Math.max(0, Math.min(1, mouse.x / width));
                }

                anchors.fill: parent

                onPositionChanged: mouse => update(mouse)
                onPressed: mouse => update(mouse)
            }
        }

        RowLayout {
            spacing: 8
            width: 180

            Rectangle {
                Layout.preferredHeight: 24
                Layout.preferredWidth: 24
                border.color: AppTheme.border
                border.width: 1
                color: root.selectionColor
                radius: 12
            }
            Item {
                Layout.fillWidth: true
            }

            Rectangle {
                Layout.preferredHeight: 24
                Layout.preferredWidth: 50
                border.color: AppTheme.border
                border.width: 1
                color: cancelArea.pressed ? AppTheme.buttonBgDown : AppTheme.surface
                radius: AppTheme.radiusSmall

                Text {
                    anchors.centerIn: parent
                    color: AppTheme.text
                    font.pixelSize: 11
                    text: qsTr("Cancel")
                }
                MouseArea {
                    id: cancelArea

                    anchors.fill: parent

                    onClicked: root.rejected()
                }
            }

            Rectangle {
                Layout.preferredHeight: 24
                Layout.preferredWidth: 50
                color: confirmArea.pressed ? Qt.darker(AppTheme.primary, 1.1) : AppTheme.primary
                radius: 4

                Text {
                    anchors.centerIn: parent
                    color: AppTheme.primaryText
                    font.bold: true
                    font.pixelSize: 11
                    text: qsTr("OK")
                }
                MouseArea {
                    id: confirmArea

                    anchors.fill: parent

                    onClicked: root.accepted(root.selectionColor)
                }
            }
        }
    }
}
