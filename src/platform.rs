//! Native backend selection and taskbar presence for an ordinary tiled window.

/// Checks whether tray-backed window hide/restore can be attempted.
///
/// # Returns
/// On Linux, whether X11 and a registered StatusNotifier host are available.
/// Other supported platforms return `true`; tray creation checks native support.
pub(crate) fn tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    return x11rb::connect(None).is_ok() && crate::tray::linux_host_available();
    #[cfg(not(target_os = "linux"))]
    true
}

/// Selects X11 for Linux tray operation and accessory activation on macOS.
///
/// # Arguments
/// - `options` - Native options to configure before creating the event loop.
/// - `tray_supported` - Whether Linux should use X11 for tray hide/restore.
pub(crate) fn configure(options: &mut eframe::NativeOptions, tray_supported: bool) {
    #[cfg(target_os = "linux")]
    if tray_supported {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11();
        }));
    }
    #[cfg(target_os = "macos")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }
    let _ = (options, tray_supported);
}

/// Updates taskbar exclusion, preserving X11 fullscreen and other state hints.
///
/// macOS uses activation policy instead.
///
/// # Arguments
/// - `window` - Native window whose taskbar presence should change.
/// - `hidden` - Whether to exclude the window from the taskbar.
///
/// # Returns
/// `Ok(())` after submitting the hint, or when this backend does not use it.
///
/// # Errors
/// Returns native handle, X11 connection, or protocol errors.
pub(crate) fn set_taskbar_hidden(
    window: &winit::window::Window,
    hidden: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::WindowExtWindows;
        window.set_skip_taskbar(hidden);
    }
    #[cfg(target_os = "linux")]
    {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let handle = window.window_handle().map_err(|error| error.to_string())?;
        let id = match handle.as_raw() {
            RawWindowHandle::Xlib(handle) => {
                u32::try_from(handle.window).map_err(|error| error.to_string())?
            }
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            _ => return Ok(()),
        };
        // The X11 event loop and this connection both use the unchanged DISPLAY.
        update_x11_taskbar(id, hidden).map_err(|error| error.to_string())?;
    }
    let _ = (window, hidden);
    Ok(())
}

/// Adds or removes only the taskbar hint from an unmapped window's state.
#[cfg(any(target_os = "linux", test))]
fn taskbar_state(current: &[u32], skip: u32, hidden: bool) -> Vec<u32> {
    let mut state: Vec<_> = current
        .iter()
        .copied()
        .filter(|atom| *atom != skip)
        .collect();
    if hidden {
        state.push(skip);
    }
    state
}

/// Applies the EWMH hint before mapping, or asks the window manager to update it.
#[cfg(target_os = "linux")]
fn update_x11_taskbar(window: u32, hidden: bool) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{
            AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, MapState, PropMode,
        },
        wrapper::ConnectionExt as _,
    };
    let (connection, _) = x11rb::connect(None)?;
    let state = connection
        .intern_atom(false, b"_NET_WM_STATE")?
        .reply()?
        .atom;
    let skip = connection
        .intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?
        .reply()?
        .atom;
    if connection.get_window_attributes(window)?.reply()?.map_state == MapState::UNMAPPED {
        let reply = connection
            .get_property(false, window, state, AtomEnum::ATOM, 0, u32::MAX)?
            .reply()?;
        let current: Vec<_> = reply.value32().into_iter().flatten().collect();
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                state,
                AtomEnum::ATOM,
                &taskbar_state(&current, skip, hidden),
            )?
            .check()?;
    } else {
        let root = connection.get_geometry(window)?.reply()?.root;
        let event = ClientMessageEvent::new(32, window, state, [u32::from(hidden), skip, 0, 1, 0]);
        connection
            .send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )?
            .check()?;
    }
    connection.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::taskbar_state;

    #[test]
    fn taskbar_changes_preserve_fullscreen_and_unknown_states() {
        let original = [11, 22, 99];
        let hidden = taskbar_state(&original, 33, true);
        assert_eq!(hidden, [11, 22, 99, 33]);
        assert_eq!(taskbar_state(&hidden, 33, true), hidden);
        assert_eq!(taskbar_state(&hidden, 33, false), original);
    }
}
