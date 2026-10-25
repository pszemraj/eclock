//! Native tray menu and event-driven delivery to the clock's event loop.

use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

/// Window actions requested by the tray icon or its menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Show,
    Hide,
    Quit,
}

/// Keeps the native tray alive and receives actions without polling the UI.
pub(crate) struct Tray {
    _icon: TrayIcon,
    actions: Receiver<Action>,
}

impl Tray {
    /// Creates the tray on the running native event-loop thread.
    ///
    /// # Returns
    /// The tray lifetime guard and action receiver; native events wake `ctx`.
    ///
    /// # Errors
    /// Returns menu creation, icon rendering, or native tray registration errors.
    pub(crate) fn new(ctx: &egui::Context) -> Result<Self, String> {
        let menu = Menu::new();
        let show = MenuItem::new("Show eclock", true, None);
        let hide = MenuItem::new("Hide eclock", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append_items(&[&show, &hide, &quit])
            .map_err(|error| error.to_string())?;
        let data = crate::icon::tray_icon().ok_or("Could not render the tray icon")?;
        let icon = Icon::from_rgba(data.rgba, data.width, data.height)
            .map_err(|error| error.to_string())?;
        let builder = TrayIconBuilder::new()
            .with_id("eclock")
            .with_tooltip("eclock")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_icon(icon);
        let icon = builder.build().map_err(|error| error.to_string())?;

        let (sender, actions) = mpsc::channel();
        let menu_sender = sender.clone();
        let menu_ctx = ctx.clone();
        let show_id = show.id().clone();
        let hide_id = hide.id().clone();
        let quit_id = quit.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id == show_id {
                Action::Show
            } else if event.id == hide_id {
                Action::Hide
            } else if event.id == quit_id {
                Action::Quit
            } else {
                return;
            };
            let _ = menu_sender.send(action);
            menu_ctx.request_repaint();
        }));
        let tray_ctx = ctx.clone();
        let tray_id = icon.id().clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(event, TrayIconEvent::Click {
                id, button: MouseButton::Left, button_state: MouseButtonState::Up, ..
            } if id == tray_id)
            {
                let _ = sender.send(Action::Show);
                tray_ctx.request_repaint();
            }
        }));
        Ok(Self {
            _icon: icon,
            actions,
        })
    }

    /// Takes the next pending native action without blocking.
    ///
    /// # Returns
    /// The next Show, Hide, or Quit action, or `None` when none is pending.
    pub(crate) fn next_action(&self) -> Option<Action> {
        self.actions.try_recv().ok()
    }
}

/// Reports whether Linux exposes a registered StatusNotifier tray host.
///
/// # Returns
/// Whether the session bus reports a tray host; `false` if absent or unreachable.
#[cfg(target_os = "linux")]
pub(crate) fn linux_host_available() -> bool {
    let Ok(connection) = zbus::blocking::connection::Builder::session().and_then(|builder| {
        builder
            .method_timeout(std::time::Duration::from_secs(1))
            .build()
    }) else {
        return false;
    };
    zbus::blocking::Proxy::new(
        &connection,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .and_then(|proxy| proxy.get_property::<bool>("IsStatusNotifierHostRegistered"))
    .unwrap_or(false)
}
