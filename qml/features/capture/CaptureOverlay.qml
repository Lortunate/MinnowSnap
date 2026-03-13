import QtQuick
import QtQuick.Window
import com.lortunate.minnow
import "components"
import "components/annotations"
import "../../components"

Window {
    id: overlayWindow

    readonly property int toolbarPadding: 10
    readonly property int toolbarSpacingAbove: 4
    readonly property int toolbarSpacingBelow: 4
    readonly property int defaultY: 40
    readonly property int infoTooltipSpacing: 8
    readonly property int resolutionTooltipSpacing: 8

    property string backgroundImageSource: sessionController.backgroundImageSource
    property alias currentSelection: controller.selectionRect
    readonly property rect activeRect: {
        if (isLockedState || controller.state === states.dragging) {
            return controller.selectionRect
        } else if (isBrowsingWithTarget) {
            return controller.targetRect
        }
        return Qt.rect(0, 0, 0, 0)
    }
    readonly property bool hasSelection: activeRect.width > 0 && activeRect.height > 0

    readonly property bool isLockedState: controller.state === states.locked || controller.state === states.moving || controller.state === states.resizing
    readonly property bool isBrowsingWithTarget: controller.state === states.browsing && controller.hasTarget

    readonly property color maskColor: AppTheme.overlayMask
    property bool actionProcessing: sessionController.actionProcessing
    readonly property bool processing: actionProcessing || captureCompositor.processing
    property bool annotationDisplayReady: sessionController.annotationDisplayReady
    property var screenCapture: null
    readonly property color selectionColor: AppTheme.selection
    readonly property color selectionFillColor: AppTheme.selectionFill

    signal cancelled

    function cancelSession(force) {
        sessionController.cancelSession(force === true)
    }

    function confirmSelection(action) {
        sessionController.confirmAction(action, currentSelection, annotationLayer.hasAnnotations)
    }

    function endResize() {
        controller.endResize()
    }

    function startResize(corner, mouseX, mouseY) {
        controller.startResize(corner, mouseX, mouseY)
    }

    function updateResize(mouseX, mouseY) {
        controller.updateResize(mouseX, mouseY)
    }

    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    height: Screen.height
    opacity: 0
    visible: false
    width: Screen.width
    x: Screen.virtualX
    y: Screen.virtualY

    Component.onCompleted: {
        controller.setupWindow(overlayWindow)
    }

    onVisibleChanged: {
        if (visible) {
            focusScope.forceActiveFocus()
        }
    }

    onIsLockedStateChanged: {
        sessionController.onLockedStateChanged(isLockedState)
        if (isLockedState && !annotationDisplayReady) {
            Qt.callLater(function () {
                if (isLockedState) {
                    sessionController.promoteAnnotationDisplayReady()
                }
            })
        }
    }

    CaptureSessionController {
        id: sessionController

        busy: overlayWindow.processing
        hasScreenCapture: overlayWindow.screenCapture !== null
        screenWidth: overlayWindow.width
        screenHeight: overlayWindow.height
        toolbarPadding: overlayWindow.toolbarPadding
        toolbarSpacingAbove: overlayWindow.toolbarSpacingAbove
        toolbarSpacingBelow: overlayWindow.toolbarSpacingBelow
        defaultToolbarY: overlayWindow.defaultY
    }

    OverlayController {
        id: controller

        screenHeight: overlayWindow.height
        screenWidth: overlayWindow.width
    }

    QtObject {
        id: states

        readonly property string browsing: "BROWSING"
        readonly property string dragging: "DRAGGING"
        readonly property string locked: "LOCKED"
        readonly property string moving: "MOVING"
        readonly property string resizing: "RESIZING"
    }

    Connections {
        function onScreenshotCaptured(path) {
            if (path !== "") {
                sessionController.beginSession(PathUtils.toUrl(path))
            }
        }
        function onWindowInfoReady(json) {
            controller.updateWindowList(json)
        }
        function onCaptureReady() {
            sessionController.beginSession("image://minnow/preview")
        }

        function onActionFinished() {
            cancelSession(true)
        }

        function onRequestComposition(action, x, y, w, h) {
            captureCompositor.performComposition(Qt.rect(x, y, w, h), action)
        }

        target: screenCapture
    }

    Connections {
        target: sessionController

        function onRequestAnnotationReset() {
            toolbar.activeTool = AnnotationTools.NoTool
            annotationLayer.clear()
        }

        function onRequestCompositorAbort() {
            captureCompositor.abortComposition()
        }

        function onRequestOverlayControllerReset() {
            controller.reset()
            overlayWindow.opacity = 0
        }

        function onRequestOverlayHide() {
            overlayWindow.hide()
        }

        function onRequestOverlayPresent() {
            overlayWindow.opacity = 0
            overlayWindow.show()
            overlayWindow.raise()
            overlayWindow.requestActivate()
            Qt.callLater(function () {
                if (overlayWindow.visible) {
                    overlayWindow.opacity = 1
                }
            })
        }

        function onRequestCaptureFlag(value) {
            if (screenCapture) {
                screenCapture.isCapturing = value
            }
        }

        function onRequestActionDispatch(action, x, y, width, height, hasAnnotations) {
            if (screenCapture) {
                screenCapture.requestAction(
                    sessionController.backgroundImageSource,
                    action,
                    x,
                    y,
                    width,
                    height,
                    hasAnnotations
                )
            }
        }

        function onRequestUndo() {
            annotationLayer.undo()
        }

        function onRequestRedo() {
            annotationLayer.redo()
        }

        function onSessionCancelled() {
            if (screenCapture) {
                screenCapture.releaseCaptureBuffers()
            }
            cancelled()
        }
    }

    FocusScope {
        id: focusScope

        anchors.fill: parent
        focus: true

        Keys.onEnterPressed: {
            if (hasSelection) {
                confirmSelection(CaptureActions.Copy)
            }
        }
        Keys.onEscapePressed: {
            cancelSession(false)
        }
        Keys.onPressed: event => {
            if (!isLockedState && controller.state !== states.dragging) {
                if (event.key === Qt.Key_C) {
                    if (colorPicker.visible) {
                        colorPicker.copyColor()
                        event.accepted = true
                    }
                }
                if (event.key === Qt.Key_Shift && !event.isAutoRepeat) {
                    if (colorPicker.visible) {
                        colorPicker.cycleFormat()
                        event.accepted = true
                    }
                }
            }

            if ((event.key === Qt.Key_Z) && (event.modifiers & Qt.ControlModifier)) {
                if (controller.state === states.locked) {
                    if (event.modifiers & Qt.ShiftModifier) {
                        confirmSelection(CaptureActions.Redo)
                    } else {
                        confirmSelection(CaptureActions.Undo)
                    }
                    event.accepted = true
                }
            }
        }
        Keys.onReturnPressed: {
            if (hasSelection) {
                confirmSelection(CaptureActions.Copy)
            }
        }

        Image {
            id: bgImage

            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            horizontalAlignment: Image.AlignLeft
            source: overlayWindow.backgroundImageSource
            verticalAlignment: Image.AlignTop
            layer.enabled: true
            cache: false
            asynchronous: false
        }

        SelectionMask {
            activeH: activeRect.height
            activeW: activeRect.width
            activeX: activeRect.x
            activeY: activeRect.y
            anchors.fill: parent
            maskColor: overlayWindow.maskColor
            visible: hasSelection
        }

        Rectangle {
            anchors.fill: parent
            color: overlayWindow.maskColor
            visible: !hasSelection
        }

        MouseArea {
            id: mouseArea

            acceptedButtons: Qt.LeftButton | Qt.RightButton
            anchors.fill: parent
            cursorShape: {
                if (controller.state === states.locked) {
                    if (containsMouseInRect(mouseX, mouseY)) {
                        return Qt.SizeAllCursor
                    }
                    return Qt.ArrowCursor
                }
                if (colorPicker.visible) {
                    return Qt.CrossCursor
                }
                return (controller.state === states.browsing && controller.hasTarget) ? Qt.PointingHandCursor : Qt.CrossCursor
            }
            hoverEnabled: true

            function containsMouseInRect(mx, my) {
                return mx >= currentSelection.x && mx <= currentSelection.x + currentSelection.width && my >= currentSelection.y && my <= currentSelection.y + currentSelection.height
            }

            onPositionChanged: mouse => {
                if (controller.state === states.browsing) {
                    controller.updateHover(mouse.x, mouse.y)
                    if (colorPicker.visible) {
                        colorPicker.mouseX = mouse.x
                        colorPicker.mouseY = mouse.y
                    }
                } else if (controller.state === states.dragging) {
                    controller.updateSelection(mouse.x, mouse.y)
                } else if (controller.state === states.moving) {
                    controller.updateMove(mouse.x, mouse.y)
                }
            }

            onPressed: mouse => {
                if (mouse.button === Qt.RightButton) {
                    if (controller.state === states.locked) {
                        controller.reset()
                    } else {
                        cancelSession(false)
                    }
                    return
                }

                if (controller.state === states.locked) {
                    annotationLayer.deselectAll()
                    if (containsMouseInRect(mouse.x, mouse.y)) {
                        controller.startMove(mouse.x, mouse.y)
                    }
                } else if (controller.state === states.browsing) {
                    sessionController.resetAnnotationState()
                    controller.startSelection(mouse.x, mouse.y)
                }
            }

            onReleased: mouse => {
                if (controller.state === states.dragging) {
                    controller.endSelection()
                } else if (controller.state === states.moving) {
                    controller.endMove()
                }
            }
        }

        Item {
            id: container

            anchors.fill: parent

            SelectionRect {
                id: unifiedSelectionRect

                bindToRect: isLockedState

                x: !bindToRect ? activeRect.x : 0
                y: !bindToRect ? activeRect.y : 0
                width: !bindToRect ? activeRect.width : 0
                height: !bindToRect ? activeRect.height : 0

                rectProperty: overlayWindow.currentSelection
                overlayWindow: overlayWindow
                visible: hasSelection || isBrowsingWithTarget

                AnnotationLayer {
                    id: annotationLayer

                    activeTool: toolbar.activeTool
                    anchors.fill: parent
                    clip: true
                    sourceImage: bgImage
                    layerX: unifiedSelectionRect.x
                    layerY: unifiedSelectionRect.y
                    visible: isLockedState && annotationDisplayReady
                    onRequestSetTool: tool => toolbar.activeTool = tool
                }
            }

            InfoTooltip {
                id: infoBox
                text: controller.targetInfo
                visible: isBrowsingWithTarget
                x: activeRect.x
                y: {
                    let val = activeRect.y - height - infoTooltipSpacing
                    return val < 0 ? activeRect.y + activeRect.height + infoTooltipSpacing : val
                }
            }

            ResolutionTooltip {
                id: resolutionBox
                property bool showAbove: activeRect.y - height - resolutionTooltipSpacing >= 0

                heightValue: activeRect.height
                visible: isLockedState || controller.state === states.dragging
                widthValue: activeRect.width
                x: activeRect.x
                y: showAbove ? activeRect.y - height - resolutionTooltipSpacing : activeRect.y + activeRect.height + resolutionTooltipSpacing
            }

            BusyStatus {
                anchors.centerIn: parent
                running: processing
            }

            ColorPicker {
                id: colorPicker

                imageSource: overlayWindow.backgroundImageSource
                mouseX: mouseArea.mouseX
                mouseY: mouseArea.mouseY
                screenCapture: overlayWindow.screenCapture
                surfaceWidth: overlayWindow.width
                surfaceHeight: overlayWindow.height
                visible: !isLockedState && controller.state !== states.dragging
                z: 1000

                onColorCopied: {
                    cancelSession(false)
                }
            }
        }

        SelectionToolbar {
            id: toolbar

            property point pos: sessionController.toolbarPosition(
                Qt.rect(
                    currentSelection.x,
                    currentSelection.y,
                    currentSelection.width,
                    currentSelection.height
                ),
                width,
                height,
                false
            )

            visible: controller.state === states.locked
            x: pos.x
            y: pos.y
            z: 999

            onActionConfirmed: action => confirmSelection(action)
            onCanceled: {
                cancelSession(false)
            }
        }

        AnnotationProperties {
            id: propBar

            property point pos: sessionController.toolbarPosition(
                Qt.rect(
                    toolbar.x,
                    toolbar.y,
                    toolbar.width,
                    toolbar.height
                ),
                width,
                height,
                false
            )

            activeColor: annotationLayer.activeColor
            hasOutline: annotationLayer.activeHasOutline
            hasStroke: annotationLayer.activeHasStroke
            mosaicType: annotationLayer.activeMosaicType
            mode: {
                if (annotationLayer.selectedItem) {
                    return annotationLayer.itemTypeToTool[annotationLayer.selectedItem.type] || AnnotationTools.Arrow
                }
                if (toolbar.activeTool === AnnotationTools.Arrow)
                    return AnnotationTools.Arrow
                if (toolbar.activeTool === AnnotationTools.Rectangle)
                    return AnnotationTools.Rectangle
                if (toolbar.activeTool === AnnotationTools.Circle)
                    return AnnotationTools.Circle
                if (toolbar.activeTool === AnnotationTools.Counter)
                    return AnnotationTools.Counter
                if (toolbar.activeTool === AnnotationTools.Text)
                    return AnnotationTools.Text
                if (toolbar.activeTool === AnnotationTools.Mosaic)
                    return AnnotationTools.Mosaic
                return AnnotationTools.Arrow
            }
            activeSize: {
                if (mode === AnnotationTools.Counter)
                    return annotationLayer.activeCounterSize
                if (mode === AnnotationTools.Text)
                    return annotationLayer.activeFontSize
                if (mode === AnnotationTools.Mosaic)
                    return annotationLayer.activeIntensity
                return annotationLayer.activeLineWidth
            }
            visible: controller.state === states.locked && (toolbar.activeTool === AnnotationTools.Arrow || toolbar.activeTool === AnnotationTools.Rectangle || toolbar.activeTool === AnnotationTools.Circle || toolbar.activeTool === AnnotationTools.Counter || toolbar.activeTool === AnnotationTools.Text || toolbar.activeTool === AnnotationTools.Mosaic || annotationLayer.selectedItem)

            x: pos.x
            y: pos.y
            z: 999

            onRequestColorChange: c => annotationLayer.activeColor = c
            onRequestOutlineChange: enabled => annotationLayer.activeHasOutline = enabled
            onRequestStrokeChange: enabled => annotationLayer.activeHasStroke = enabled
            onRequestMosaicTypeChange: type => annotationLayer.activeMosaicType = type
            onRequestSizeChange: size => {
                if (mode === AnnotationTools.Counter)
                    annotationLayer.activeCounterSize = size
                else if (mode === AnnotationTools.Text)
                    annotationLayer.activeFontSize = size
                else if (mode === AnnotationTools.Mosaic)
                    annotationLayer.activeIntensity = size
                else
                    annotationLayer.activeLineWidth = size
            }
        }
    }

    LongCaptureManager {
        id: longCaptureManager

        screenCapture: overlayWindow.screenCapture

        onRequestHide: overlayWindow.hide()
        onRequestReset: sessionController.resetSession()
    }

    CaptureCompositor {
        id: captureCompositor
        z: -1

        annotationLayer: annotationLayer
        lockedSelectionRect: unifiedSelectionRect
        overlayWindow: overlayWindow
        sessionController: sessionController
        screenCapture: overlayWindow.screenCapture

    }
}
