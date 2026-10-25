//! Linux launcher registration from assets embedded in the executable.

use std::{fs, io, path::Path};

/// Registers the current executable and bundled icon before creating its window.
///
/// # Returns
/// `Ok(())` after the launcher and icon match the running executable.
///
/// # Errors
/// Returns an error if the data directory or executable path is unavailable,
/// or the launcher and icon cannot be written.
pub(crate) fn register() -> io::Result<()> {
    let data = eframe::storage_dir("eclock")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no application data directory"))?;
    register_in(&data, &std::env::current_exe()?)
}

/// Writes the icon and launcher when their bundled content or location changes.
///
/// # Arguments
/// - `data` - The application's absolute data directory, beside `applications`.
/// - `executable` - Absolute path of the executable the launcher must start.
///
/// # Returns
/// `Ok(())` when both assets are current, without rewriting unchanged files.
///
/// # Errors
/// Returns an error if paths are not UTF-8 or a directory/file cannot be written.
fn register_in(data: &Path, executable: &Path) -> io::Result<()> {
    let applications = data.with_file_name("applications");
    let icon = data.join("icon.svg");
    let launcher = applications.join("eclock.desktop");
    let executable = executable
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 executable path"))?;
    let icon_path = icon
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 icon path"))?;
    let entry = include_str!("../assets/eclock.desktop")
        .replace(
            "Exec=eclock\n",
            &format!("Exec={}\n", exec_argument(executable)),
        )
        .replace(
            "Icon=eclock\n",
            &format!("Icon={}\n", desktop_string(icon_path)),
        );

    fs::create_dir_all(data)?;
    fs::create_dir_all(applications)?;
    let svg = include_bytes!("../assets/icon.svg");
    let icon_changed = fs::read(&icon).ok().as_deref() != Some(svg.as_slice());
    if icon_changed {
        fs::write(icon, svg)?;
    }
    // Rewriting the launcher also tells the desktop to reload a changed icon.
    if icon_changed || fs::read(&launcher).ok().as_deref() != Some(entry.as_bytes()) {
        fs::write(launcher, entry)?;
    }
    Ok(())
}

/// Escapes a desktop-entry string value, before any Exec argument parsing.
///
/// # Returns
/// The value with desktop-entry backslash and whitespace escapes applied.
fn desktop_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Quotes an executable path using the desktop entry's two escaping layers.
///
/// # Returns
/// A single quoted Exec argument with literal percent signs and metacharacters.
fn exec_argument(value: &str) -> String {
    let mut quoted = String::from("\"");
    for character in value.chars() {
        if matches!(character, '\\' | '"' | '`' | '$') {
            quoted.push('\\');
        }
        quoted.push(character);
        if character == '%' {
            quoted.push('%');
        }
    }
    quoted.push('"');
    desktop_string(&quoted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn registration_creates_updates_and_reuses_bundled_desktop_assets() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/desktop-registration-test")
            .join(std::process::id().to_string());
        let data = root.join("data/eclock");
        let executable = root.join("installed clock/bin/eclock");
        let launcher = root.join("data/applications/eclock.desktop");
        let icon = data.join("icon.svg");
        assert!(!launcher.exists());
        register_in(&data, &executable).unwrap();
        let entry = fs::read_to_string(&launcher).unwrap();
        assert!(entry.contains(&format!("Exec=\"{}\"\n", executable.display())));
        assert!(entry.contains(&format!("Icon={}\n", icon.display())));
        assert!(entry.contains("StartupWMClass=eclock\n"));
        assert_eq!(
            fs::read(&icon).unwrap(),
            include_bytes!("../assets/icon.svg")
        );

        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        for path in [&launcher, &icon] {
            fs::File::open(path).unwrap().set_modified(old).unwrap();
        }
        register_in(&data, &executable).unwrap();
        for path in [&launcher, &icon] {
            assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), old);
        }

        fs::write(&icon, "old icon").unwrap();
        register_in(&data, &executable).unwrap();
        assert_eq!(
            fs::read(&icon).unwrap(),
            include_bytes!("../assets/icon.svg")
        );
        assert!(fs::metadata(&launcher).unwrap().modified().unwrap() > old);

        let moved = root.join("new install/bin/eclock");
        register_in(&data, &moved).unwrap();
        assert!(
            fs::read_to_string(&launcher)
                .unwrap()
                .contains(&format!("Exec=\"{}\"\n", moved.display()))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
