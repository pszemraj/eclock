//! Native backend selection and platform-specific application presence.

/// Checks whether tray-backed window hide/restore can be attempted.
///
/// # Returns
/// On Linux, `true` when X11 and a registered StatusNotifier host are available.
/// On other supported platforms, `true` to attempt native tray creation.
pub(crate) fn tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    return x11rb::connect(None).is_ok() && crate::tray::linux_host_available();
    #[cfg(not(target_os = "linux"))]
    true
}

/// Selects X11 for Linux tray hide/restore and accessory activation on macOS.
/// Linux windows retain their ordinary taskbar presence and tiling eligibility.
///
/// # Arguments
/// - `options` - Native options whose event-loop configuration is updated in place.
/// - `tray_supported` - Enables the X11 backend on Linux; unused on other platforms.
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

/// Sets whether the Windows window is excluded from the taskbar.
///
/// # Arguments
/// - `window` - Native window whose taskbar presence is updated.
/// - `hidden` - `true` to hide the entry, or `false` to restore it.
#[cfg(target_os = "windows")]
pub(crate) fn set_taskbar_hidden(window: &winit::window::Window, hidden: bool) {
    use winit::platform::windows::WindowExtWindows;
    window.set_skip_taskbar(hidden);
}
