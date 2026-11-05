//! Key injection back-ends.
//!
//! The keyboard logic only knows the [`Injector`] trait. On Linux the real
//! back-end creates a virtual keyboard through `/dev/uinput`, which works on
//! X11 and Wayland alike because the compositor sees an ordinary keyboard.

use std::io::{self, Write};

pub trait Injector {
    /// Press (`down == true`) or release one key. `code` is a Linux input-event code.
    fn key(&mut self, code: u16, down: bool);

    /// Short description shown in the title bar.
    fn name(&self) -> String;

    /// `false` when keys are only printed instead of delivered.
    fn is_live(&self) -> bool {
        true
    }
}

/// Prints key events instead of sending them. Useful for working on the UI,
/// and as the fallback when no virtual keyboard can be created.
pub struct DryRun;

impl Injector for DryRun {
    fn key(&mut self, code: u16, down: bool) {
        // `writeln!` (not `println!`) and errors ignored on purpose: if stdout
        // is a pipe whose reader has gone away, printing must not kill the app.
        let mut out = io::stdout();
        let _ = writeln!(out, "key {code} {}", if down { "down" } else { "up" });
    }

    fn name(&self) -> String {
        "Preview mode: keys are only printed".to_string()
    }

    fn is_live(&self) -> bool {
        false
    }
}

/// Picks the best back-end available on this machine.
pub fn best_available() -> Box<dyn Injector> {
    #[cfg(target_os = "linux")]
    {
        match uinput::UInput::new() {
            Ok(device) => return Box::new(device),
            Err(err) => eprintln!(
                "osk: cannot create a virtual keyboard via /dev/uinput ({err}); \
                 falling back to preview mode. See README.md, section \"Typing permission\"."
            ),
        }
    }
    Box::new(DryRun)
}

#[cfg(target_os = "linux")]
pub mod uinput {
    use super::Injector;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Write};
    use std::os::unix::io::AsRawFd;
    use std::time::Duration;

    // ioctl request numbers from <linux/uinput.h>.
    const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
    const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
    const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
    const UI_DEV_CREATE: libc::c_ulong = 0x5501;
    const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

    const EV_SYN: u16 = 0x00;
    const EV_KEY: u16 = 0x01;
    const SYN_REPORT: u16 = 0;
    const BUS_USB: u16 = 0x03;

    #[repr(C)]
    #[allow(dead_code)]
    struct InputId {
        bustype: u16,
        vendor: u16,
        product: u16,
        version: u16,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct UinputSetup {
        id: InputId,
        name: [u8; 80],
        ff_effects_max: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    struct InputEvent {
        time: libc::timeval,
        kind: u16,
        code: u16,
        value: i32,
    }

    impl InputEvent {
        fn new(kind: u16, code: u16, value: i32) -> InputEvent {
            // SAFETY: `InputEvent` is plain old data and the kernel stamps
            // events with its own clock, so an all-zero `time` is a valid,
            // deterministic starting point (it also avoids sending whatever
            // happened to be in that stack memory).
            let mut event: InputEvent = unsafe { std::mem::zeroed() };
            event.kind = kind;
            event.code = code;
            event.value = value;
            event
        }
    }

    fn check(ret: libc::c_int) -> io::Result<()> {
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// A virtual keyboard registered with the kernel.
    pub struct UInput {
        file: File,
        /// Codes still held down, so `Drop` can release them: a modifier
        /// stuck in the OS would corrupt every later keystroke.
        held: Vec<u16>,
        /// Cleared once a write has failed; the device is dead, so stop
        /// using it instead of spamming errors and half-events.
        healthy: bool,
    }

    impl UInput {
        pub fn new() -> io::Result<UInput> {
            // Opened in blocking mode on purpose: `write_all` then retries
            // short writes and waits out a temporarily full kernel queue.
            // (The old code opened /dev/uinput with O_NONBLOCK, so a full
            // queue made writes fail with EAGAIN and key events were
            // silently dropped - a lost key-up leaves that key stuck down
            // in the OS, auto-repeating forever.)
            let file = OpenOptions::new().write(true).open("/dev/uinput")?;
            let fd = file.as_raw_fd();

            let mut setup = UinputSetup {
                id: InputId {
                    bustype: BUS_USB,
                    vendor: 0x1209,
                    product: 0x0517,
                    version: 1,
                },
                name: [0u8; 80],
                ff_effects_max: 0,
            };
            let name = b"OSK Virtual Keyboard";
            setup.name[..name.len()].copy_from_slice(name);

            // SAFETY: plain ioctls on a file descriptor we own; `setup` outlives the call.
            unsafe {
                check(libc::ioctl(fd, UI_SET_EVBIT as _, EV_KEY as libc::c_int))?;
                for code in 1..=255 {
                    check(libc::ioctl(fd, UI_SET_KEYBIT as _, code as libc::c_int))?;
                }
                check(libc::ioctl(
                    fd,
                    UI_DEV_SETUP as _,
                    &setup as *const UinputSetup,
                ))?;
                check(libc::ioctl(fd, UI_DEV_CREATE as _))?;
            }

            // Give the compositor a moment to notice the new device.
            std::thread::sleep(Duration::from_millis(250));
            Ok(UInput {
                file,
                held: Vec::new(),
                healthy: true,
            })
        }

        /// Sends one key event and its sync report in a single `write_all`:
        /// the pair can never be split by a short write, and a blocking
        /// write to a temporarily full queue waits instead of failing.
        fn emit(&mut self, code: u16, value: i32) -> io::Result<()> {
            let events = [
                InputEvent::new(EV_KEY, code, value),
                InputEvent::new(EV_SYN, SYN_REPORT, 0),
            ];
            // SAFETY: `InputEvent` is #[repr(C)] plain old data.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (&events as *const [InputEvent; 2]) as *const u8,
                    2 * std::mem::size_of::<InputEvent>(),
                )
            };
            self.file.write_all(bytes)
        }

        /// Releases every key that is still down; called from `Drop` so the
        /// OS never inherits a stuck key when the window closes or panics.
        fn release_all(&mut self) {
            if !self.healthy {
                return;
            }
            for code in std::mem::take(&mut self.held) {
                if let Err(err) = self.emit(code, 0) {
                    eprintln!("osk: failed to release key {code}: {err}");
                    self.healthy = false;
                    break;
                }
            }
        }
    }

    impl Injector for UInput {
        fn key(&mut self, code: u16, down: bool) {
            if !self.healthy {
                return;
            }
            let value = if down { 1 } else { 0 };
            if let Err(err) = self.emit(code, value) {
                eprintln!("osk: failed to send key {code}: {err}");
                // The event stream is broken; stop using the device.
                self.healthy = false;
                return;
            }
            if down {
                self.held.push(code);
            } else if let Some(pos) = self.held.iter().position(|&held| held == code) {
                self.held.remove(pos);
            }
        }

        fn name(&self) -> String {
            "Typing via uinput".to_string()
        }
    }

    impl Drop for UInput {
        fn drop(&mut self) {
            self.release_all();
            // SAFETY: destroys the device created in `new`.
            unsafe {
                libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY as _);
            }
        }
    }
}
