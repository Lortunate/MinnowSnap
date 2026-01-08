import QtQuick

InfoTooltip {
    property real heightValue: 0
    property real widthValue: 0

    text: Math.round(widthValue) + " x " + Math.round(heightValue)
}