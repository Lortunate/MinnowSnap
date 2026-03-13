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

    resources: [
        LongCaptureFrame {
            id: scrollFrame
            visible: controller.frameVisible
            selectionX: controller.selectionRect.x
            selectionY: controller.selectionRect.y
            selectionWidth: controller.selectionRect.width
            selectionHeight: controller.selectionRect.height
            warningText: controller.warningText
        },
        LongCaptureToolbarWindow {
            id: scrollToolbar
            visible: controller.toolbarVisible
            isBusy: controller.toolbarBusy
            selectionX: controller.selectionRect.x
            selectionY: controller.selectionRect.y
            selectionWidth: controller.selectionRect.width
            selectionHeight: controller.selectionRect.height

            onActionClicked: action => controller.handleToolbarAction(action)
        },
        LongCapturePreviewWindow {
            id: scrollPreview
            visible: controller.previewVisible
            selectionX: controller.selectionRect.x
            selectionY: controller.selectionRect.y
            selectionWidth: controller.selectionRect.width
            selectionHeight: controller.selectionRect.height
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
            controller.start(selectionRect)
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
