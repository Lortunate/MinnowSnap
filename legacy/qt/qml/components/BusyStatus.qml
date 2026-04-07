import QtQuick
import com.lortunate.minnow

Rectangle {
    id: root

    property bool running: false
    property string text: qsTr("Processing...")

    border.color: AppTheme.border
    border.width: 1
    color: AppTheme.surface
    height: 32
    radius: 16
    visible: running
    width: Math.max(110, contentRow.width + 24)

    Row {
        id: contentRow

        anchors.centerIn: parent
        spacing: 8

        Item {
            height: 14
            width: 14

            RotationAnimator on rotation {
                duration: 800
                from: 0
                loops: Animation.Infinite
                running: root.running
                to: 360
            }

            Canvas {
                anchors.fill: parent
                antialiasing: true
                renderTarget: Canvas.Image

                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    ctx.beginPath();
                    ctx.arc(7, 7, 5.5, 0, Math.PI * 1.5);
                    ctx.lineWidth = 1.5;
                    ctx.lineCap = "round";
                    ctx.strokeStyle = AppTheme.primary;
                    ctx.stroke();
                }
            }
        }

        Text {
            color: AppTheme.text
            font.family: AppTheme.fontFamily
            font.pixelSize: AppTheme.fontSizeSmall
            font.weight: Font.Medium
            text: root.text
            verticalAlignment: Text.AlignVCenter
        }
    }
}
