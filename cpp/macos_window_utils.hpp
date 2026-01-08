#pragma once
#include <stddef.h>

#ifdef __APPLE__
void setup_macos_window(size_t window_ptr);
void setup_unified_titlebar(size_t window_ptr);
#else
inline void setup_macos_window(size_t window_ptr) {}
inline void setup_unified_titlebar(size_t window_ptr) {}
#endif
