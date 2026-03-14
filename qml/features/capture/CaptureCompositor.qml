import QtQuick
import QtQuick.Window
import QtCore
import com.lortunate.minnow

Item {
    id: root

    property var annotationLayer
    property var lockedSelectionRect
    property var overlayWindow
    property var sessionController

    property var pinnedWindows: []
    property var pinWindowComponent: null

    property var screenCapture
    readonly property bool processing: compositorController.processing


    function restoreAnnotationLayerParent() {
        if (annotationLayer && lockedSelectionRect && annotationLayer.parent !== lockedSelectionRect) {
            annotationLayer.parent = lockedSelectionRect
            annotationLayer.anchors.fill = lockedSelectionRect
        }
    }

    function abortComposition() {
        compositorController.abort()
    }

    function getPinWindowComponent() {
        if (!pinWindowComponent) {
            pinWindowComponent = Qt.createComponent("../pin/PinWindow.qml")
        }
        if (pinWindowComponent.status === Component.Error) {
            console.error("Failed to load PinWindow component:", pinWindowComponent.errorString())
            return null
        }
        if (pinWindowComponent.status !== Component.Ready) {
            return null
        }
        return pinWindowComponent
    }

    function performComposition(selectionRect, action) {
        compositorController.start(
            action,
            selectionRect,
            annotationLayer ? annotationLayer.hasAnnotations : false,
            compositorBg.sourceSize.width,
            compositorBg.width,
            Screen.devicePixelRatio
        )
    }
    
    function showPin(path, rect, autoOcr) {
        let component = getPinWindowComponent()
        if (!component) {
            return
        }

        let shadowMargin = 20
        let finalPath = PathUtils.toUrl(path)

        let win = component.createObject(null, {
            "imageSource": finalPath,
            "screenCapture": root.screenCapture,
            "x": rect.x - shadowMargin,
            "y": rect.y - shadowMargin,
            "width": rect.width + (shadowMargin * 2),
            "height": rect.height + (shadowMargin * 2),
            "autoOcr": autoOcr === true
        })
        if (!win) {
            return
        }

        pinnedWindows.push(win)

        win.closing.connect(function () {
            let index = pinnedWindows.indexOf(win)
            if (index > -1) {
                pinnedWindows.splice(index, 1)
            }
        })

        win.show()
    }

    Item {
        id: compositor

        visible: true
        width: 0
        height: 0

        z: -100

        Item {
            anchors.fill: parent
            clip: true

            Image {
                id: compositorBg

                fillMode: Image.PreserveAspectCrop
                height: overlayWindow ? overlayWindow.height : 0
                horizontalAlignment: Image.AlignLeft
                source: (root.processing && overlayWindow) ? overlayWindow.backgroundImageSource : ""
                verticalAlignment: Image.AlignTop
                width: overlayWindow ? overlayWindow.width : 0
                cache: false
                asynchronous: false
            }
        }

        Item {
            id: annotationWrapper

            anchors.fill: parent
        }
    }

    CaptureCompositorController {
        id: compositorController
    }

    Connections {
        target: compositorController

        function onRequestPrepareComposition(compositionId, rect, outW, outH) {
            compositor.width = rect.width
            compositor.height = rect.height
            compositorBg.x = -rect.x
            compositorBg.y = -rect.y

            if (annotationLayer) {
                annotationLayer.parent = annotationWrapper
                annotationLayer.anchors.fill = annotationWrapper
                annotationLayer.deselectAll()
            }

            if (!screenCapture) {
                compositorController.handleGrabResult(compositionId, "", false)
                return
            }

            const savePath = screenCapture.generateTempPath("png")

            Qt.callLater(function () {
                if (!compositorController.isActive(compositionId)) {
                    return
                }
                compositor.grabToImage(function (result) {
                    if (!compositorController.isActive(compositionId)) {
                        return
                    }
                    const saved = result.saveToFile(savePath)
                    compositorController.handleGrabResult(compositionId, savePath, saved)
                }, Qt.size(outW, outH))
            })
        }

        function onRequestSubmitDirect(action, rect) {
            if (screenCapture) {
                screenCapture.submitCapture(
                    overlayWindow.backgroundImageSource,
                    action,
                    rect
                )
            }
        }

        function onRequestSubmitComposited(path, action, rect) {
            if (screenCapture) {
                screenCapture.submitCompositedCapture(
                    PathUtils.toUrl(path),
                    action,
                    rect
                )
            }
        }

        function onRequestRestoreAnnotation() {
            restoreAnnotationLayerParent()
        }

        function onRequestCompositionFailed() {
            console.error("Failed to save composite image")
            if (sessionController) {
                sessionController.cancelSession(true)
            }
        }

        function onRequestResetSurface() {
            compositor.width = 0
            compositor.height = 0
        }
    }

    Connections {
        target: screenCapture
        function onPinWindowRequested(path, selectionRect, autoOcr) {
            showPin(path, selectionRect, autoOcr)
        }
    }
}
