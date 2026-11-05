//! The keyboard layout as plain data (no UI types).
//!
//! Every key is a [`Spec`]. Rows are measured in "key units": a normal key is
//! 1.0 wide, and the widths in each row add up to the same total so the right
//! edges line up. To change the keyboard, edit [`us_qwerty`].

use crate::keys::*;

/// Modifiers that latch: tap once for the next key only, tap twice to lock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mod {
    Shift,
    Ctrl,
    Alt,
    Meta,
    Func,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Send this Linux key code, wrapped in whatever modifiers are latched.
    Key(u16),
    /// Latch or lock a modifier.
    Mod(Mod),
    /// Software caps lock: a persistent Shift, independent of the sticky Shift.
    Caps,
    /// Filler: lines the rows up without drawing or typing anything.
    None,
}

/// Visual style. The numbers match `KeyData.kind` in `ui/app.slint`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Char = 0,
    Func = 1,
    Space = 2,
    /// Invisible filler (the gap around the arrow cluster).
    Blank = 3,
}

/// What a key becomes while Fn is latched (for example the left arrow turns into Home).
#[derive(Clone, Copy, Debug)]
pub struct FnAlt {
    pub code: u16,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub action: Action,
    /// Main label. Letters are lower case; Shift upper-cases them.
    pub label: &'static str,
    /// Symbol typed with Shift (drawn small above the label). Empty for letters.
    pub shifted: &'static str,
    /// SVG path on a 24x24 grid, drawn instead of (or above) the label.
    pub icon: &'static str,
    /// Width in key units.
    pub w: f32,
    pub kind: Kind,
    /// Keep firing while the key is held down.
    pub repeat: bool,
    /// Use the small label size.
    pub small: bool,
    pub fn_alt: Option<FnAlt>,
}

pub struct Row {
    /// Distance from the top of the keyboard, in key units.
    pub y: f32,
    /// Row height in key units.
    pub h: f32,
    pub keys: Vec<Spec>,
}

pub struct Layout {
    /// Width of the widest row, in key units.
    pub cols: f32,
    /// Total height, in key units.
    pub total_h: f32,
    pub rows: Vec<Row>,
}

/// Key glyphs as SVG paths on a 24x24 grid, drawn as 1.7px-ish outlines.
pub mod icons {
    pub const SHIFT: &str = "M12 3.5 L3.5 13 H8.5 V20 H15.5 V13 H20.5 Z";
    pub const BACKSPACE: &str =
        "M9 5.5 H20 V18.5 H9 L2.5 12 Z M12.8 9.5 L16.8 14.5 M16.8 9.5 L12.8 14.5";
    pub const RETURN: &str = "M20 5.5 V12 H5.5 M9.5 8 L5.5 12 L9.5 16";
    pub const TAB: &str = "M3.5 12 H17.5 M13.5 8 L17.5 12 L13.5 16 M20.5 6 V18";
    pub const COMMAND: &str = "M9 9 V6.5 A2.5 2.5 0 1 0 6.5 9 H17.5 A2.5 2.5 0 1 0 15 6.5 V17.5 A2.5 2.5 0 1 0 17.5 15 H6.5 A2.5 2.5 0 1 0 9 17.5 Z";
    pub const OPTION: &str = "M3.5 7 H8.5 L15.5 17 H20.5 M13.5 7 H20.5";
    pub const CONTROL: &str = "M6 15 L12 9 L18 15";
    pub const LEFT: &str = "M14.5 5.5 L8 12 L14.5 18.5";
    pub const RIGHT: &str = "M9.5 5.5 L16 12 L9.5 18.5";
    pub const UP: &str = "M5.5 14.5 L12 8 L18.5 14.5";
    pub const DOWN: &str = "M5.5 9.5 L12 16 L18.5 9.5";
}

const F_KEYS: [u16; 12] = [
    KEY_F1, KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10, KEY_F11,
    KEY_F12,
];

const F_LABELS: [&str; 12] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
];

impl Spec {
    fn base(action: Action) -> Spec {
        Spec {
            action,
            label: "",
            shifted: "",
            icon: "",
            w: 1.0,
            kind: Kind::Char,
            repeat: false,
            small: false,
            fn_alt: None,
        }
    }
}

/// A character key: label plus the symbol typed with Shift ("" for letters).
fn ch(code: u16, label: &'static str, shifted: &'static str) -> Spec {
    Spec {
        label,
        shifted,
        ..Spec::base(Action::Key(code))
    }
}

/// A darker "function" key such as Shift, Tab or an arrow.
fn func(action: Action, label: &'static str, icon: &'static str, w: f32) -> Spec {
    Spec {
        label,
        icon,
        w,
        kind: Kind::Func,
        ..Spec::base(action)
    }
}

fn small(spec: Spec) -> Spec {
    Spec {
        small: true,
        ..spec
    }
}

fn repeating(spec: Spec) -> Spec {
    Spec {
        repeat: true,
        ..spec
    }
}

fn wide(spec: Spec, w: f32) -> Spec {
    Spec { w, ..spec }
}

/// Invisible filler so the rows keep lining up (see the arrow cluster).
fn blank(w: f32) -> Spec {
    Spec {
        w,
        kind: Kind::Blank,
        ..Spec::base(Action::None)
    }
}

fn with_alt(spec: Spec, code: u16, label: &'static str) -> Spec {
    Spec {
        fn_alt: Some(FnAlt { code, label }),
        ..spec
    }
}

/// Full US QWERTY: function row, number row, three letter rows and a bottom
/// row with an inverted-T arrow cluster. Every row is 15 key units wide.
pub fn us_qwerty() -> Layout {
    let letter = |code: u16, label: &'static str| ch(code, label, "");
    let arrow = |code: u16, icon: &'static str, alt_code: u16, alt_label: &'static str| {
        repeating(with_alt(
            func(Action::Key(code), "", icon, 1.0),
            alt_code,
            alt_label,
        ))
    };

    // Function row (shorter keys): 1.5 + 12 + 1.5
    let mut r0 = vec![small(func(Action::Key(KEY_ESC), "esc", "", 1.5))];
    for (i, code) in F_KEYS.iter().enumerate() {
        r0.push(small(func(Action::Key(*code), F_LABELS[i], "", 1.0)));
    }
    r0.push(small(repeating(func(
        Action::Key(KEY_DELETE),
        "del",
        "",
        1.5,
    ))));

    // Number row: 13 + 2
    let r1 = vec![
        ch(KEY_GRAVE, "`", "~"),
        ch(KEY_1, "1", "!"),
        ch(KEY_2, "2", "@"),
        ch(KEY_3, "3", "#"),
        ch(KEY_4, "4", "$"),
        ch(KEY_5, "5", "%"),
        ch(KEY_6, "6", "^"),
        ch(KEY_7, "7", "&"),
        ch(KEY_8, "8", "*"),
        ch(KEY_9, "9", "("),
        ch(KEY_0, "0", ")"),
        ch(KEY_MINUS, "-", "_"),
        ch(KEY_EQUAL, "=", "+"),
        repeating(func(Action::Key(KEY_BACKSPACE), "", icons::BACKSPACE, 2.0)),
    ];

    // Top letter row: 1.5 + 12 + 1.5
    let r2 = vec![
        func(Action::Key(KEY_TAB), "", icons::TAB, 1.5),
        letter(KEY_Q, "q"),
        letter(KEY_W, "w"),
        letter(KEY_E, "e"),
        letter(KEY_R, "r"),
        letter(KEY_T, "t"),
        letter(KEY_Y, "y"),
        letter(KEY_U, "u"),
        letter(KEY_I, "i"),
        letter(KEY_O, "o"),
        letter(KEY_P, "p"),
        ch(KEY_LEFTBRACE, "[", "{"),
        ch(KEY_RIGHTBRACE, "]", "}"),
        wide(ch(KEY_BACKSLASH, "\\", "|"), 1.5),
    ];

    // Home row: 1.75 + 11 + 2.25
    let r3 = vec![
        small(func(Action::Caps, "caps", "", 1.75)),
        letter(KEY_A, "a"),
        letter(KEY_S, "s"),
        letter(KEY_D, "d"),
        letter(KEY_F, "f"),
        letter(KEY_G, "g"),
        letter(KEY_H, "h"),
        letter(KEY_J, "j"),
        letter(KEY_K, "k"),
        letter(KEY_L, "l"),
        ch(KEY_SEMICOLON, ";", ":"),
        ch(KEY_APOSTROPHE, "'", "\""),
        func(Action::Key(KEY_ENTER), "", icons::RETURN, 2.25),
    ];

    // Bottom letter row: 2.25 + 10 + 0.75 gap + 1 (up arrow) + 1.
    // The up arrow sits directly above the down arrow in the row below,
    // completing the inverted-T cluster; a narrow right shift fills the
    // last slot so the row still adds up to 15 key units.
    let r4 = vec![
        func(Action::Mod(Mod::Shift), "", icons::SHIFT, 2.25),
        letter(KEY_Z, "z"),
        letter(KEY_X, "x"),
        letter(KEY_C, "c"),
        letter(KEY_V, "v"),
        letter(KEY_B, "b"),
        letter(KEY_N, "n"),
        letter(KEY_M, "m"),
        ch(KEY_COMMA, ",", "<"),
        ch(KEY_DOT, ".", ">"),
        ch(KEY_SLASH, "/", "?"),
        blank(0.75),
        arrow(KEY_UP, icons::UP, KEY_PAGEUP, "pg up"),
        func(Action::Mod(Mod::Shift), "", icons::SHIFT, 1.0),
    ];

    // Modifier row: 4 x 1.25 + 5.75 + 1.25 + 3 x 1
    let r5 = vec![
        small(func(Action::Mod(Mod::Func), "fn", "", 1.25)),
        func(Action::Mod(Mod::Ctrl), "ctrl", icons::CONTROL, 1.25),
        func(Action::Mod(Mod::Alt), "alt", icons::OPTION, 1.25),
        func(Action::Mod(Mod::Meta), "super", icons::COMMAND, 1.25),
        Spec {
            w: 5.75,
            kind: Kind::Space,
            repeat: true,
            ..Spec::base(Action::Key(KEY_SPACE))
        },
        func(Action::Mod(Mod::Alt), "alt", icons::OPTION, 1.25),
        arrow(KEY_LEFT, icons::LEFT, KEY_HOME, "home"),
        arrow(KEY_DOWN, icons::DOWN, KEY_PAGEDOWN, "pg dn"),
        arrow(KEY_RIGHT, icons::RIGHT, KEY_END, "end"),
    ];

    let plan = vec![
        (0.62_f32, r0),
        (1.0, r1),
        (1.0, r2),
        (1.0, r3),
        (1.0, r4),
        (1.0, r5),
    ];

    let mut rows = Vec::new();
    let mut y = 0.0_f32;
    let mut cols = 0.0_f32;
    for (h, keys) in plan {
        let width: f32 = keys.iter().map(|k| k.w).sum();
        if width > cols {
            cols = width;
        }
        rows.push(Row { y, h, keys });
        y += h;
    }

    Layout {
        cols,
        total_h: y,
        rows,
    }
}
