use core::pin::Pin;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cpp/macos_window_utils.hpp");
        /// # Safety
        /// This function operates on raw pointers and Objective-C objects.
        /// Caller must ensure that window_ptr is a valid pointer to a QWindow/NSWindow.
        #[cxx_name = "setup_macos_window"]
        unsafe fn setup_macos_window_cpp(window_ptr: usize);

        /// # Safety
        /// This function operates on raw pointers and Objective-C objects.
        /// Caller must ensure that window_ptr is a valid pointer to a QWindow/NSWindow.
        #[cxx_name = "setup_unified_titlebar"]
        unsafe fn setup_unified_titlebar_cpp(window_ptr: usize);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type WindowHelper = super::WindowHelperRust;

        #[qinvokable]
        #[cxx_name = "setupUnifiedTitlebar"]
        unsafe fn setup_unified_titlebar(self: Pin<&mut WindowHelper>, window: *mut QObject);
    }
}

pub struct WindowHelperRust;

impl Default for WindowHelperRust {
    fn default() -> Self {
        Self
    }
}

impl ffi::WindowHelper {
    pub fn setup_unified_titlebar(self: Pin<&mut Self>, window: *mut ffi::QObject) {
        #[cfg(target_os = "macos")]
        unsafe {
            let ptr = window as usize;
            ffi::setup_unified_titlebar_cpp(ptr);
        }
    }
}

pub fn setup_macos_window(ptr: usize) {
    unsafe {
        ffi::setup_macos_window_cpp(ptr);
    }
}
