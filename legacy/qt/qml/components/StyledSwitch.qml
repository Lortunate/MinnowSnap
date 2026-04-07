import QtQuick
import QtQuick.Controls.Basic
import com.lortunate.minnow

Switch {
    id: control

    implicitWidth: 44
    implicitHeight: 24

    indicator: Rectangle {
        id: track
        implicitWidth: 44
        implicitHeight: 24
        x: control.leftPadding
        y: parent.height / 2 - height / 2
        radius: height / 2
        color: control.checked ? AppTheme.primary : AppTheme.switchTrackOff
        border.color: control.checked ? AppTheme.primary : AppTheme.switchBorderOff
        border.width: 1

        Behavior on color {
            ColorAnimation {
                duration: 200
                easing.type: Easing.InOutQuad
            }
        }
        
        Behavior on border.color {
            ColorAnimation {
                duration: 200
                easing.type: Easing.InOutQuad
            }
        }

        Rectangle {
            id: handle
            x: control.checked ? parent.width - width - 2 : 2
            y: 2
            width: parent.height - 4
            height: parent.height - 4
            radius: width / 2
            color: "#FFFFFF"
            
            border.color: "#0000001A" 
            border.width: 1

            Behavior on x {
                NumberAnimation {
                    duration: 200
                    easing.type: Easing.OutCubic
                }
            }
        }
    }
}
