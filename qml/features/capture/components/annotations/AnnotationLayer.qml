import QtQuick
import com.lortunate.minnow

Item {
    id: root

    property color activeColor: AppTheme.danger
    property bool activeHasOutline: true
    property bool activeHasStroke: false
    property int activeLineWidth: 4
    property int activeIntensity: 10
    property string activeMosaicType: "mosaic"
    property int activeCounterSize: 32
    property int activeFontSize: 24
    property string activeTool: ""
    readonly property bool hasAnnotations: annotationController.hasAnnotations
    property var selectedItem: null
    property Item sourceImage: null
    property real layerX: 0
    property real layerY: 0
    readonly property var annotationComponents: ({
        "arrow": "ArrowItem.qml",
        "rectangle": "RectangleItem.qml",
        "circle": "CircleItem.qml",
        "counter": "CounterItem.qml",
        "text": "TextItem.qml",
        "mosaic": "MosaicItem.qml"
    })
    property var componentCache: ({})

    signal requestSetTool(string tool)

    function toColorString(colorValue) {
        return colorValue.toString()
    }

    function eachAnnotationChild(visitor) {
        for (let i = 0; i < annotations.children.length; i++) {
            visitor(annotations.children[i], i)
        }
    }

    function findItemByAnnotationId(annotationId) {
        for (let i = 0; i < annotations.children.length; i++) {
            const child = annotations.children[i]
            if (child.annotationId === annotationId) {
                return child
            }
        }
        return null
    }

    function bringToFront(item) {
        let maxZ = 0
        eachAnnotationChild(function (child) {
            if (child.z > maxZ) {
                maxZ = child.z
            }
        })
        item.z = maxZ + 1
    }

    function deselectAll() {
        eachAnnotationChild(function (child) {
            if (child.selected !== undefined) {
                child.selected = false
            }
        })
        selectedItem = null
    }

    function updateDrawingMode(isDrawing) {
        if (isDrawing) {
            return
        }
        eachAnnotationChild(function (child) {
            if (child.drawingMode !== undefined) {
                child.drawingMode = false
            }
        })
    }

    function clear() {
        annotationController.clearAll()
    }

    function undo() {
        annotationController.undo()
    }

    function redo() {
        annotationController.redo()
    }

    function valueOrFallback(item, key, fallbackValue) {
        return item[key] !== undefined ? item[key] : fallbackValue
    }

    function applyActiveStateToSelectedItem() {
        if (!selectedItem) {
            return
        }

        if (selectedItem.color !== undefined && selectedItem.color !== activeColor) {
            selectedItem.color = activeColor
        }
        if (selectedItem.hasOutline !== undefined && selectedItem.hasOutline !== activeHasOutline) {
            selectedItem.hasOutline = activeHasOutline
        }
        if (selectedItem.hasStroke !== undefined && selectedItem.hasStroke !== activeHasStroke) {
            selectedItem.hasStroke = activeHasStroke
        }

        switch (selectedItem.type) {
        case "counter":
            if (selectedItem.size !== activeCounterSize) {
                selectedItem.size = activeCounterSize
            }
            break
        case "text":
            if (selectedItem.fontSize !== activeFontSize) {
                selectedItem.fontSize = activeFontSize
            }
            break
        case "mosaic":
            if (selectedItem.intensity !== activeIntensity) {
                selectedItem.intensity = activeIntensity
            }
            if (selectedItem.mosaicType !== activeMosaicType) {
                selectedItem.mosaicType = activeMosaicType
            }
            break
        default:
            if (selectedItem.lineWidth !== undefined && selectedItem.lineWidth !== activeLineWidth) {
                selectedItem.lineWidth = activeLineWidth
            }
            break
        }
    }

    function getComponent(componentName) {
        let component = componentCache[componentName]
        if (!component) {
            component = Qt.createComponent(componentName)
            if (!component) {
                return null
            }
            componentCache[componentName] = component
        }

        if (component.status === Component.Error) {
            delete componentCache[componentName]
            return null
        }

        if (component.status !== Component.Ready) {
            return null
        }

        return component
    }

    function createAnnotationItem(componentName, props) {
        const component = getComponent(componentName)
        if (!component) {
            return null
        }

        const item = component.createObject(annotations, props)
        if (!item) {
            return null
        }

        item.requestRemove.connect(function () {
            annotationController.removeAnnotation(item.annotationId)
        })

        item.requestSelect.connect(function () {
            notifySelection(item, false)
        })

        return item
    }

    function notifySelection(item, deactivateTool) {
        annotationController.onAnnotationSelected(
            item.annotationId,
            item.type,
            toColorString(item.color),
            valueOrFallback(item, "hasOutline", false),
            valueOrFallback(item, "hasStroke", false),
            valueOrFallback(item, "lineWidth", activeLineWidth),
            valueOrFallback(item, "intensity", activeIntensity),
            valueOrFallback(item, "mosaicType", activeMosaicType),
            valueOrFallback(item, "size", activeCounterSize),
            valueOrFallback(item, "fontSize", activeFontSize),
            deactivateTool
        )
    }

    function syncRootToController(rootValue, controllerValue, updater, applySelected) {
        if (controllerValue !== rootValue) {
            updater(rootValue)
        }
        if (applySelected) {
            applyActiveStateToSelectedItem()
        }
    }

    function syncRootColorToController() {
        const value = toColorString(activeColor)
        if (annotationController.activeColor !== value) {
            annotationController.updateActiveColor(value)
        }
        applyActiveStateToSelectedItem()
    }

    function setRootValueIfChanged(key, value) {
        if (root[key] !== value) {
            root[key] = value
        }
    }

    function setRootColorIfChanged(controllerColor) {
        const c = Qt.color(controllerColor)
        if (root.activeColor !== c) {
            root.activeColor = c
        }
    }

    function buildCreateProps(annotationId, startPoint) {
        const props = {
            "annotationId": annotationId,
            "color": root.activeColor,
            "selected": false,
            "p1": startPoint,
            "hasOutline": root.activeHasOutline,
            "hasStroke": root.activeHasStroke,
            "lineWidth": root.activeLineWidth,
            "drawingMode": true
        }

        if (root.activeTool === "counter") {
            props["selected"] = true
            props["number"] = annotationController.nextCounterValue
            props["size"] = root.activeCounterSize
        } else if (root.activeTool === "text") {
            props["selected"] = true
            props["fontSize"] = root.activeFontSize
        } else if (root.activeTool === "mosaic") {
            props["p2"] = startPoint
            props["sourceItem"] = root.sourceImage
            props["intensity"] = root.activeIntensity
            props["mosaicType"] = root.activeMosaicType
            props["targetLayer"] = root
        } else {
            props["p2"] = startPoint
        }

        return props
    }

    onActiveToolChanged: {
        updateDrawingMode(activeTool !== "")
        syncRootToController(activeTool, annotationController.activeTool, annotationController.updateActiveTool, false)
    }

    onActiveColorChanged: {
        syncRootColorToController()
    }

    onActiveHasOutlineChanged: {
        syncRootToController(activeHasOutline, annotationController.activeHasOutline, annotationController.updateActiveHasOutline, true)
    }

    onActiveHasStrokeChanged: {
        syncRootToController(activeHasStroke, annotationController.activeHasStroke, annotationController.updateActiveHasStroke, true)
    }

    onActiveLineWidthChanged: {
        syncRootToController(activeLineWidth, annotationController.activeLineWidth, annotationController.updateActiveLineWidth, true)
    }

    onActiveIntensityChanged: {
        syncRootToController(activeIntensity, annotationController.activeIntensity, annotationController.updateActiveIntensity, true)
    }

    onActiveMosaicTypeChanged: {
        syncRootToController(activeMosaicType, annotationController.activeMosaicType, annotationController.updateActiveMosaicType, true)
    }

    onActiveCounterSizeChanged: {
        syncRootToController(activeCounterSize, annotationController.activeCounterSize, annotationController.updateActiveCounterSize, true)
    }

    onActiveFontSizeChanged: {
        syncRootToController(activeFontSize, annotationController.activeFontSize, annotationController.updateActiveFontSize, true)
    }

    Component.onCompleted: {
        annotationController.initializeDefaults(toColorString(activeColor))
    }

    AnnotationController {
        id: annotationController

        onRequestSetTool: tool => root.requestSetTool(tool)

        onRequestClearSelection: {
            root.deselectAll()
        }

        onRequestSelectAnnotation: annotationId => {
            const item = root.findItemByAnnotationId(annotationId)
            if (item && item.visible) {
                item.selected = true
                root.selectedItem = item
            }
        }

        onRequestBringToFront: annotationId => {
            const item = root.findItemByAnnotationId(annotationId)
            if (item) {
                root.bringToFront(item)
            }
        }

        onRequestRemoveAnnotation: annotationId => {
            const item = root.findItemByAnnotationId(annotationId)
            if (item) {
                item.destroy()
            }
            if (root.selectedItem && root.selectedItem.annotationId === annotationId) {
                root.selectedItem = null
            }
        }

        onRequestSetAnnotationVisible: (annotationId, visible) => {
            const item = root.findItemByAnnotationId(annotationId)
            if (item) {
                item.visible = visible
                if (!visible && root.selectedItem && root.selectedItem.annotationId === annotationId) {
                    root.selectedItem = null
                }
            }
        }

        onActiveToolChanged: {
            root.setRootValueIfChanged("activeTool", annotationController.activeTool)
        }

        onActiveColorChanged: {
            root.setRootColorIfChanged(annotationController.activeColor)
        }

        onActiveHasOutlineChanged: {
            root.setRootValueIfChanged("activeHasOutline", annotationController.activeHasOutline)
        }

        onActiveHasStrokeChanged: {
            root.setRootValueIfChanged("activeHasStroke", annotationController.activeHasStroke)
        }

        onActiveLineWidthChanged: {
            root.setRootValueIfChanged("activeLineWidth", annotationController.activeLineWidth)
        }

        onActiveIntensityChanged: {
            root.setRootValueIfChanged("activeIntensity", annotationController.activeIntensity)
        }

        onActiveMosaicTypeChanged: {
            root.setRootValueIfChanged("activeMosaicType", annotationController.activeMosaicType)
        }

        onActiveCounterSizeChanged: {
            root.setRootValueIfChanged("activeCounterSize", annotationController.activeCounterSize)
        }

        onActiveFontSizeChanged: {
            root.setRootValueIfChanged("activeFontSize", annotationController.activeFontSize)
        }
    }

    MouseArea {
        property var currentItem: null
        property point startPoint: Qt.point(0, 0)

        anchors.fill: parent
        cursorShape: root.activeTool === "text" ? Qt.IBeamCursor : Qt.CrossCursor
        enabled: root.activeTool !== ""

        onPositionChanged: mouse => {
            if (currentItem) {
                let p2 = Qt.point(mouse.x, mouse.y)

                if ((mouse.modifiers & Qt.ShiftModifier) && (root.activeTool === "circle" || root.activeTool === "rectangle")) {
                    const dx = p2.x - currentItem.p1.x
                    const dy = p2.y - currentItem.p1.y
                    const size = Math.max(Math.abs(dx), Math.abs(dy))
                    const sx = dx >= 0 ? 1 : -1
                    const sy = dy >= 0 ? 1 : -1
                    p2 = Qt.point(currentItem.p1.x + sx * size, currentItem.p1.y + sy * size)
                }

                if (root.activeTool !== "text") {
                    currentItem.p2 = p2
                }
            }
        }

        onPressed: mouse => {
            const componentName = root.annotationComponents[root.activeTool]
            if (!componentName) {
                return
            }

            startPoint = Qt.point(mouse.x, mouse.y)

            const annotationId = annotationController.beginCreateAnnotation()
            if (annotationId < 0) {
                return
            }

            const props = root.buildCreateProps(annotationId, startPoint)
            const item = root.createAnnotationItem(componentName, props)

            if (!item) {
                annotationController.cancelCreatedAnnotation(annotationId)
                return
            }

            annotationController.registerCreatedAnnotation(annotationId, root.activeTool)
            if (root.activeTool !== "counter" && root.activeTool !== "text") {
                currentItem = item
            }
            mouse.accepted = true
        }

        onReleased: mouse => {
            if (currentItem) {
                if (Math.abs(currentItem.p1.x - currentItem.p2.x) < 5 && Math.abs(currentItem.p1.y - currentItem.p2.y) < 5) {
                    annotationController.cancelCreatedAnnotation(currentItem.annotationId)
                } else {
                    currentItem.drawingMode = false
                    currentItem.interactionEnabled = true
                    notifySelection(currentItem, false)
                }
                currentItem = null
            }
        }
    }

    Item {
        id: annotations
        anchors.fill: parent
    }

    Shortcut {
        sequence: "Backspace"
        enabled: root.selectedItem !== null
        onActivated: {
            if (root.selectedItem) {
                annotationController.removeAnnotation(root.selectedItem.annotationId)
            }
        }
    }
}
