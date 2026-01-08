import QtQuick

Item {
    property int activeH: 0
    property int activeW: 0
    property int activeX: 0
    property int activeY: 0
    property color maskColor: "#66000000"

    Rectangle {
        color: parent.maskColor
        height: parent.activeY
        width: parent.width
        x: 0
        y: 0
    }
    Rectangle {
        color: parent.maskColor
        height: Math.max(0, parent.height - y)
        width: parent.width
        x: 0
        y: parent.activeY + parent.activeH
    }
    Rectangle {
        color: parent.maskColor
        height: parent.activeH
        width: parent.activeX
        x: 0
        y: parent.activeY
    }
    Rectangle {
        color: parent.maskColor
        height: parent.activeH
        width: Math.max(0, parent.width - x)
        x: parent.activeX + parent.activeW
        y: parent.activeY
    }
}