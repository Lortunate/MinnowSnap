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

    readonly property real colorEqualThreshold: 0.001
    readonly property int mainLayoutSpacing: 12
    readonly property int colorRowSpacing: 6
    readonly property int styleRowSpacing: 2
    readonly property int sliderLayoutSpacing: 12
    readonly property int separatorWidth: 1
    readonly property int separatorHeight: 16
    readonly property int colorDotSize: 16
    readonly property int colorDotRadius: 8
    readonly property int colorSelectionRingSize: 22
    readonly property int colorSelectionRingRadius: 11
    readonly property real colorSelectionRingBorderWidth: 1.5
    readonly property real colorSelectionRingOpacity: 0.8
    readonly property int buttonPadding: 0
    readonly property int styleButtonSpacing: 2
    readonly property int colorPickerZIndex: 100
    readonly property int colorPickerBottomMargin: 12
    readonly property int comboBoxWidth: 100
    readonly property int comboBoxHeight: 28
    readonly property int comboBoxDelegateHeight: 28
    readonly property int comboBoxHorizontalPadding: 8
    readonly property int comboBoxPopupYOffset: 4
    readonly property int comboBoxPopupPadding: 4
    readonly property int comboBoxIndicatorPadding: 2
    readonly property int sliderControlWidth: 80
    readonly property int sliderSpacing: 8
    readonly property int sliderBackgroundHeight: 4
    readonly property int sliderBackgroundRadius: 2
    readonly property int sliderHandleSize: 12
    readonly property int sliderHandleRadius: 6
    readonly property int sliderHandleMarginTop: 1
    readonly property int opacitySliderLabelWidth: 32
    readonly property real opacityMinValue: 0.1
    readonly property real opacityMaxValue: 1.0
    readonly property real opacityStep: 0.1
    readonly property int popupTransitionDuration: 100
    readonly property int shadowTopMargin: 2
    readonly property int shadowZIndex: -1
    readonly property int contentMarginTop: 2
    readonly property int contentPadding: 8
    readonly property int contentItemHeight: 28
    readonly property int contentItemLeftPadding: 8
    readonly property int contentItemRightPadding: 24
    readonly property int contentItemTopMargin: 1
    readonly property int tooltipDelay: 500
    readonly property real shadowOpacity: 0.2

    signal requestColorChange(color c)
    signal requestSizeChange(int size)
    signal requestOutlineChange(bool enabled)
    signal requestStrokeChange(bool enabled)
    signal requestMosaicTypeChange(string type)

    function areColorsEqual(c1, c2) {
        const e = colorEqualThreshold;
        return Math.abs(c1.r - c2.r) < e && Math.abs(c1.g - c2.g) < e && Math.abs(c1.b - c2.b) < e;
    }

    height: AppTheme.annotationPropertiesHeight
    width: mainLayout.implicitWidth + AppTheme.spacingLarge
    radius: AppTheme.radiusLarge
    color: AppTheme.surface
    border.color: AppTheme.border
    border.width: 1

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: shadowTopMargin
        z: shadowZIndex
        radius: AppTheme.radiusLarge
        color: AppTheme.shadowColor
    }

    RowLayout {
        id: mainLayout
        anchors.centerIn: parent
        spacing: mainLayoutSpacing

        Row {
            visible: !root.isMosaic
            spacing: colorRowSpacing
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

        Row {
            spacing: AppTheme.spacingSmall
            Layout.alignment: Qt.AlignVCenter

            Row {
                visible: !root.isMosaic
                spacing: styleRowSpacing

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

            StyleButton {
                visible: !root.isMosaic && !root.hasOutline
                iconSource: "qrc:/resources/icons/border_all.svg"
                isActive: root.hasStroke
                tooltip: qsTr("Border")
                onClicked: root.requestStrokeChange(!root.hasStroke)
            }

            MosaicTypeComboBox {
                visible: root.isMosaic
                selectedType: root.mosaicType
                onTypePicked: type => root.requestMosaicTypeChange(type)
            }
        }

        Separator {
            visible: !root.isMosaic
        }

        RowLayout {
            spacing: sliderLayoutSpacing
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
                labelWidth: opacitySliderLabelWidth
                sliderValue: root.activeColor.a
                fromValue: opacityMinValue
                toValue: opacityMaxValue
                step: opacityStep
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
        z: colorPickerZIndex
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.top
        anchors.bottomMargin: colorPickerBottomMargin

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
        width: separatorWidth
        height: separatorHeight
        color: AppTheme.border
    }

    component ColorDot: Rectangle {
        id: dot
        property color dotColor: "transparent"
        property bool selected: false
        property bool isAddButton: false
        signal clicked

        width: colorDotSize
        height: colorDotSize
        radius: colorDotRadius
        color: isAddButton ? "transparent" : dotColor
        border.width: 1
        border.color: isAddButton ? AppTheme.subText : "transparent"

        Rectangle {
            anchors.centerIn: parent
            width: colorSelectionRingSize
            height: colorSelectionRingSize
            radius: colorSelectionRingRadius
            color: "transparent"
            border.color: AppTheme.text
            border.width: colorSelectionRingBorderWidth
            visible: dot.selected && !dot.isAddButton
            opacity: colorSelectionRingOpacity
        }

        Basic.Button {
            visible: dot.isAddButton
            anchors.centerIn: parent
            width: colorDotSize
            height: colorDotSize
            padding: buttonPadding
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
        padding: buttonPadding
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
            delay: tooltipDelay
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

        spacing: sliderSpacing

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
            Layout.preferredWidth: sliderControlWidth
            from: fromValue
            to: toValue
            stepSize: step
            value: sliderValue

            background: Rectangle {
                x: sliderControl.leftPadding
                y: sliderControl.topPadding + sliderControl.availableHeight / 2 - height / 2
                width: sliderControl.availableWidth
                height: sliderBackgroundHeight
                radius: sliderBackgroundRadius
                color: AppTheme.itemHover

                Rectangle {
                    width: sliderControl.visualPosition * parent.width
                    height: parent.height
                    color: AppTheme.primary
                    radius: sliderBackgroundRadius
                }
            }

            handle: Rectangle {
                x: sliderControl.leftPadding + sliderControl.visualPosition * (sliderControl.availableWidth - width)
                y: sliderControl.topPadding + sliderControl.availableHeight / 2 - height / 2
                implicitWidth: sliderHandleSize
                implicitHeight: sliderHandleSize
                radius: sliderHandleRadius
                color: "white"

                Rectangle {
                    anchors.fill: parent
                    anchors.topMargin: sliderHandleMarginTop
                    radius: sliderHandleRadius
                    color: "black"
                    opacity: shadowOpacity
                    z: shadowZIndex
                }
            }

            onMoved: parent.moved(value)
        }
    }

    component MosaicTypeComboBox: Basic.ComboBox {
        id: combo
        property string selectedType: "mosaic"
        signal typePicked(string type)

        Layout.preferredWidth: comboBoxWidth
        Layout.preferredHeight: comboBoxHeight

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
            height: comboBoxDelegateHeight
            horizontalPadding: comboBoxHorizontalPadding

            contentItem: Text {
                text: model.modelData
                color: itemDelegate.highlighted ? AppTheme.primary : AppTheme.text
                font.pixelSize: 12
                font.weight: combo.currentIndex === index ? Font.DemiBold : Font.Normal
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                leftPadding: contentItemLeftPadding
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
            leftPadding: contentItemLeftPadding
            rightPadding: contentItemRightPadding
        }

        indicator: Basic.Button {
            x: combo.width - width - comboBoxIndicatorPadding
            y: (combo.height - height) / 2
            width: AppTheme.iconSizeLarge
            height: AppTheme.iconSizeLarge
            padding: buttonPadding
            enabled: true
            background: null

            icon.source: combo.opened ? "qrc:/resources/icons/arrow_drop_up.svg" : "qrc:/resources/icons/arrow_drop_down.svg"
            icon.width: AppTheme.iconSizeLarge
            icon.height: AppTheme.iconSizeLarge
            icon.color: AppTheme.subText

            onPressed: mouse => mouse.accepted = false
        }

        popup: Popup {
            y: combo.height + comboBoxPopupYOffset
            width: combo.width
            implicitHeight: contentItem.implicitHeight + contentPadding * 2
            padding: comboBoxPopupPadding

            background: Rectangle {
                radius: AppTheme.radiusLarge
                color: AppTheme.surface
                border.width: 1
                border.color: AppTheme.border

                Rectangle {
                    anchors.fill: parent
                    anchors.topMargin: contentMarginTop
                    z: shadowZIndex
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
                    duration: popupTransitionDuration
                }
                NumberAnimation {
                    property: "y"
                    from: combo.height
                    to: combo.height + comboBoxPopupYOffset
                    duration: AppTheme.durationFast
                }
            }
            exit: Transition {
                NumberAnimation {
                    property: "opacity"
                    from: 1
                    to: 0
                    duration: popupTransitionDuration
                }
            }
        }
    }
}
