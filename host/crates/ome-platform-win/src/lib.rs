// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Owned platform operations for Windows, with raw Win32 values kept private.
#![deny(unsafe_code)]
#![deny(clippy::undocumented_unsafe_blocks)]

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

#[cfg(windows)]
mod ffi;

/// A platform operation that cannot be represented as a plain I/O error.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// The operation has no implementation on the current host OS.
    #[error("operation is unsupported on this platform")]
    Unsupported,
    /// The user cancelled the Windows elevation prompt.
    #[error("user declined elevation")]
    UserDeclinedElevation,
    /// The declared interface is intentionally not wired in this milestone.
    #[error("operation is not wired: {0}")]
    Unwired(&'static str),
    /// A Win32 or standard I/O operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Result of waiting for an owned kernel object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitOutcome {
    /// The object became signaled.
    Signaled,
    /// The timeout elapsed first.
    TimedOut,
    /// A mutex was abandoned; waiting still acquired it.
    Abandoned,
}

/// An owned Windows kernel handle, closed exactly once on drop.
///
/// `Send` and `Sync` are sound because Windows kernel handles are process-wide
/// tokens and the kernel serializes operations on the referenced object.
#[cfg(windows)]
#[derive(Debug)]
pub struct OwnedHandle(ffi::handle::OwnedHandle);

/// Placeholder on non-Windows hosts; it cannot be constructed successfully.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct OwnedHandle;

impl OwnedHandle {
    /// Waits until the object is signaled or `timeout` elapses; `None` waits forever.
    pub fn wait(&self, timeout: Option<Duration>) -> Result<WaitOutcome, PlatformError> {
        #[cfg(windows)]
        {
            self.0.wait(timeout).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err(PlatformError::Unsupported)
        }
    }
}

/// A process job configured to terminate all members when its last handle closes.
///
/// No active-process limit is applied because QEMU may create helper processes.
#[cfg(windows)]
#[derive(Debug)]
pub struct JobObject(ffi::job::JobObject);

/// Placeholder on non-Windows hosts.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct JobObject;

impl JobObject {
    /// Creates an anonymous kill-on-close job without an active-process limit.
    pub fn kill_on_close() -> io::Result<Self> {
        #[cfg(windows)]
        {
            ffi::job::JobObject::kill_on_close().map(Self)
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                PlatformError::Unsupported,
            ))
        }
    }

    /// Assigns `child` to this job; both values must still own live handles.
    pub fn assign(&self, child: &Child) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.0.assign(&child.inner)
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                PlatformError::Unsupported,
            ))
        }
    }
}

/// Inputs for a Windows process launch with explicit standard-handle inheritance.
#[derive(Debug)]
pub struct ProcessLaunch {
    /// Executable passed directly to `CreateProcessW`.
    pub executable: PathBuf,
    /// Discrete arguments quoted according to `CommandLineToArgvW` rules.
    pub arguments: Vec<OsString>,
    /// Child standard-output file; ownership moves into the launch.
    pub stdout: File,
    /// Child standard-error file; ownership moves into the launch.
    pub stderr: File,
    /// Optional child working directory.
    pub cwd: Option<PathBuf>,
    /// Variables added to or overriding the current environment for the child.
    pub environment: Vec<(OsString, OsString)>,
}

impl ProcessLaunch {
    /// Starts the process with stdin attached to `NUL` and only stdin, stdout,
    /// and stderr in the explicit inherited-handle list.
    pub fn spawn(self) -> Result<Child, PlatformError> {
        #[cfg(windows)]
        {
            ffi::process::spawn(self).map(|inner| Child { inner })
        }
        #[cfg(not(windows))]
        {
            let _ = self;
            Err(PlatformError::Unsupported)
        }
    }

    /// Creates the process suspended, assigns it to `job`, then resumes its
    /// initial thread so no helper can escape the job before assignment.
    pub fn spawn_in_job(self, job: &JobObject) -> Result<Child, PlatformError> {
        #[cfg(windows)]
        {
            ffi::process::spawn_in_job(self, &job.0).map(|inner| Child { inner })
        }
        #[cfg(not(windows))]
        {
            let _ = (self, job);
            Err(PlatformError::Unsupported)
        }
    }
}

/// A spawned process represented by its owned process handle and immutable PID.
#[cfg(windows)]
#[derive(Debug)]
pub struct Child {
    inner: ffi::process::Child,
}

/// Placeholder child on non-Windows hosts.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct Child;

impl Child {
    /// Returns the PID captured from `CreateProcessW`.
    pub fn pid(&self) -> u32 {
        #[cfg(windows)]
        {
            self.inner.pid()
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// Returns `Some(exit_code)` after process exit, or `None` while it runs.
    pub fn try_wait(&self) -> io::Result<Option<u32>> {
        #[cfg(windows)]
        {
            self.inner.try_wait()
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                PlatformError::Unsupported,
            ))
        }
    }

    /// Waits for process exit, returning [`WaitOutcome::TimedOut`] at the deadline.
    pub fn wait(&self, timeout: Option<Duration>) -> io::Result<WaitOutcome> {
        #[cfg(windows)]
        {
            self.inner.wait(timeout)
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                PlatformError::Unsupported,
            ))
        }
    }

    /// Terminates the process with exit code 1.
    pub fn terminate(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.inner.terminate()
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                PlatformError::Unsupported,
            ))
        }
    }
}

/// One native client rectangle in physical screen pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenRect {
    /// Horizontal screen coordinate.
    pub x: i32,
    /// Vertical screen coordinate.
    pub y: i32,
    /// Physical width.
    pub width: u32,
    /// Physical height.
    pub height: u32,
}

/// Opaque numeric window token used to bridge differing `windows` crate versions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowHandle(isize);

impl WindowHandle {
    /// Wraps an HWND numeric value supplied by Tauri without taking ownership.
    ///
    /// The caller must ensure the value denotes a live window whenever a method
    /// is called. Windows owns HWND lifetime; this wrapper never destroys it.
    pub fn from_raw(value: isize) -> Self {
        Self(value)
    }

    /// Returns the opaque numeric token used by the window-hosting contract.
    pub fn as_u64(self) -> u64 {
        self.0 as usize as u64
    }

    /// Wraps the numeric token used by the window-hosting contract.
    pub fn from_u64(value: u64) -> Result<Self, PlatformError> {
        usize::try_from(value)
            .map(|value| Self(value as isize))
            .map_err(|_| {
                PlatformError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "window token does not fit the host pointer width",
                ))
            })
    }

    /// Returns the window's registered class name.
    pub fn class_name(self) -> Result<String, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::class_name(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the window's title text, empty when it has none.
    pub fn title(self) -> Result<String, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::title(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Reports whether the window is visible.
    pub fn is_visible(self) -> Result<bool, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::is_visible(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Reports whether this token still denotes a window owned by `pid`.
    ///
    /// The PID check guards against HWND reuse after the original window exits.
    pub fn belongs_to_process(self, pid: u32) -> bool {
        #[cfg(windows)]
        {
            ffi::window::belongs_to_process(self.0, pid)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            false
        }
    }

    /// Hides this window without changing focus or activation.
    pub fn hide(self) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::hide(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Shows this window without activating it.
    pub fn show_inactive(self) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::show_inactive(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Reports whether this window owns keyboard focus in its GUI thread queue.
    pub fn has_keyboard_focus(self) -> Result<bool, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::has_keyboard_focus(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the focused window in this window's GUI thread queue.
    pub fn thread_focus(self) -> Result<Option<Self>, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::thread_focus(self.0)
                .map(|window| window.map(Self))
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Converts this top-level window to `WS_CHILD`, reparents it, and refreshes
    /// its non-client frame. The returned value must be retained for restoration.
    pub fn make_child_of(self, parent: WindowHandle) -> Result<PreviousStyle, PlatformError> {
        self.make_child_of_at(parent, 0, 0, 0, 0)
    }

    /// Converts and reparents this window while applying its first child bounds
    /// in one recoverable platform operation.
    pub fn make_child_of_at(
        self,
        parent: WindowHandle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<PreviousStyle, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::make_child_of_at(self.0, parent.0, x, y, width, height)
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (parent, x, y, width, height);
            Err(PlatformError::Unsupported)
        }
    }

    /// Converts this window to a borderless, non-activating top-level popup owned by `owner`.
    ///
    /// Coordinates and dimensions are physical screen pixels. Style, extended style, and owner
    /// changes are rolled back together if any native operation fails.
    pub fn make_owned_popup(
        self,
        owner: WindowHandle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<PreviousStyle, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::make_owned_popup(self.0, owner.0, x, y, width, height)
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (owner, x, y, width, height);
            Err(PlatformError::Unsupported)
        }
    }

    /// Restores the saved style, extended style, and original window relationship.
    pub fn restore_top_level(self, previous: PreviousStyle) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::restore_top_level(self.0, previous).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = previous;
            Err(PlatformError::Unsupported)
        }
    }

    /// Moves and resizes the window in parent-client physical pixels.
    pub fn set_bounds(self, x: i32, y: i32, width: i32, height: i32) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::set_bounds(self.0, x, y, width, height).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (x, y, width, height);
            Err(PlatformError::Unsupported)
        }
    }

    /// Places this top-level window directly behind `insert_after`, or at the top when omitted.
    ///
    /// Coordinates and dimensions are physical screen pixels. The Z-order change is intentional;
    /// the window is never activated.
    pub fn place_behind(
        self,
        insert_after: Option<WindowHandle>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::place_behind(
                self.0,
                insert_after.map(|window| window.0),
                x,
                y,
                width,
                height,
            )
            .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (insert_after, x, y, width, height);
            Err(PlatformError::Unsupported)
        }
    }

    /// Changes only the Z-order of this top-level window: directly behind `insert_after`, or at
    /// the top when omitted.
    ///
    /// Position, size and activation stay untouched (`SWP_NOMOVE | SWP_NOSIZE |
    /// SWP_NOACTIVATE`). Moving a window and changing its Z-order are the host changes that do not
    /// freeze an SDL GL window (`docs/evidence/M2/embedded-display-freeze.md`).
    pub fn place_z_order(self, insert_after: Option<WindowHandle>) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::place_z_order(self.0, insert_after.map(|window| window.0))
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = insert_after;
            Err(PlatformError::Unsupported)
        }
    }

    /// Activates `parent` on its owner thread without leaving focus on this hosted child.
    pub fn focus_as_child_of(self, parent: WindowHandle) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::focus_child(self.0, parent.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = parent;
            Err(PlatformError::Unsupported)
        }
    }

    /// Requests foreground activation for a separate top-level window.
    pub fn to_foreground(self) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::to_foreground(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the window rectangle in physical screen pixels.
    pub fn window_rect(self) -> Result<ScreenRect, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::window_rect(self.0)
                .map(|(x, y, width, height)| ScreenRect {
                    x,
                    y,
                    width,
                    height,
                })
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the owner of this top-level window, if one is registered.
    pub fn owner(self) -> Option<Self> {
        #[cfg(windows)]
        {
            ffi::window::owner(self.0).map(Self)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Reports whether the window has no child-parent relationship.
    ///
    /// Owned popups remain top-level windows even though Win32 `GetParent` reports their owner.
    pub fn is_top_level(self) -> bool {
        #[cfg(windows)]
        {
            ffi::window::is_top_level(self.0)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Reports whether `WS_EX_NOACTIVATE` is present in the extended style.
    pub fn has_no_activate_style(self) -> Result<bool, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::has_no_activate_style(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the physical client size of the window.
    pub fn client_size(self) -> Result<(i32, i32), PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::client_size(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the client rectangle in physical screen pixels.
    pub fn client_screen_rect(self) -> Result<ScreenRect, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::client_screen_rect(self.0)
                .map(|(x, y, width, height)| ScreenRect {
                    x,
                    y,
                    width,
                    height,
                })
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the window's effective DPI; zero from Win32 is reported as an error.
    pub fn dpi(self) -> Result<u32, PlatformError> {
        #[cfg(windows)]
        {
            ffi::window::dpi(self.0).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }
}

/// Style, extended style, and window relationship saved before native hosting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviousStyle {
    style: isize,
    ex_style: isize,
    parent: Option<isize>,
    owner: Option<isize>,
    hosting: PreviousHosting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviousHosting {
    Child,
    OwnedPopup,
}

/// Returns the current Windows foreground-window token, or zero when there is none.
pub fn foreground_window() -> u64 {
    #[cfg(windows)]
    {
        ffi::window::foreground_window() as usize as u64
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Returns the window under one physical screen coordinate, or zero if none exists.
pub fn window_at(x: i32, y: i32) -> u64 {
    #[cfg(windows)]
    {
        ffi::mouse::window_at(x, y) as usize as u64
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y);
        0
    }
}

/// Moves the host cursor and injects one primary-button click.
///
/// This helper exists for native integration checks that verify embedded guest pointer input.
pub fn click_primary_at(x: i32, y: i32) -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        ffi::mouse::click_primary_at(x, y).map_err(PlatformError::Io)
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y);
        Err(PlatformError::Unsupported)
    }
}

/// Returns the share of lit pixels in one physical screen region, from 0.0 to 1.0.
///
/// This helper exists for native integration checks that tell a live guest window from a frozen
/// one. It copies the composed screen with GDI, samples every 12th pixel on each axis and counts a
/// pixel as lit when its red, green and blue values sum above 60. A sleeping monitor stops DWM
/// composition, so the copy then shows the last composed frame.
pub fn screen_region_lit_share(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<f64, PlatformError> {
    #[cfg(windows)]
    {
        ffi::screen::lit_share(x, y, width, height).map_err(PlatformError::Io)
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y, width, height);
        Err(PlatformError::Unsupported)
    }
}

/// Keeps the display on until dropped.
///
/// The user is watching a game, so while a guest runs the emulator keeps the display on the way
/// video players do: `SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED)`. The
/// system-sleep flag (`ES_SYSTEM_REQUIRED`) is not held. The state belongs to the calling thread,
/// so the guard holds it on its own long-lived thread, which clears it (`ES_CONTINUOUS` alone)
/// when the guard drops. A sleeping monitor also stops DWM composition, which leaves the guest
/// window showing stale frames (`docs/evidence/M2/embedded-display-freeze.md`).
#[derive(Debug)]
pub struct DisplayKeepAwake {
    release: Option<std::sync::mpsc::Sender<()>>,
    holder: Option<std::thread::JoinHandle<()>>,
}

impl DisplayKeepAwake {
    /// Starts the holder thread and returns once it holds the display requirement.
    pub fn acquire() -> Result<Self, PlatformError> {
        #[cfg(windows)]
        {
            let (release, released) = std::sync::mpsc::channel::<()>();
            let (ready, acquired) = std::sync::mpsc::channel::<io::Result<()>>();
            let holder = std::thread::Builder::new()
                .name("ome-display-keep-awake".to_owned())
                .spawn(move || {
                    let result = ffi::power::require_display();
                    let held = result.is_ok();
                    let _ = ready.send(result);
                    if held {
                        // Returns when the guard drops its sender.
                        let _ = released.recv();
                        ffi::power::release_display();
                    }
                })
                .map_err(PlatformError::Io)?;
            match acquired.recv() {
                Ok(Ok(())) => Ok(Self {
                    release: Some(release),
                    holder: Some(holder),
                }),
                Ok(Err(error)) => {
                    let _ = holder.join();
                    Err(PlatformError::Io(error))
                }
                Err(_) => {
                    let _ = holder.join();
                    Err(PlatformError::Io(io::Error::other(
                        "display keep-awake thread ended before reporting",
                    )))
                }
            }
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }
}

impl Drop for DisplayKeepAwake {
    fn drop(&mut self) {
        drop(self.release.take());
        if let Some(holder) = self.holder.take() {
            let _ = holder.join();
        }
    }
}

/// Enumerates all top-level windows currently owned by `pid`.
pub fn find_windows_of_process(pid: u32) -> Result<Vec<WindowHandle>, PlatformError> {
    #[cfg(windows)]
    {
        ffi::window::find_windows_of_process(pid)
            .map(|items| items.into_iter().map(WindowHandle).collect())
            .map_err(PlatformError::Io)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        Err(PlatformError::Unsupported)
    }
}

/// Makes this process per-monitor DPI aware (v2), so window and screen coordinates are physical.
///
/// The Tauri shell's event loop does the same (`tao`), and QEMU runs with
/// `SDL_WINDOWS_DPI_AWARENESS=permonitorv2`; the QMP window geometry is in physical pixels.
/// Native integration checks call this before creating any window so that they see the same
/// coordinates as the product. Windows rejects the call once the process default is set.
pub fn set_process_dpi_awareness_per_monitor_v2() -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        ffi::window::set_process_dpi_awareness_per_monitor_v2().map_err(PlatformError::Io)
    }
    #[cfg(not(windows))]
    {
        Err(PlatformError::Unsupported)
    }
}

/// Guard that restores the thread's previous DPI-hosting behavior on drop.
#[cfg(windows)]
#[derive(Debug)]
pub struct DpiHostingGuard {
    _guard: ffi::window::DpiHostingGuard,
}

/// Placeholder DPI guard on non-Windows hosts.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct DpiHostingGuard;

/// Switches the current thread to mixed DPI hosting until the returned guard drops.
///
/// Call this on the thread that will create the parent window, before creating
/// that window. Keep the returned guard alive for at least as long as the parent
/// window so child hosting continues under the selected behavior.
pub fn set_thread_dpi_hosting_mixed() -> Result<DpiHostingGuard, PlatformError> {
    #[cfg(windows)]
    {
        ffi::window::set_thread_dpi_hosting_mixed()
            .map(|guard| DpiHostingGuard { _guard: guard })
            .map_err(PlatformError::Io)
    }
    #[cfg(not(windows))]
    {
        Err(PlatformError::Unsupported)
    }
}

/// A plain top-level window whose creating thread owns its message loop.
///
/// This type exists for native integration tests; it creates no webview and
/// destroys the window on its owner thread when dropped.
#[cfg(windows)]
#[derive(Debug)]
pub struct TestHostWindow(ffi::test_window::TestHostWindow);

/// Placeholder test host window on non-Windows systems.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct TestHostWindow;

impl TestHostWindow {
    /// Creates and shows a top-level window with the requested client size.
    pub fn create(title: &str, width: i32, height: i32) -> Result<Self, PlatformError> {
        #[cfg(windows)]
        {
            ffi::test_window::TestHostWindow::create(title, width, height)
                .map(Self)
                .map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (title, width, height);
            Err(PlatformError::Unsupported)
        }
    }

    /// Returns the opaque token for the live test window.
    pub fn handle(&self) -> WindowHandle {
        #[cfg(windows)]
        {
            WindowHandle(self.0.raw())
        }
        #[cfg(not(windows))]
        {
            WindowHandle(0)
        }
    }

    /// Returns the current physical client size.
    pub fn client_size(&self) -> Result<(i32, i32), PlatformError> {
        #[cfg(windows)]
        {
            self.0.client_size().map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported)
        }
    }

    /// Places the native test window in or out of the topmost Z-order band.
    pub fn set_topmost(&self, enabled: bool) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            self.0.set_topmost(enabled).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = enabled;
            Err(PlatformError::Unsupported)
        }
    }

    /// Resizes the window so its client area has the requested physical size.
    pub fn resize(&self, width: i32, height: i32) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            self.0.resize(width, height).map_err(PlatformError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (width, height);
            Err(PlatformError::Unsupported)
        }
    }
}

/// Process launched through Windows elevation with an owned process handle.
#[cfg(windows)]
#[derive(Debug)]
pub struct ElevatedChild(ffi::elevation::ElevatedChild);

/// Placeholder elevated child on non-Windows hosts.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct ElevatedChild;

impl ElevatedChild {
    /// Waits up to `timeout` and returns the process exit code.
    ///
    /// A timeout is reported as `io::ErrorKind::TimedOut`.
    pub fn wait(&self, timeout: Duration) -> Result<u32, PlatformError> {
        #[cfg(windows)]
        {
            self.0.wait(timeout)
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err(PlatformError::Unsupported)
        }
    }
}

/// Starts `executable` with the `runas` verb and retains the shell's process handle.
pub fn launch_elevated(
    executable: &Path,
    parameters: &OsStr,
) -> Result<ElevatedChild, PlatformError> {
    #[cfg(windows)]
    {
        ffi::elevation::launch_elevated(executable, parameters).map(ElevatedChild)
    }
    #[cfg(not(windows))]
    {
        let _ = (executable, parameters);
        Err(PlatformError::Unsupported)
    }
}

/// Reports whether Windows exposes firmware-enabled hardware virtualization.
#[cfg(windows)]
pub fn virtualization_firmware_enabled() -> bool {
    ffi::host::virtualization_firmware_enabled()
}

/// Reports whether x86_64 CPUID says a hypervisor is present.
#[cfg(windows)]
pub fn hypervisor_present() -> bool {
    ffi::host::hypervisor_present()
}

/// Reports whether either Windows servicing stack records a pending reboot.
#[cfg(windows)]
pub fn reboot_pending() -> io::Result<bool> {
    ffi::host::reboot_pending()
}

/// Reports free bytes available to the caller on the volume containing `path`.
///
/// When `path` does not exist yet, the nearest existing directory ancestor is used.
#[cfg(windows)]
pub fn free_disk_bytes(path: &Path) -> io::Result<u64> {
    ffi::host::free_disk_bytes(path)
}

/// Reports total physical memory in bytes.
#[cfg(windows)]
pub fn total_memory_bytes() -> io::Result<u64> {
    ffi::host::total_memory_bytes()
}

/// Reports active logical processors across every Windows processor group.
#[cfg(windows)]
pub fn logical_processors() -> u32 {
    ffi::host::logical_processors()
}

/// Reports how many waveform-audio output devices Windows exposes right now.
///
/// Zero means DirectSound cannot open, so the guest must run without an audio backend.
#[cfg(windows)]
pub fn audio_output_devices() -> u32 {
    ffi::audio::output_devices()
}

/// Reports whether the Windows Hypervisor Platform is present.
///
/// A missing `WinHvPlatform.dll` is a normal `Ok(false)` result.
pub fn whpx_available() -> Result<bool, PlatformError> {
    #[cfg(windows)]
    {
        ffi::hypervisor::whpx_available().map_err(PlatformError::Io)
    }
    #[cfg(not(windows))]
    {
        Err(PlatformError::Unsupported)
    }
}

/// Quotes one argument according to the parsing rules used by `CommandLineToArgvW`.
///
/// The returned string is suitable for concatenation into `CreateProcessW`'s
/// mutable command-line buffer. No shell expansion occurs.
pub fn quote_windows_argument(value: &OsStr) -> String {
    let value = value.to_string_lossy();
    if !value.is_empty()
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return value.into_owned();
    }

    let mut quoted = String::from('"');
    let mut backslashes = 0_usize;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quoting_table_matches_launcher_rules() {
        let cases = [
            ("", "\"\""),
            ("plain", "plain"),
            ("two words", "\"two words\""),
            ("a\\b", "a\\b"),
            ("a\"b", "\"a\\\"b\""),
            ("a b\\", "\"a b\\\\\""),
            ("a\\\"b", "\"a\\\\\\\"b\""),
        ];
        for (input, expected) in cases {
            assert_eq!(
                quote_windows_argument(OsStr::new(input)),
                expected,
                "{input:?}"
            );
        }
    }

    #[cfg(windows)]
    fn cmd_echo(variable: &str, environment: Vec<(OsString, OsString)>) -> String {
        let directory = std::env::temp_dir().join(format!(
            "ome-platform-win-env-{}-{}-{}",
            variable,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create environment test directory");
        let stdout_path = directory.join("stdout.log");
        let stderr_path = directory.join("stderr.log");
        let launch = ProcessLaunch {
            executable: PathBuf::from(r"C:\Windows\System32\cmd.exe"),
            arguments: vec![
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from(format!("echo %{variable}%")),
            ],
            stdout: File::create(&stdout_path).expect("create stdout"),
            stderr: File::create(&stderr_path).expect("create stderr"),
            cwd: None,
            environment,
        };
        let child = launch.spawn().expect("spawn cmd");
        assert_eq!(
            child.wait(Some(Duration::from_secs(5))).expect("wait"),
            WaitOutcome::Signaled
        );
        assert_eq!(child.try_wait().expect("exit code"), Some(0));
        let output = std::fs::read_to_string(&stdout_path).expect("read stdout");
        std::fs::remove_dir_all(directory).expect("remove environment test directory");
        output
    }

    #[cfg(windows)]
    #[test]
    fn process_launch_adds_environment_variable() {
        let output = cmd_echo(
            "OME_Y1_TEST",
            vec![(OsString::from("OME_Y1_TEST"), OsString::from("value"))],
        );
        assert_eq!(output.trim(), "value");
    }

    #[cfg(windows)]
    #[test]
    fn process_launch_empty_environment_list_inherits_path() {
        let output = cmd_echo("PATH", Vec::new());
        assert!(!output.trim().is_empty());
        assert_ne!(output.trim(), "%PATH%");
    }

    #[cfg(windows)]
    #[test]
    fn process_launch_job_and_exit_code_work_together() {
        use std::io::{Read, Seek, SeekFrom};

        let directory = std::env::temp_dir().join(format!(
            "ome-platform-win-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create test directory");
        let stdout_path = directory.join("stdout.log");
        let stderr_path = directory.join("stderr.log");
        let stdout = File::create(&stdout_path).expect("create stdout");
        let stderr = File::create(&stderr_path).expect("create stderr");
        let launch = ProcessLaunch {
            executable: PathBuf::from(r"C:\Windows\System32\cmd.exe"),
            arguments: vec![
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("echo out& echo err 1>&2& exit 3"),
            ],
            stdout,
            stderr,
            cwd: None,
            environment: Vec::new(),
        };
        let job = JobObject::kill_on_close().expect("create job");
        let child = launch.spawn_in_job(&job).expect("spawn cmd in job");
        assert_eq!(
            child.wait(Some(Duration::from_secs(5))).expect("wait"),
            WaitOutcome::Signaled
        );
        assert_eq!(child.try_wait().expect("exit code"), Some(3));

        let mut output = String::new();
        let mut file = File::open(&stdout_path).expect("open stdout");
        file.seek(SeekFrom::Start(0)).expect("seek stdout");
        file.read_to_string(&mut output).expect("read stdout");
        assert!(output.contains("out"));
        output.clear();
        File::open(&stderr_path)
            .expect("open stderr")
            .read_to_string(&mut output)
            .expect("read stderr");
        assert!(output.contains("err"));
        std::fs::remove_dir_all(directory).expect("remove test directory");
    }
}
