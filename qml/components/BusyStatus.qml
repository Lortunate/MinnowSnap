import QtQuick
import com.lortunate.minnow

Rectangle {
    id: root

    property bool running: false
    property string text: qsTr("Processing...")

    border.color: AppTheme.border
    border.width: 1
    color: AppTheme.surface
    height: 44
    radius: 22
    visible: running
    width: Math.max(140, label.contentWidth + 50)

    // Shadow effect
    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 2
        anchors.topMargin: 2
        color: AppTheme.shadowColor
        radius: 22
        z: -1
    }
    Row {
        anchors.centerIn: parent
        spacing: 12

        // Spinner
        Item {
            height: 20
            width: 20

            RotationAnimator on rotation {
                duration: 1000
                from: 0
                loops: Animation.Infinite
                running: root.running
                to: 360
            }

            Canvas {
                anchors.fill: parent

                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    ctx.beginPath();
                    ctx.arc(10, 10, 8, 0, Math.PI * 1.5);
                    ctx.lineWidth = 2;
                    ctx.strokeStyle = AppTheme.primary;
                    ctx.stroke();
                }
            }
        }
        Text {
            id: label

            color: AppTheme.text
            font.family: AppTheme.fontFamily
            font.pixelSize: AppTheme.fontSizeBody
            font.weight: Font.DemiBold
            text: root.text
            verticalAlignment: Text.AlignVCenter
        }
    }
}
