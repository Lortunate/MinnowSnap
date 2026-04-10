#include "macos_window_utils.hpp"
#include <QWindow>
#include <QDebug>
#ifdef __APPLE__
#include <Cocoa/Cocoa.h>

// Private helper to get NSWindow from QWindow pointer
static NSWindow* get_nswindow(size_t window_ptr, const char* caller_name) {
    QObject* object = reinterpret_cast<QObject*>(window_ptr);
    QWindow* window = qobject_cast<QWindow*>(object);
    if (!window) {
        qWarning() << caller_name << ": Not a QWindow";
        return nil;
    }

    // Ensure the platform window is created
    if (!window->handle()) {
        window->create();
    }

    // Get the NSView from QWindow
    NSView* view = (__bridge NSView*)reinterpret_cast<void*>(window->winId());
    if (!view) {
        qWarning() << caller_name << ": No view";
        return nil;
    }

    NSWindow* nsWindow = [view window];
    if (!nsWindow) {
        qWarning() << caller_name << ": No NSWindow";
    }
    return nsWindow;
}

void setup_macos_window(size_t window_ptr) {
    NSWindow* nsWindow = get_nswindow(window_ptr, "setup_macos_window");
    if (!nsWindow) return;

    // Set Window Level to Shielding Level (covers Menu Bar and Dock)
    [nsWindow setLevel:CGShieldingWindowLevel()];

    // Set Collection Behavior
    [nsWindow setCollectionBehavior:
        NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorStationary |
        NSWindowCollectionBehaviorIgnoresCycle
    ];

    // Disable shadow for overlay
    [nsWindow setHasShadow:NO];
}

void setup_unified_titlebar(size_t window_ptr) {
    NSWindow* nsWindow = get_nswindow(window_ptr, "setup_unified_titlebar");
    if (!nsWindow) return;

    // Unified title bar style
    NSWindowStyleMask styleMask = [nsWindow styleMask];
    styleMask |= NSWindowStyleMaskTitled;
    styleMask |= NSWindowStyleMaskClosable;
    styleMask |= NSWindowStyleMaskFullSizeContentView;

    // Exclude Miniaturizable and Resizable to hide those buttons.
    // Also explicitly remove UtilityWindow to ensure standard sized traffic lights.
    styleMask &= ~NSWindowStyleMaskMiniaturizable;
    styleMask &= ~NSWindowStyleMaskResizable;
    styleMask &= ~NSWindowStyleMaskUtilityWindow;

    [nsWindow setStyleMask:styleMask];

    [nsWindow setTitlebarAppearsTransparent:YES];
    [nsWindow setTitleVisibility:NSWindowTitleHidden];

    // Ensure only the close button is visible
    [[nsWindow standardWindowButton:NSWindowCloseButton] setHidden:NO];
    [[nsWindow standardWindowButton:NSWindowMiniaturizeButton] setHidden:YES];
    [[nsWindow standardWindowButton:NSWindowZoomButton] setHidden:YES];
}
#endif
