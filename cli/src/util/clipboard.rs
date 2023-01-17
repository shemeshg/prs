use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
))]
use copypasta_ext::display::DisplayServer;
use copypasta_ext::prelude::*;
#[cfg(all(feature = "notify", target_os = "linux", not(target_env = "musl")))]
use notify_rust::Hint;
#[cfg(all(feature = "notify", not(target_env = "musl")))]
use notify_rust::Notification;
use prs_lib::Plaintext;
use thiserror::Error;

use crate::util::error::{self, ErrorHintsBuilder};

/// Get clipboard contents.
///
/// If clipboard is unset, an emtpy string is returned.
pub fn get() -> Result<String> {
    let mut ctx = copypasta_ext::x11_fork::ClipboardContext::new().map_err(Err::Clipboard)?;
    Ok(ctx.get_contents().unwrap_or_else(|_| String::new()))
}

/// Set clipboard contents.
pub fn set(data: &[u8]) -> Result<()> {
    let mut ctx = copypasta_ext::x11_fork::ClipboardContext::new().map_err(Err::Clipboard)?;
    ctx.set_contents(std::str::from_utf8(data).unwrap().into())
        .map_err(|err| Err::Clipboard(err).into())
}

/// Copy the given plain text to the user clipboard.
#[allow(unreachable_code)]
pub fn copy_timeout(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    if timeout == 0 {
        return set(data);
    }

    // macOS
    #[cfg(target_os = "macos")]
    return copy_timeout_macos(data, timeout, report);

    // Windows
    #[cfg(target_os = "windows")]
    return copy_timeout_windows(data, timeout, report);

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
    ))]
    if is_wayland() {
        return copy_timeout_wayland_bin(data, timeout, report);
    }

    // X11 with musl
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
        target_env = "musl",
    ))]
    return copy_timeout_x11_bin(data, timeout, report);

    // X11
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
        not(target_env = "musl"),
    ))]
    return copy_timeout_x11(data, timeout, report);

    // Other clipboard contexts
    copy_timeout_blocking(data, timeout, report)
}

/// Copy with timeout on X11.
///
/// Keeps clipboard contents in clipboard even if application quits. Doesn't fuck with other
/// clipboard contents and reverts back to previous contents once a timeout is reached.
///
/// Forks & detaches two processes to set/keep clipboard contents and to drive the timeout.
///
/// Based on: https://docs.rs/copypasta-ext/0.3.4/copypasta_ext/x11_fork/index.html
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
    not(target_env = "musl")
))]
fn copy_timeout_x11(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    use copypasta_ext::x11_fork::{ClipboardContext, Error};
    use x11_clipboard::Clipboard as X11Clipboard;

    // Remember previous clipboard contents
    let mut ctx = ClipboardContext::new().map_err(Err::Clipboard)?;
    let previous = ctx.get_contents().unwrap_or_else(|_| String::new());

    let bin = crate::util::bin_name();

    // Detach fork to set given clipboard contents, keeps in clipboard until changed
    let setter_pid = match unsafe { libc::fork() } {
        -1 => return Err(Error::Fork.into()),
        0 => {
            // Obtain new X11 clipboard context, set clipboard contents
            let clip = X11Clipboard::new()
                .unwrap_or_else(|_| panic!("{}: failed to obtain X11 clipboard context", bin));
            clip.store(
                clip.setter.atoms.clipboard,
                clip.setter.atoms.utf8_string,
                data,
            )
            .unwrap_or_else(|_| {
                panic!(
                    "{}: failed to set clipboard contents through forked process",
                    bin,
                )
            });

            // Wait for clipboard to change, then kill fork
            clip.load_wait(
                clip.getter.atoms.clipboard,
                clip.getter.atoms.utf8_string,
                clip.getter.atoms.property,
            )
            .unwrap_or_else(|_| {
                panic!(
                    "{}: failed to wait on new clipboard value in forked process",
                    bin,
                )
            });

            // Update cleared state, show notification
            let _ = notify_cleared();

            error::quit();
        }
        pid => pid,
    };

    // Detach fork to revert clipboard after timeout unless changed
    match unsafe { libc::fork() } {
        -1 => return Err(Error::Fork.into()),
        0 => {
            thread::sleep(Duration::from_secs(timeout));

            // Determine if clipboard is already cleared, which is the case if the fork that set
            // the clipboard has died
            let cleared = unsafe {
                let pid_search_status = libc::kill(setter_pid, 0);
                let errno = *libc::__errno_location() as i32;
                pid_search_status == -1 && errno == libc::ESRCH
            };

            // Revert to previous clipboard contents if not yet cleared
            if !cleared {
                let mut ctx = ClipboardContext::new()
                    .unwrap_or_else(|_| panic!("{}: failed to obtain X11 clipboard context", bin,));
                ctx.set_contents(previous).unwrap_or_else(|_| {
                    panic!(
                        "{}: failed to revert clipboard contents through forked process",
                        bin,
                    )
                });
            }

            error::quit();
        }
        _pid => {}
    }

    if report {
        eprintln!(
            "Secret copied to clipboard. Clearing after {} seconds...",
            timeout
        );
    }

    Ok(())
}

/// Copy with timeout on Wayland using the wl-copy and wl-paste binaries.
///
/// Keeps clipboard contents in clipboard even if application quits. Doesn't fuck with other
/// clipboard contents and reverts back to previous contents once a timeout is reached.
///
/// Forks & detaches two processes to set/keep clipboard contents and to drive the timeout.
///
/// Based on: https://docs.rs/copypasta-ext/0.3.4/copypasta_ext/wayland_fork/index.html
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
))]
fn copy_timeout_wayland_bin(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    use copypasta_ext::wayland_bin::WaylandBinClipboardContext as ClipboardContext;

    let data = std::str::from_utf8(data).map_err(Err::Utf8)?;
    let bin = crate::util::bin_name();

    // Remember previous clipboard contents
    let mut ctx = ClipboardContext::new().map_err(Err::Clipboard)?;
    let previous = ctx.get_contents().unwrap_or_else(|_| String::new());

    // Set clipboard
    ctx.set_contents(data.to_string()).map_err(Err::Clipboard)?;

    // Detach fork to revert clipboard after timeout unless changed
    match unsafe { libc::fork() } {
        -1 => panic!("failed to fork"),
        0 => {
            thread::sleep(Duration::from_secs(timeout));

            // Obtain new clipboard context, get current contents
            let mut ctx = ClipboardContext::new()
                .unwrap_or_else(|_| panic!("{}: failed to obtain Wayland clipboard context", bin,));
            let now = ctx.get_contents().unwrap_or_else(|_| {
                panic!(
                    "{}: failed to get clipboard contents through forked process",
                    bin,
                )
            });

            // If clipboard contents didn't change, revert back to previous
            if data == now {
                ctx.set_contents(previous).unwrap_or_else(|_| {
                    panic!(
                        "{}: failed to revert clipboard contents through forked process",
                        bin,
                    )
                });

                // Update cleared state, show notification
                let _ = notify_cleared();
            }

            error::quit();
        }
        _pid => {}
    }

    if report {
        eprintln!(
            "Secret copied to clipboard. Clearing after {} seconds...",
            timeout
        );
    }

    Ok(())
}

/// Copy with timeout on X11 using xclip or xsel binaries.
///
/// Keeps clipboard contents in clipboard even if application quits. Doesn't fuck with other
/// clipboard contents and reverts back to previous contents once a timeout is reached.
///
/// Forks & detaches two processes to set/keep clipboard contents and to drive the timeout.
///
/// Based on: https://docs.rs/copypasta-ext/0.3.4/copypasta_ext/x11_fork/index.html
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
    target_env = "musl",
))]
fn copy_timeout_x11_bin(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    use copypasta_ext::x11_bin::X11BinClipboardContext as ClipboardContext;

    let data = std::str::from_utf8(data).map_err(Err::Utf8)?;
    let bin = crate::util::bin_name();

    // Remember previous clipboard contents
    let mut ctx = ClipboardContext::new().map_err(Err::Clipboard)?;
    let previous = ctx.get_contents().unwrap_or_else(|_| String::new());

    // Set clipboard
    ctx.set_contents(data.to_string()).map_err(Err::Clipboard)?;

    // Detach fork to revert clipboard after timeout unless changed
    match unsafe { libc::fork() } {
        -1 => panic!("failed to fork"),
        0 => {
            thread::sleep(Duration::from_secs(timeout));

            // Obtain new clipboard context, get current contents
            let mut ctx = ClipboardContext::new()
                .expect(&format!("{}: failed to obtain X11 clipboard context", bin,));
            let now = ctx.get_contents().expect(&format!(
                "{}: failed to get clipboard contents through forked process",
                bin,
            ));

            // If clipboard contents didn't change, revert back to previous
            if data == now {
                ctx.set_contents(previous).expect(&format!(
                    "{}: failed to revert clipboard contents through forked process",
                    bin,
                ));

                // Update cleared state, show notification
                let _ = notify_cleared();
            }

            error::quit();
        }
        _pid => {}
    }

    if report {
        eprintln!(
            "Secret copied to clipboard. Clearing after {} seconds...",
            timeout
        );
    }

    Ok(())
}

/// Copy with timeout on macOS.
///
/// Keeps clipboard contents in clipboard even if application quits. Doesn't fuck with other
/// clipboard contents and reverts back to previous contents once a timeout is reached.
///
/// Spawns and disowns a process to manage reverting the clipboard after timeout.
#[cfg(target_os = "macos")]
fn copy_timeout_macos(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    copy_timeout_process(data, timeout, report)
}

/// Copy with timeout on Windows.
///
/// Keeps clipboard contents in clipboard even if application quits. Doesn't fuck with other
/// clipboard contents and reverts back to previous contents once a timeout is reached.
///
/// Spawns and disowns a process to manage reverting the clipboard after timeout.
#[cfg(target_os = "windows")]
fn copy_timeout_windows(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    copy_timeout_process(data, timeout, report)
}

/// Copy with timeout using subprocess.
///
/// Copy with timeout. Spawn and disown a process to manage reverting the clipboard contents.
///
/// Falls back to blocking method if it fails to determine the current executable path.
#[allow(unused)]
fn copy_timeout_process(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    // Find current exe path, or fall back to basic timeout copy
    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(_) => match std::env::args().next() {
            Some(bin) => bin.into(),
            None => return copy_timeout_blocking(data, timeout, report),
        },
    };

    // Set clipboard, remember previous contents
    let previous = get().unwrap_or_else(|_| "".into());
    set(data)?;

    // Spawn & disown background process to revert clipboard, send previous contents to it
    let process = Command::new(current_exe)
        .arg("internal")
        .arg("clip-revert")
        .arg("--previous-base64-stdin")
        .arg("--timeout")
        .arg(&format!("{}", timeout))
        .stdin(Stdio::piped())
        .spawn()
        .map_err(Err::Timeout)?;
    writeln!(
        process.stdin.unwrap(),
        "{}",
        base64::engine::general_purpose::STANDARD.encode(previous)
    )
    .map_err(Err::Timeout)?;

    if report {
        eprintln!(
            "Secret copied to clipboard. Clearing after {} seconds...",
            timeout
        );
    }

    Ok(())
}

/// Copy with timeout, blocking.
///
/// Simple fallback method blocking for timeout until cleared.
fn copy_timeout_blocking(data: &[u8], timeout: u64, report: bool) -> Result<()> {
    use copypasta_ext::copypasta::ClipboardContext;

    let mut ctx = ClipboardContext::new().map_err(Err::Clipboard)?;
    ctx.set_contents(std::str::from_utf8(data).unwrap().into())
        .map_err(Err::Clipboard)?;

    // TODO: clear clipboard on ctrl+c
    if report {
        eprintln!(
            "Secret copied to clipboard. Waiting {} seconds to clear...",
            timeout
        );
    }
    thread::sleep(Duration::from_secs(timeout));

    ctx.set_contents("".into()).map_err(Err::Clipboard)?;
    let _ = notify_cleared();

    Ok(())
}

/// Copy the given plain text to the user clipboard.
pub(crate) fn plaintext_copy(
    mut plaintext: Plaintext,
    first_line: bool,
    error_empty: bool,
    report: bool,
    timeout: u64,
) -> Result<()> {
    if first_line {
        plaintext = plaintext.first_line()?;
    }

    // Do not copy empty secret
    if error_empty && plaintext.is_empty() {
        error::quit_error_msg(
            "secret is empty, did not copy to clipboard",
            ErrorHintsBuilder::default().force(true).build().unwrap(),
        )
    }

    copy_timeout(plaintext.unsecure_ref(), timeout, report).map_err(Err::CopySecret)?;

    Ok(())
}

/// Show notification to user about cleared clipboard.
pub(crate) fn notify_cleared() -> Result<()> {
    // Do not show notification with not notify or on musl due to segfault
    #[cfg(all(feature = "notify", not(target_env = "musl")))]
    {
        let mut n = Notification::new();
        n.appname(&crate::util::bin_name())
            .summary(&format!("Clipboard cleared - {}", crate::util::bin_name()))
            .body("Secret cleared from clipboard")
            .auto_icon()
            .icon("lock")
            .timeout(3000);

        #[cfg(target_os = "linux")]
        n.urgency(notify_rust::Urgency::Low)
            .hint(Hint::Category("presence.offline".into()));

        n.show()?;
        return Ok(());
    }

    // Fallback if we cannot notify
    #[allow(unreachable_code)]
    {
        eprintln!("Secret cleared from clipboard");
        Ok(())
    }
}

/// Check if running on Wayland.
///
/// This checks at runtime whether the user is running the Wayland display server. This is a best
/// effort, and may not be reliable.
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
))]
fn is_wayland() -> bool {
    DisplayServer::select() == DisplayServer::Wayland
}

#[derive(Debug, Error)]
pub enum Err {
    #[cfg(not(windows))]
    #[error("failed to parse clipboard contents as UTF-8")]
    Utf8(#[source] std::str::Utf8Error),

    #[error("failed to set clipboard")]
    Clipboard(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("failed to copy secret to clipboard")]
    CopySecret(#[source] anyhow::Error),

    #[error("failed to set-up clipboard clearing timeout")]
    Timeout(#[source] std::io::Error),
}
