pragma Singleton
import QtQuick
import com.lortunate.minnow

QtObject {
    id: theme

    readonly property bool isDark: {
        if (Config.theme === "Dark")
            return true;
        if (Config.theme === "Light")
            return false;
        return Qt.styleHints.colorScheme === Qt.ColorScheme.Dark;
    }

    // Surfaces
    readonly property color background: isDark ? "#1E1E1E" : "#FFFFFF"
    readonly property color border: isDark ? "#404040" : "#E0E0E0"
    readonly property color danger: "#D32F2F"
    readonly property color divider: border
    readonly property color icon: text

    // Interactive
    readonly property color itemHover: isDark ? "#2D2D30" : "#EAEAEA"
    readonly property color itemPress: isDark ? "#303035" : "#E0E0E0"
    readonly property color itemSelected: isDark ? "#37373D" : "#E4E4E4"
    readonly property color overlayMask: "#80000000"
    readonly property real opacityDisabled: 0.4

    // Brand / Status
    readonly property color primary: "#007AFF"
    readonly property color primaryHover: Qt.lighter(primary, 1.1)
    readonly property color primaryText: "#FFFFFF"

    // Selection / Overlay
    readonly property color selection: primary
    readonly property color selectionFill: Qt.rgba(primary.r, primary.g, primary.b, 0.2)
    readonly property color subText: isDark ? "#AAAAAA" : "#666666"
    readonly property color success: "#388E3C"
    readonly property color surface: isDark ? "#252526" : "#F3F3F3"

    // Content
    readonly property color text: isDark ? "#FFFFFF" : "#333333"
    readonly property color warning: "#F57C00"

    // Form / Grouped List
    readonly property color formBackground: isDark ? background : surface
    readonly property color cardBackground: isDark ? surface : background
    readonly property color cardBorder: isDark ? "#353535" : "#E5E5E5"

    // Shadow Colors
    readonly property color shadowLight: isDark ? "#40000000" : "#10000000"
    readonly property color shadowColor: isDark ? "#60000000" : "#20000000"
    readonly property color shadowMedium: isDark ? "#80000000" : "#30000000"
    readonly property color shadowHeavy: isDark ? "#A0000000" : "#40000000"

    // Typography
    property string fontFamily: Config.fontFamily !== "" ? Config.fontFamily : "Helvetica Neue, Helvetica, Arial, sans-serif"
    readonly property string fontFamilyMono: "SF Mono, Menlo, Monaco, Consolas, monospace"
    readonly property int fontSizeSmall: 11
    readonly property int fontSizeBody: 13
    readonly property int fontSizeLarge: 20

    // Spacing
    readonly property int spacingTiny: 4
    readonly property int spacingSmall: 8
    readonly property int spacingMedium: 16
    readonly property int spacingLarge: 20
    readonly property int spacingXLarge: 24

    // Border Radius
    readonly property int radiusSmall: 4
    readonly property int radiusMedium: 6
    readonly property int radiusLarge: 8
    readonly property int radiusXLarge: 10

    // Animation Durations
    readonly property int durationFast: 100
    readonly property int durationNormal: 150
    readonly property int durationSlow: 200

    // Component Sizing
    readonly property int buttonHeight: 28
    readonly property int inputHeight: 24
    readonly property int settingItemHeight: 48
    readonly property int settingItemHeightTall: 58
    readonly property int cardPadding: 12

    // Specialized Component Sizes
    readonly property int toolbarButtonSize: 36
    readonly property int annotationHandleSize: 12
    readonly property int annotationPropertiesHeight: 36
    readonly property int comboBoxItemHeight: 36
    readonly property int sidebarItemHeight: 32
    readonly property int tooltipPadding: 12
    readonly property int iconSizeSmall: 12
    readonly property int iconSizeDefault: 16
    readonly property int iconSizeMedium: 18
    readonly property int iconSizeLarge: 24
}
