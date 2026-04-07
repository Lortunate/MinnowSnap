import QtQuick
import QtQuick.Controls.Basic as Basic
import com.lortunate.minnow

Basic.Button {
    id: root

    enum Variant {
        Primary,
        Secondary,
        Outline,
        Text
    }

    property int variant: StyledButton.Variant.Secondary
    property color customColor: "transparent"

    implicitHeight: AppTheme.buttonHeight
    font.family: AppTheme.fontFamily
    font.pixelSize: AppTheme.fontSizeBody
    padding: AppTheme.spacingSmall
    leftPadding: AppTheme.spacingMedium
    rightPadding: AppTheme.spacingMedium

    background: Rectangle {
        implicitHeight: AppTheme.buttonHeight
        radius: AppTheme.radiusMedium

        color: {
            if (root.variant === StyledButton.Variant.Primary) {
                return root.down ? Qt.darker(AppTheme.primary, 1.1) : (root.hovered ? AppTheme.primaryHover : AppTheme.primary);
            } else if (root.variant === StyledButton.Variant.Outline) {
                return root.down ? AppTheme.itemPress : (root.hovered ? AppTheme.itemHover : "transparent");
            } else if (root.variant === StyledButton.Variant.Text) {
                return root.down ? AppTheme.itemPress : (root.hovered ? AppTheme.itemHover : "transparent");
            } else { // Secondary
                return root.down ? AppTheme.itemPress : (root.hovered ? AppTheme.itemHover : "transparent");
            }
        }

        border.width: root.variant === StyledButton.Variant.Outline ? 1 : 0
        border.color: root.variant === StyledButton.Variant.Outline ? AppTheme.border : "transparent"

        Behavior on color {
            ColorAnimation {
                duration: AppTheme.durationFast
            }
        }
    }

    contentItem: Text {
        text: root.text
        font: root.font
        color: {
            if (root.variant === StyledButton.Variant.Primary) {
                return AppTheme.primaryText;
            } else if (root.variant === StyledButton.Variant.Text) {
                return root.down ? AppTheme.text : (root.hovered ? AppTheme.text : AppTheme.subText);
            } else {
                return AppTheme.text;
            }
        }
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
}
