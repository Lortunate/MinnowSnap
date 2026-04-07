#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowCandidate {
    pid: u32,
    visible: bool,
    iconic: bool,
    rect: (i32, i32, i32, i32),
}

fn point_in_rect(point: (i32, i32), rect: (i32, i32, i32, i32)) -> bool {
    point.0 >= rect.0 && point.0 < rect.2 && point.1 >= rect.1 && point.1 < rect.3
}

fn is_candidate_acceptable(candidate: &WindowCandidate, current_pid: u32, point: (i32, i32)) -> bool {
    candidate.visible && !candidate.iconic && candidate.pid != current_pid && point_in_rect(point, candidate.rect)
}

#[cfg_attr(not(test), allow(dead_code))]
fn pick_candidate_index(candidates: &[WindowCandidate], current_pid: u32, point: (i32, i32)) -> Option<usize> {
    candidates
        .iter()
        .position(|candidate| is_candidate_acceptable(candidate, current_pid, point))
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
const WHEEL_DELTA: i16 = 120;
#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
const MAX_WHEEL_STEPS: i32 = i16::MAX as i32 / WHEEL_DELTA as i32;
#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
const MIN_WHEEL_STEPS: i32 = i16::MIN as i32 / WHEEL_DELTA as i32;

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn wheel_delta_from_steps(steps: i32) -> i16 {
    let clamped_steps = steps.clamp(MIN_WHEEL_STEPS, MAX_WHEEL_STEPS);
    (clamped_steps * i32::from(WHEEL_DELTA)) as i16
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn resolve_scroll_target_order<T: Copy + Eq>(top_level: T, child: Option<T>) -> (T, Option<T>) {
    let fallback = match child {
        Some(child) if child != top_level => Some(child),
        _ => None,
    };
    (top_level, fallback)
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::{WindowCandidate, is_candidate_acceptable, resolve_scroll_target_order, wheel_delta_from_steps};
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::MapWindowPoints;
    use windows::Win32::UI::WindowsAndMessaging::{
        CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, ChildWindowFromPointEx, GA_ROOT, GW_HWNDNEXT, GetAncestor, GetCursorPos, GetWindow,
        GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEWHEEL,
        WindowFromPoint,
    };

    const MK_MBUTTON_WPARAM: usize = 0x0010;

    pub(crate) fn forward_middle_click_to_underlying() -> Result<(), String> {
        let mut screen_point = POINT::default();
        // SAFETY: GetCursorPos writes to the provided valid pointer.
        unsafe {
            GetCursorPos(&mut screen_point).map_err(|err| format!("GetCursorPos failed: {err}"))?;
        }

        let current_pid = std::process::id();
        let (target, rect) = find_underlying_window_at_point(screen_point, current_pid)
            .ok_or_else(|| "No underlying window found for middle-click forwarding".to_string())?;

        post_middle_click(target, screen_point, rect)
    }

    pub(crate) fn forward_scroll_wheel_to_underlying(steps: i32) -> Result<(), String> {
        let wheel_delta = wheel_delta_from_steps(steps);
        if wheel_delta == 0 {
            return Ok(());
        }

        let mut screen_point = POINT::default();
        // SAFETY: GetCursorPos writes to the provided valid pointer.
        unsafe {
            GetCursorPos(&mut screen_point).map_err(|err| format!("GetCursorPos failed: {err}"))?;
        }

        let current_pid = std::process::id();
        let (top_level, _rect) = find_underlying_window_at_point(screen_point, current_pid)
            .ok_or_else(|| "No underlying window found for scroll forwarding".to_string())?;
        let child = find_deepest_child_at_point(top_level, screen_point);
        let (primary_target, fallback_target) = resolve_scroll_target_order(top_level, child);

        tracing::debug!(
            target: "minnowsnap::io::mouse_forwarding",
            steps,
            wheel_delta,
            point_x = screen_point.x,
            point_y = screen_point.y,
            top_level = ?top_level,
            child = ?child,
            primary_target = ?primary_target,
            fallback_target = ?fallback_target,
            "Forwarding scroll wheel to underlying window",
        );

        match post_scroll_wheel(primary_target, screen_point, wheel_delta) {
            Ok(()) => Ok(()),
            Err(primary_err) => {
                let Some(fallback_target) = fallback_target else {
                    tracing::debug!(
                        target: "minnowsnap::io::mouse_forwarding",
                        steps,
                        wheel_delta,
                        point_x = screen_point.x,
                        point_y = screen_point.y,
                        primary_target = ?primary_target,
                        error = %primary_err,
                        "Primary scroll target failed and no fallback target is available",
                    );
                    return Err(primary_err);
                };

                tracing::debug!(
                    target: "minnowsnap::io::mouse_forwarding",
                    steps,
                    wheel_delta,
                    point_x = screen_point.x,
                    point_y = screen_point.y,
                    primary_target = ?primary_target,
                    fallback_target = ?fallback_target,
                    error = %primary_err,
                    "Primary scroll target failed, retrying fallback target",
                );

                match post_scroll_wheel(fallback_target, screen_point, wheel_delta) {
                    Ok(()) => {
                        tracing::debug!(
                            target: "minnowsnap::io::mouse_forwarding",
                            steps,
                            wheel_delta,
                            point_x = screen_point.x,
                            point_y = screen_point.y,
                            fallback_target = ?fallback_target,
                            "Fallback scroll target succeeded",
                        );
                        Ok(())
                    }
                    Err(fallback_err) => {
                        tracing::debug!(
                            target: "minnowsnap::io::mouse_forwarding",
                            steps,
                            wheel_delta,
                            point_x = screen_point.x,
                            point_y = screen_point.y,
                            primary_target = ?primary_target,
                            fallback_target = ?fallback_target,
                            primary_error = %primary_err,
                            fallback_error = %fallback_err,
                            "Fallback scroll target also failed",
                        );
                        Err(format!(
                            "Failed to post WM_MOUSEWHEEL to primary target {primary_target:?}: {primary_err}; fallback target {fallback_target:?}: {fallback_err}"
                        ))
                    }
                }
            }
        }
    }

    fn find_underlying_window_at_point(screen_point: POINT, current_pid: u32) -> Option<(HWND, RECT)> {
        // SAFETY: WindowFromPoint and GetAncestor are read-only OS queries.
        let topmost = unsafe { WindowFromPoint(screen_point) };
        if topmost.0.is_null() {
            return None;
        }

        // SAFETY: GA_ROOT ascends to the root ancestor of the discovered window.
        let mut cursor = unsafe { GetAncestor(topmost, GA_ROOT) };
        while !cursor.0.is_null() {
            if let Some(rect) = candidate_rect(cursor, current_pid, screen_point) {
                return Some((cursor, rect));
            }

            // SAFETY: GetWindow queries the next window in z-order.
            cursor = unsafe {
                match GetWindow(cursor, GW_HWNDNEXT) {
                    Ok(next) => next,
                    Err(_) => break,
                }
            };
        }

        None
    }

    fn candidate_rect(hwnd: HWND, current_pid: u32, point: POINT) -> Option<RECT> {
        let mut pid = 0u32;
        // SAFETY: GetWindowThreadProcessId only writes pid output.
        unsafe {
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }

        let visible = {
            // SAFETY: IsWindowVisible reads process-managed window state.
            unsafe { IsWindowVisible(hwnd).as_bool() }
        };
        let iconic = {
            // SAFETY: IsIconic reads process-managed window state.
            unsafe { IsIconic(hwnd).as_bool() }
        };

        let mut rect = RECT::default();
        // SAFETY: GetWindowRect writes to valid RECT pointer.
        unsafe {
            GetWindowRect(hwnd, &mut rect).ok()?;
        }

        let candidate = WindowCandidate {
            pid,
            visible,
            iconic,
            rect: (rect.left, rect.top, rect.right, rect.bottom),
        };
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return None;
        }

        if is_candidate_acceptable(&candidate, current_pid, (point.x, point.y)) {
            Some(rect)
        } else {
            None
        }
    }

    fn find_deepest_child_at_point(root: HWND, screen_point: POINT) -> Option<HWND> {
        let flags = CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT;
        let mut current = root;

        loop {
            let (client_x, client_y) = to_client_point(current, screen_point)?;
            let client_point = POINT { x: client_x, y: client_y };

            // SAFETY: ChildWindowFromPointEx performs a read-only hit-test in window hierarchy.
            let next = unsafe { ChildWindowFromPointEx(current, client_point, flags) };
            if next.0.is_null() || next == current {
                break;
            }

            current = next;
        }

        (current != root).then_some(current)
    }

    fn post_middle_click(hwnd: HWND, screen_point: POINT, window_rect: RECT) -> Result<(), String> {
        let (client_x, client_y) =
            to_client_point(hwnd, screen_point).unwrap_or((screen_point.x - window_rect.left, screen_point.y - window_rect.top));
        let lparam = make_lparam(client_x, client_y);

        // SAFETY: PostMessageW is called with a live HWND and plain message payload.
        unsafe {
            PostMessageW(Some(hwnd), WM_MBUTTONDOWN, WPARAM(MK_MBUTTON_WPARAM), LPARAM(lparam))
                .map_err(|err| format!("Failed to post WM_MBUTTONDOWN: {err}"))?;
            PostMessageW(Some(hwnd), WM_MBUTTONUP, WPARAM(0), LPARAM(lparam)).map_err(|err| format!("Failed to post WM_MBUTTONUP: {err}"))?;
        }

        Ok(())
    }

    fn post_scroll_wheel(hwnd: HWND, screen_point: POINT, wheel_delta: i16) -> Result<(), String> {
        let wparam = make_mouse_wheel_wparam(wheel_delta);
        let lparam = make_lparam(screen_point.x, screen_point.y);

        // SAFETY: PostMessageW is called with a live HWND and plain message payload.
        unsafe {
            PostMessageW(Some(hwnd), WM_MOUSEWHEEL, WPARAM(wparam), LPARAM(lparam)).map_err(|err| format!("Failed to post WM_MOUSEWHEEL: {err}"))?;
        }

        Ok(())
    }

    fn to_client_point(hwnd: HWND, screen_point: POINT) -> Option<(i32, i32)> {
        let mut points = [screen_point];
        // SAFETY: MapWindowPoints converts the provided point in place from screen-space into hwnd client-space.
        let _ = unsafe { MapWindowPoints(None, Some(hwnd), &mut points) };
        Some((points[0].x, points[0].y))
    }

    fn make_lparam(x: i32, y: i32) -> isize {
        let x_bits = (x as u16) as u32;
        let y_bits = (y as u16) as u32;
        ((y_bits << 16) | x_bits) as isize
    }

    fn make_mouse_wheel_wparam(wheel_delta: i16) -> usize {
        let delta_bits = (wheel_delta as u16) as u32;
        (delta_bits << 16) as usize
    }
}

#[cfg(target_os = "windows")]
pub(crate) use windows_impl::{forward_middle_click_to_underlying, forward_scroll_wheel_to_underlying};

#[cfg(not(target_os = "windows"))]
pub(crate) fn forward_middle_click_to_underlying() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn forward_scroll_wheel_to_underlying(_steps: i32) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_scroll_target_order;
    use super::{WindowCandidate, pick_candidate_index, wheel_delta_from_steps};

    #[test]
    fn candidate_filter_skips_current_process_window() {
        let current_pid = 100u32;
        let point = (120, 120);
        let candidates = [
            WindowCandidate {
                pid: current_pid,
                visible: true,
                iconic: false,
                rect: (0, 0, 400, 400),
            },
            WindowCandidate {
                pid: 200,
                visible: true,
                iconic: false,
                rect: (0, 0, 400, 400),
            },
        ];

        let picked = pick_candidate_index(&candidates, current_pid, point);
        assert_eq!(picked, Some(1));
    }

    #[test]
    fn candidate_filter_returns_none_for_empty_or_invalid_inputs() {
        let current_pid = 100u32;
        let point = (50, 50);
        let empty: [WindowCandidate; 0] = [];
        assert_eq!(pick_candidate_index(&empty, current_pid, point), None);

        let invalid = [WindowCandidate {
            pid: 200,
            visible: false,
            iconic: false,
            rect: (0, 0, 100, 100),
        }];
        assert_eq!(pick_candidate_index(&invalid, current_pid, point), None);
    }

    #[test]
    fn candidate_filter_uses_left_top_inclusive_right_bottom_exclusive_bounds() {
        let current_pid = 100u32;
        let candidate = [WindowCandidate {
            pid: 200,
            visible: true,
            iconic: false,
            rect: (10, 10, 20, 20),
        }];

        assert_eq!(pick_candidate_index(&candidate, current_pid, (10, 10)), Some(0));
        assert_eq!(pick_candidate_index(&candidate, current_pid, (19, 19)), Some(0));
        assert_eq!(pick_candidate_index(&candidate, current_pid, (20, 10)), None);
        assert_eq!(pick_candidate_index(&candidate, current_pid, (10, 20)), None);
    }

    #[test]
    fn wheel_delta_scales_and_clamps_step_values() {
        assert_eq!(wheel_delta_from_steps(1), 120);
        assert_eq!(wheel_delta_from_steps(-1), -120);
        assert_eq!(wheel_delta_from_steps(0), 0);
        assert_eq!(wheel_delta_from_steps(i32::MAX), 32760);
        assert_eq!(wheel_delta_from_steps(i32::MIN), -32760);
    }

    #[test]
    fn scroll_target_order_prefers_top_level_and_uses_child_as_fallback() {
        assert_eq!(resolve_scroll_target_order(11usize, Some(42usize)), (11usize, Some(42usize)));
    }

    #[test]
    fn scroll_target_order_omits_fallback_when_child_missing_or_same() {
        assert_eq!(resolve_scroll_target_order(11usize, None), (11usize, None));
        assert_eq!(resolve_scroll_target_order(11usize, Some(11usize)), (11usize, None));
    }
}
