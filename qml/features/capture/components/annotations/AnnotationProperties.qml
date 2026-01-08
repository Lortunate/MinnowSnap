import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Controls.Basic as Basic
import com.lortunate.minnow

Rectangle {
    id: root

    property color activeColor: AppTheme.danger
    property int activeSize: 4
    property string mode: "arrow"

    property bool hasOutline: false
    property bool hasStroke: false
    property string mosaicType: "mosaic"

    readonly property bool isMosaic: mode === "mosaic"
    readonly property int minSize: mode === "counter" || mode === "text" ? 16 : (isMosaic ? 5 : 2)
    readonly property int maxSize: mode === "text" ? 96 : (mode === "counter" ? 64 : (isMosaic ? 50 : 20))
    readonly property int stepSize: mode === "counter" || mode === "text" ? 4 : (isMosaic ? 5 : 2)

    readonly property var colors: [AppTheme.danger, "#FF9500", "#FFCC00", "#4CD964", "#5AC8FA", "#007AFF", "#5856D6", "#000000", "#FFFFFF"]

    signal requestColorChange(color c)
    signal requestSizeChange(int size)
    signal requestOutlineChange(bool enabled)
    signal requestStrokeChange(bool enabled)
    signal requestMosaicTypeChange(string type)

    function areColorsEqual(c1, c2) {
        const e = 0.001;
        return Math.abs(c1.r - c2.r) < e && Math.abs(c1.g - c2.g) < e && Math.abs(c1.b - c2.b) < e;
    }

    height: AppTheme.annotationPropertiesHeight
    width: mainLayout.implicitWidth + AppTheme.spacingLarge
    radius: AppTheme.radiusLarge
    color: AppTheme.surface
    border.color: AppTheme.border
    border.width: 1

    // Soft Shadow
    Rectangle {
        anchors.fill: parent
        anchors.topMargin: 2
        z: -1
        radius: AppTheme.radiusLarge
        color: AppTheme.shadowColor
    }

    RowLayout {
        id: mainLayout
        anchors.centerIn: parent
        spacing: 12

        // Color Selection
        Row {
            visible: !root.isMosaic
            spacing: 6
            Layout.alignment: Qt.AlignVCenter

            Repeater {
                model: root.colors
                delegate: ColorDot {
                    dotColor: modelData
                    selected: root.areColorsEqual(root.activeColor, dotColor)
                    onClicked: {
                        let newColor = dotColor;
                        newColor.a = root.activeColor.a;
                        root.requestColorChange(newColor);
                    }
                }
            }

            ColorDot {
                isAddButton: true
                onClicked: {
                    colorPicker.setColor(root.activeColor);
                    colorPicker.visible = true;
                }
            }
        }

        Separator {
            visible: !root.isMosaic
        }

        // Style / Type Selection
        Row {
            spacing: AppTheme.spacingSmall
            Layout.alignment: Qt.AlignVCenter

            // Shape Style (Fill/Outline)
            Row {
                visible: !root.isMosaic
                spacing: 2

                StyleButton {
                    iconSource: "qrc:/resources/icons/square.svg"
                    isActive: root.hasOutline
                    tooltip: qsTr("Outline")
                    onClicked: root.requestOutlineChange(true)
                }
                StyleButton {
                    iconSource: "qrc:/resources/icons/square_fill.svg"
                    isActive: !root.hasOutline
                    tooltip: qsTr("Fill")
                    onClicked: root.requestOutlineChange(false)
                }
            }

            // Stroke Toggle
            StyleButton {
                visible: !root.isMosaic && !root.hasOutline
                iconSource: "qrc:/resources/icons/border_all.svg"
                isActive: root.hasStroke
                tooltip: qsTr("Border")
                onClicked: root.requestStrokeChange(!root.hasStroke)
            }

            // Mosaic Type
            MosaicTypeComboBox {
                visible: root.isMosaic
                selectedType: root.mosaicType
                onTypePicked: type => root.requestMosaicTypeChange(type)
            }
        }

        Separator {
            visible: !root.isMosaic
        }

        // Sliders (Size / Opacity)
        RowLayout {
            spacing: 12
            Layout.alignment: Qt.AlignVCenter

            ValueSlider {
                label: Math.round(sliderValue)
                sliderValue: root.activeSize
                fromValue: root.minSize
                toValue: root.maxSize
                step: root.stepSize
                onMoved: v => root.requestSizeChange(v)
            }

            ValueSlider {
                visible: !root.isMosaic
                label: Math.round(sliderValue * 100) + "%"
                labelWidth: 32
                sliderValue: root.activeColor.a
                fromValue: 0.1
                toValue: 1.0
                step: 0.1
                onMoved: v => {
                    let c = root.activeColor;
                    c.a = v;
                    root.requestColorChange(c);
                }
            }
        }
    }

    CustomColorPicker {
        id: colorPicker
        visible: false
        z: 100
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.top
        anchors.bottomMargin: 12

        onAccepted: c => {
            let alpha = root.activeColor.a;
            let newColor = c;
            newColor.a = alpha;
            root.requestColorChange(newColor);
            visible = false;
        }
        onRejected: visible = false
    }

    component Separator: Rectangle {
        width: 1
        height: 16
        color: AppTheme.border
    }

    component ColorDot: Rectangle {
        id: dot
        property color dotColor: "transparent"
        property bool selected: false
        property bool isAddButton: false
        signal clicked

        width: 16
        height: 16
        radius: 8
        color: isAddButton ? "transparent" : dotColor
        border.width: 1
        border.color: isAddButton ? AppTheme.subText : "transparent"

        // Selection Ring
        Rectangle {
            anchors.centerIn: parent
            width: 22
            height: 22
            radius: 11
            color: "transparent"
            border.color: AppTheme.text
            border.width: 1.5
            visible: dot.selected && !dot.isAddButton
            opacity: 0.8
        }

        Basic.Button {
            visible: dot.isAddButton
            anchors.centerIn: parent
            width: 16
            height: 16
            padding: 0
            background: null
            display: AbstractButton.IconOnly
            icon.source: "qrc:/resources/icons/add.svg"
            icon.width: AppTheme.iconSizeSmall
            icon.height: AppTheme.iconSizeSmall
            icon.color: AppTheme.text
        }

        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: dot.clicked()
        }
    }

    component StyleButton: Basic.Button {
        id: btn
        property bool isActive: false
        property string iconSource: ""
        property string tooltip: ""

        width: AppTheme.buttonHeight
        height: AppTheme.buttonHeight
        padding: 0
        display: AbstractButton.IconOnly

        icon.source: iconSource
        icon.width: AppTheme.iconSizeMedium
        icon.height: AppTheme.iconSizeMedium
        icon.color: isActive ? "white" : AppTheme.text

        background: Rectangle {
            radius: AppTheme.radiusSmall
            color: btn.isActive ? AppTheme.primary : (btn.hovered ? AppTheme.itemHover : "transparent")
            border.width: 1
            border.color: btn.isActive ? AppTheme.primary : "transparent"
        }

        Basic.ToolTip {
            visible: btn.hovered
            text: btn.tooltip
            delay: 500
        }
    }

    component ValueSlider: RowLayout {
        property string label: ""
        property real sliderValue: 0
        property real fromValue: 0
        property real toValue: 100
        property real step: 1
        property int labelWidth: 24
        signal moved(real value)

        spacing: 8

        Text {
            Layout.preferredWidth: labelWidth
            text: label
            color: AppTheme.text
            font.pixelSize: AppTheme.fontSizeSmall
            font.bold: true
            horizontalAlignment: Text.AlignRight
        }

        Basic.Slider {
            id: sliderControl
            Layout.preferredWidth: 80
            from: fromValue
            to: toValue
            stepSize: step
            value: sliderValue

            background: Rectangle {
                x: sliderControl.leftPadding
                y: sliderControl.topPadding + sliderControl.availableHeight / 2 - height / 2
                width: sliderControl.availableWidth
                height: 4
                radius: 2
                color: AppTheme.itemHover

                Rectangle {
                    width: sliderControl.visualPosition * parent.width
                    height: parent.height
                    color: AppTheme.primary
                    radius: 2
                }
            }

            handle: Rectangle {
                x: sliderControl.leftPadding + sliderControl.visualPosition * (sliderControl.availableWidth - width)
                y: sliderControl.topPadding + sliderControl.availableHeight / 2 - height / 2
                implicitWidth: 12
                implicitHeight: 12
                radius: 6
                color: "white"

                Rectangle {
                    anchors.fill: parent
                    anchors.topMargin: 1
                    radius: 6
                    color: "black"
                    opacity: 0.2
                    z: -1
                }
            }

            onMoved: parent.moved(value)
        }
    }

    component MosaicTypeComboBox: Basic.ComboBox {
        id: combo
        property string selectedType: "mosaic"
        signal typePicked(string type)

        Layout.preferredWidth: 100
        Layout.preferredHeight: 28

        model: [qsTr("Pixelate"), qsTr("Blur")]
        currentIndex: selectedType === "mosaic" ? 0 : 1

        onActivated: index => {
            if (index === 0)
                typePicked("mosaic");
            else if (index === 1)
                typePicked("blur");
        }

        delegate: ItemDelegate {
            id: itemDelegate
            width: combo.width
            height: 28
            horizontalPadding: 8

            contentItem: Text {
                text: model.modelData
                color: itemDelegate.highlighted ? AppTheme.primary : AppTheme.text
                font.pixelSize: 12
                font.weight: combo.currentIndex === index ? Font.DemiBold : Font.Normal
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                leftPadding: 8
            }

            background: Rectangle {
                anchors.fill: parent
                anchors.margins: 2
                radius: AppTheme.radiusSmall
                color: itemDelegate.down ? AppTheme.itemPress : (itemDelegate.highlighted ? AppTheme.itemHover : "transparent")
            }
        }

        background: Rectangle {
            radius: AppTheme.radiusSmall
            color: AppTheme.itemHover
            border.width: 1
            border.color: combo.activeFocus ? AppTheme.primary : "transparent"
        }

        contentItem: Text {
            text: combo.displayText
            color: AppTheme.text
            font.pixelSize: 12
            font.weight: Font.Medium
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: 8
            rightPadding: 24
        }

        indicator: Basic.Button {
            x: combo.width - width - 2
            y: (combo.height - height) / 2
            width: AppTheme.iconSizeLarge
            height: AppTheme.iconSizeLarge
            padding: 0
            enabled: true
            background: null

            icon.source: combo.opened ? "qrc:/resources/icons/arrow_drop_up.svg" : "qrc:/resources/icons/arrow_drop_down.svg"
            icon.width: AppTheme.iconSizeLarge
            icon.height: AppTheme.iconSizeLarge
            icon.color: AppTheme.subText

            onPressed: mouse => mouse.accepted = false
        }

        popup: Popup {
            y: combo.height + 4
            width: combo.width
            implicitHeight: contentItem.implicitHeight + 8
            padding: 4

            background: Rectangle {
                radius: AppTheme.radiusLarge
                color: AppTheme.surface
                border.width: 1
                border.color: AppTheme.border

                Rectangle {
                    anchors.fill: parent
                    anchors.topMargin: 2
                    z: -1
                    radius: AppTheme.radiusLarge
                    color: AppTheme.shadowColor
                }
            }

            contentItem: ListView {
                clip: true
                implicitHeight: contentHeight
                model: combo.delegateModel
                spacing: 2
                boundsBehavior: Flickable.StopAtBounds
            }

            enter: Transition {
                NumberAnimation {
                    property: "opacity"
                    from: 0
                    to: 1
                    duration: 100
                }
                NumberAnimation {
                    property: "y"
                    from: combo.height
                    to: combo.height + 4
                    duration: AppTheme.durationFast
                }
            }
            exit: Transition {
                NumberAnimation {
                    property: "opacity"
                    from: 1
                    to: 0
                    duration: 100
                }
            }
        }
    }
}
