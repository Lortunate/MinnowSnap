import QtQuick
import QtQuick.Window
import QtCore
import com.lortunate.minnow

Item {
    id: root

    property var annotationLayer
    property var lockedSelectionRect
    property var overlayWindow

    property bool processing: false
    property var pinnedWindows: []
    property int compositionVersion: 0
    property int activeCompositionId: -1
    property var pinWindowComponent: null

    property var screenCapture


    function normalizeRect(rect) {
        return Qt.rect(
            Math.floor(rect.x),
            Math.floor(rect.y),
            Math.max(1, Math.ceil(rect.width)),
            Math.max(1, Math.ceil(rect.height))
        )
    }

    function restoreAnnotationLayerParent() {
        if (annotationLayer && lockedSelectionRect && annotationLayer.parent !== lockedSelectionRect) {
            annotationLayer.parent = lockedSelectionRect
            annotationLayer.anchors.fill = lockedSelectionRect
        }
    }

    function finishComposition() {
        compositor.width = 0
        compositor.height = 0
        processing = false
    }

    function abortComposition() {
        activeCompositionId = -1
        restoreAnnotationLayerParent()
        finishComposition()
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
        const rect = normalizeRect(selectionRect)

        if (annotationLayer && annotationLayer.hasAnnotations) {
            const compositionId = ++compositionVersion
            activeCompositionId = compositionId
            processing = true

            let srcW = compositorBg.sourceSize.width
            let logW = compositorBg.width
            let dpr = (srcW > 0 && logW > 0) ? (srcW / logW) : Screen.devicePixelRatio

            compositor.width = rect.width
            compositor.height = rect.height

            compositorBg.x = -rect.x
            compositorBg.y = -rect.y

            annotationLayer.parent = annotationWrapper
            annotationLayer.anchors.fill = annotationWrapper

            annotationLayer.deselectAll()

            let outW = Math.max(1, Math.ceil(rect.width * dpr))
            let outH = Math.max(1, Math.ceil(rect.height * dpr))

            let savePath = screenCapture.generateTempPath("png")
            Qt.callLater(function () {
                if (activeCompositionId !== compositionId) {
                    return
                }
                compositor.grabToImage(function (result) {
                    if (activeCompositionId !== compositionId) {
                        return
                    }

                    restoreAnnotationLayerParent()
                    activeCompositionId = -1

                    if (result.saveToFile(savePath)) {
                        screenCapture.submitCompositedCapture(savePath, action, rect.x, rect.y, rect.width, rect.height)
                    } else {
                        console.error("Failed to save composite image")
                        if (overlayWindow) {
                            overlayWindow.hide()
                            overlayWindow.resetState()
                        }
                    }

                    finishComposition()
                }, Qt.size(outW, outH))
            })
            return
        }

        screenCapture.submitCapture(overlayWindow.backgroundImageSource, action, rect.x, rect.y, rect.width, rect.height)
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
                source: overlayWindow ? overlayWindow.backgroundImageSource : ""
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

    Connections {
        target: screenCapture
        function onPinWindowRequested(path, x, y, w, h, autoOcr) {
            showPin(path, Qt.rect(x,y,w,h), autoOcr)
        }
    }
}
