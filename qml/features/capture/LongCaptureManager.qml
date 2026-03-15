import QtQuick
import QtCore
import com.lortunate.minnow

Item {
    id: root

    property var screenCapture

    signal requestHide
    signal requestReset

    LongCaptureController {
        id: controller
    }

    function resolveViewport() {
        let rect = Qt.rect(0, 0, 0, 0)
        let scale = 1.0
        if (screenCapture) {
            rect = screenCapture.captureScreenRect
            scale = screenCapture.captureScreenScale > 0 ? screenCapture.captureScreenScale : 1.0
        }
        if (rect.width <= 0 || rect.height <= 0) {
            let s = Qt.application.primaryScreen
            if (s) {
                rect = Qt.rect(s.virtualX, s.virtualY, s.width, s.height)
                scale = s.devicePixelRatio > 0 ? s.devicePixelRatio : 1.0
            }
        }
        return {
            rect: rect,
            scale: scale
        }
    }

    resources: [
        LongCaptureFrame {
            id: scrollFrame
            visible: controller.frameVisible
            selectionRect: controller.selectionRect
            viewportRect: controller.viewportRect
            warningText: controller.warningText
        },
        LongCaptureToolbarWindow {
            id: scrollToolbar
            visible: controller.toolbarVisible
            isBusy: controller.toolbarBusy
            selectionRect: controller.selectionRect
            viewportRect: controller.viewportRect

            onActionClicked: action => controller.handleToolbarAction(action)
        },
        LongCapturePreviewWindow {
            id: scrollPreview
            visible: controller.previewVisible
            selectionRect: controller.selectionRect
            viewportRect: controller.viewportRect
            viewportScale: controller.viewportScale
        }
    ]

    Connections {
        target: controller

        function onRequestHideOverlay() {
            root.requestHide()
        }

        function onRequestResetOverlay() {
            root.requestReset()
        }

        function onRequestCancelScrollCapture() {
            if (screenCapture)
                screenCapture.cancelScrollCapture()
        }

        function onRequestScrollAction(action) {
            if (screenCapture)
                screenCapture.requestScrollAction(action)
        }

        function onRequestPreviewRefresh(height) {
            scrollPreview.refresh(height)
        }

        function onRequestFrameFlash() {
            scrollFrame.flashSuccess()
        }
    }

    Connections {
        target: screenCapture

        function onScrollCaptureStarted(selectionRect) {
            const viewport = resolveViewport()
            controller.start(selectionRect, viewport.rect, viewport.scale)
        }

        function onCaptureReady() {
            controller.onCaptureReady()
        }

        function onScrollCaptureFinished() {
            controller.onScrollCaptureFinished()
        }

        function onScrollCaptureUpdated(height) {
            controller.onScrollCaptureUpdated(height)
        }

        function onScrollCaptureWarning(msg) {
            controller.onScrollCaptureWarning(msg)
        }
    }
}
