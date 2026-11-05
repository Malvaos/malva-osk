# OSK

A macOS-styled on-screen keyboard written in Rust with [Slint](https://slint.dev).

- Full US QWERTY with function row, number row and an inverted-T arrow cluster
- Sticky modifiers: tap once for the next key only, tap twice to lock, tap again to release
- `caps` locks Shift in software; `fn` turns the arrows into Home, End, Page Up and Page Down
- Hold backspace, delete, space or an arrow key to repeat
- Light and dark themes, resizable, draggable by the title bar
- Types by creating a virtual keyboard through `/dev/uinput`, so it works on X11 and Wayland

Keys are sent as physical key codes, like a hardware keyboard. The labels assume your
system layout is US QWERTY.

## Build and run

Debian / Ubuntu:

```sh
sudo apt install build-essential pkg-config libfontconfig1-dev libxkbcommon-dev
cargo run --release
```

Fedora: `sudo dnf install gcc pkgconf-pkg-config fontconfig-devel libxkbcommon-devel`
Arch: `sudo pacman -S base-devel fontconfig libxkbcommon`

Options:

```
--light        start with the light theme
--dry-run      do not create a virtual keyboard, just print key events
-V, --version  print the version and exit
```

`cargo test` runs the keyboard-logic tests.

Without a local toolchain, push the project to GitHub. The workflow in
`.github/workflows/build.yml` builds it and attaches a Linux binary to the run.

## Typing permission (Linux)

Creating a virtual keyboard needs write access to `/dev/uinput`. Until you grant it, OSK
starts in preview mode (orange dot in the title bar) and only prints key events.

```sh
sudo modprobe uinput
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
sudo cp packaging/60-osk-uinput.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG input "$USER"      # then log out and back in
```

When it works, the dot is green and the title bar says "Typing via uinput".

## Focus

A virtual keyboard types into whichever window has focus. Slint opens OSK as a normal
window, and on most desktops clicking it moves focus to OSK itself, so keys would land
in the keyboard. It works well on kiosk and tablet setups, or when your compositor can
mark the window (title `OSK`) as always-on-top and never-focus. A layer-shell or
X11 dock-type back-end would be the next step for a regular desktop.

## Customising

| To change           | Edit                                                       |
| ------------------- | ---------------------------------------------------------- |
| Keys and their size | `us_qwerty()` in `src/layout.rs`                           |
| Colours and font    | the `Theme` global at the top of `ui/app.slint`            |
| Key shapes, motion  | `KeyCap` in `ui/app.slint`                                 |
| Modifier behaviour  | `src/engine.rs`                                            |
| Windows / macOS     | implement `Injector` in `src/inject.rs`                    |

## Project layout

```
build.rs          compiles ui/app.slint
ui/app.slint      every pixel: theme, key caps, title bar, grid
src/main.rs       wires the window to the engine
src/engine.rs     sticky modifiers, labels, typing (unit-tested)
src/layout.rs     the keyboard as data
src/inject.rs     uinput back-end and preview mode
src/keys.rs       Linux key codes
```

## Known limits

- Slint tracks one pointer, so two-finger shortcuts are not possible; use the sticky modifiers.
- The window cannot be moved by dragging on compositors that forbid client-side positioning.
- A tap on `super` latches it like the other modifiers, so it cannot open a launcher on its own yet.
