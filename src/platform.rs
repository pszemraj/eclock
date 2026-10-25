//! Native backend selection and platform-specific application presence.

/// Checks whether tray-backed window hide/restore can be attempted.
///
/// On Linux, requires X11 and a registered StatusNotifier host. Other supported
/// platforms attempt native tray creation inside the event loop.
pub(crate) fn tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    return x11rb::connect(None).is_ok() && crate::tray::linux_host_available();
    #[cfg(not(target_os = "linux"))]
    true
}

/// Selects X11 for Linux tray hide/restore and accessory activation on macOS.
/// Linux windows retain their ordinary taskbar presence and tiling eligibility.
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

/// Hides the Windows taskbar entry only while a native tray is available.
#[cfg(target_os = "windows")]
pub(crate) fn set_taskbar_hidden(window: &winit::window::Window, hidden: bool) {
    use winit::platform::windows::WindowExtWindows;
    window.set_skip_taskbar(hidden);
}
