//! Keyboard behaviour with no UI types: sticky modifiers, labels and typing.

use crate::inject::Injector;
use crate::keys::{KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT};
use crate::layout::{Action, Layout, Mod, Spec};

/// State of a sticky modifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Latch {
    Off,
    /// Applies to the next key only.
    Once,
    /// Stays on until tapped again.
    Locked,
}

impl Latch {
    fn next(self) -> Latch {
        match self {
            Latch::Off => Latch::Once,
            Latch::Once => Latch::Locked,
            Latch::Locked => Latch::Off,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mods {
    pub shift: Latch,
    /// Software caps lock, kept separate from the sticky Shift so tapping
    /// Shift never releases the lock (and locking Caps does not show on the
    /// Shift keys).
    pub caps: bool,
    pub ctrl: Latch,
    pub alt: Latch,
    pub meta: Latch,
    pub func: Latch,
}

impl Mods {
    fn new() -> Mods {
        Mods {
            shift: Latch::Off,
            caps: false,
            ctrl: Latch::Off,
            alt: Latch::Off,
            meta: Latch::Off,
            func: Latch::Off,
        }
    }

    /// Whether Shift is effectively held: sticky Shift or caps lock.
    fn shift_held(&self) -> bool {
        self.caps || self.shift != Latch::Off
    }

    pub fn get(&self, m: Mod) -> Latch {
        match m {
            Mod::Shift => self.shift,
            Mod::Ctrl => self.ctrl,
            Mod::Alt => self.alt,
            Mod::Meta => self.meta,
            Mod::Func => self.func,
        }
    }

    fn slot(&mut self, m: Mod) -> &mut Latch {
        match m {
            Mod::Shift => &mut self.shift,
            Mod::Ctrl => &mut self.ctrl,
            Mod::Alt => &mut self.alt,
            Mod::Meta => &mut self.meta,
            Mod::Func => &mut self.func,
        }
    }

    /// Releases every modifier that was only armed for one key.
    fn clear_once(&mut self) {
        for latch in [
            &mut self.shift,
            &mut self.ctrl,
            &mut self.alt,
            &mut self.meta,
            &mut self.func,
        ] {
            if *latch == Latch::Once {
                *latch = Latch::Off;
            }
        }
    }
}

/// Everything the UI needs to draw one key.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyView {
    pub id: i32,
    pub label: String,
    pub sub: String,
    pub icon: &'static str,
    pub x: f32,
    pub w: f32,
    pub kind: i32,
    pub active: bool,
    pub locked: bool,
    pub repeat: bool,
    pub small: bool,
}

pub struct Engine {
    layout: Layout,
    /// Every key in row order; a key's id is its index here.
    flat: Vec<Spec>,
    mods: Mods,
    injector: Box<dyn Injector>,
}

fn is_letter(label: &str) -> bool {
    label.len() == 1 && label.as_bytes()[0].is_ascii_alphabetic()
}

impl Engine {
    pub fn new(layout: Layout, injector: Box<dyn Injector>) -> Engine {
        let flat: Vec<Spec> = layout
            .rows
            .iter()
            .flat_map(|row| row.keys.iter().copied())
            .collect();
        Engine {
            layout,
            flat,
            mods: Mods::new(),
            injector,
        }
    }

    pub fn cols(&self) -> f32 {
        self.layout.cols
    }

    pub fn total_h(&self) -> f32 {
        self.layout.total_h
    }

    /// `(y, height)` of every row, in key units.
    pub fn row_geometry(&self) -> Vec<(f32, f32)> {
        self.layout.rows.iter().map(|row| (row.y, row.h)).collect()
    }

    /// The current look of every key, row by row.
    pub fn views(&self) -> Vec<Vec<KeyView>> {
        let mut id = 0usize;
        let mut rows = Vec::with_capacity(self.layout.rows.len());
        for row in &self.layout.rows {
            let mut x = 0.0_f32;
            let mut keys = Vec::with_capacity(row.keys.len());
            for spec in &row.keys {
                keys.push(self.view_of(spec, id, x));
                x += spec.w;
                id += 1;
            }
            rows.push(keys);
        }
        rows
    }

    fn view_of(&self, spec: &Spec, id: usize, x: f32) -> KeyView {
        let shift_on = self.mods.shift_held();
        let fn_on = self.mods.func != Latch::Off;

        let mut view = KeyView {
            id: id as i32,
            label: spec.label.to_string(),
            sub: String::new(),
            icon: spec.icon,
            x,
            w: spec.w,
            kind: spec.kind as i32,
            active: false,
            locked: false,
            repeat: spec.repeat,
            small: spec.small,
        };

        match spec.action {
            Action::Mod(m) => {
                let latch = self.mods.get(m);
                view.active = latch != Latch::Off;
                view.locked = latch == Latch::Locked;
            }
            Action::Caps => {
                view.active = self.mods.caps;
                view.locked = self.mods.caps;
            }
            Action::Key(_) => match spec.fn_alt {
                Some(alt) if fn_on => {
                    view.label = alt.label.to_string();
                    view.icon = "";
                    view.small = true;
                }
                _ => {
                    if spec.shifted.is_empty() {
                        if shift_on && is_letter(spec.label) {
                            view.label = spec.label.to_uppercase();
                        }
                    } else if shift_on {
                        view.label = spec.shifted.to_string();
                    } else {
                        view.sub = spec.shifted.to_string();
                    }
                }
            },
            // Filler keys are invisible in the UI; nothing to dress up.
            Action::None => {}
        }
        view
    }

    /// A key was pressed (or auto-repeated) in the UI.
    pub fn press(&mut self, id: usize) {
        let spec = match self.flat.get(id) {
            Some(spec) => *spec,
            None => return,
        };

        match spec.action {
            Action::Mod(m) => {
                let slot = self.mods.slot(m);
                *slot = slot.next();
            }
            Action::Caps => {
                // Caps lock is its own switch: tapping Shift while it is on
                // only arms a sticky Shift, it never releases the lock.
                self.mods.caps = !self.mods.caps;
            }
            Action::Key(code) => {
                let code = match spec.fn_alt {
                    Some(alt) if self.mods.func != Latch::Off => alt.code,
                    _ => code,
                };
                self.type_key(code);
                self.mods.clear_once();
            }
            // Invisible filler: never typed, never latches anything.
            Action::None => {}
        }
    }

    /// Taps `code` wrapped in the currently latched modifiers.
    fn type_key(&mut self, code: u16) {
        let mut held: Vec<u16> = Vec::new();
        if self.mods.ctrl != Latch::Off {
            held.push(KEY_LEFTCTRL);
        }
        if self.mods.alt != Latch::Off {
            held.push(KEY_LEFTALT);
        }
        if self.mods.meta != Latch::Off {
            held.push(KEY_LEFTMETA);
        }
        if self.mods.shift_held() {
            held.push(KEY_LEFTSHIFT);
        }

        for &modifier in &held {
            self.injector.key(modifier, true);
        }
        self.injector.key(code, true);
        self.injector.key(code, false);
        for &modifier in held.iter().rev() {
            self.injector.key(modifier, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::*;
    use crate::layout::us_qwerty;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<(u16, bool)>>>;

    struct Recorder(Log);

    impl Injector for Recorder {
        fn key(&mut self, code: u16, down: bool) {
            self.0.borrow_mut().push((code, down));
        }

        fn name(&self) -> String {
            "recorder".to_string()
        }
    }

    fn setup() -> (Engine, Log) {
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::new(us_qwerty(), Box::new(Recorder(log.clone())));
        (engine, log)
    }

    fn id_of(engine: &Engine, action: Action) -> usize {
        engine
            .flat
            .iter()
            .position(|spec| spec.action == action)
            .expect("key is not in the layout")
    }

    fn take(log: &Log) -> Vec<(u16, bool)> {
        log.borrow_mut().drain(..).collect()
    }

    #[test]
    fn plain_key_is_a_press_and_a_release() {
        let (mut engine, log) = setup();
        let a = id_of(&engine, Action::Key(KEY_A));
        engine.press(a);
        assert_eq!(take(&log), vec![(KEY_A, true), (KEY_A, false)]);
    }

    #[test]
    fn one_tap_of_shift_applies_to_one_key() {
        let (mut engine, log) = setup();
        let shift = id_of(&engine, Action::Mod(Mod::Shift));
        let a = id_of(&engine, Action::Key(KEY_A));

        engine.press(shift);
        engine.press(a);
        assert_eq!(
            take(&log),
            vec![
                (KEY_LEFTSHIFT, true),
                (KEY_A, true),
                (KEY_A, false),
                (KEY_LEFTSHIFT, false)
            ]
        );

        engine.press(a);
        assert_eq!(take(&log), vec![(KEY_A, true), (KEY_A, false)]);
    }

    #[test]
    fn two_taps_lock_shift_and_a_third_unlocks_it() {
        let (mut engine, log) = setup();
        let shift = id_of(&engine, Action::Mod(Mod::Shift));
        let a = id_of(&engine, Action::Key(KEY_A));

        engine.press(shift);
        engine.press(shift);
        engine.press(a);
        engine.press(a);

        let shifted = vec![
            (KEY_LEFTSHIFT, true),
            (KEY_A, true),
            (KEY_A, false),
            (KEY_LEFTSHIFT, false),
        ];
        let mut expected = shifted.clone();
        expected.extend(shifted);
        assert_eq!(take(&log), expected);

        engine.press(shift);
        engine.press(a);
        assert_eq!(take(&log), vec![(KEY_A, true), (KEY_A, false)]);
    }

    #[test]
    fn fn_turns_the_left_arrow_into_home() {
        let (mut engine, log) = setup();
        let func = id_of(&engine, Action::Mod(Mod::Func));
        let left = id_of(&engine, Action::Key(KEY_LEFT));

        engine.press(func);
        engine.press(left);
        assert_eq!(take(&log), vec![(KEY_HOME, true), (KEY_HOME, false)]);
    }

    #[test]
    fn fn_turns_the_right_arrow_into_end() {
        let (mut engine, log) = setup();
        let func = id_of(&engine, Action::Mod(Mod::Func));
        let right = id_of(&engine, Action::Key(KEY_RIGHT));

        engine.press(func);
        engine.press(right);
        assert_eq!(take(&log), vec![(KEY_END, true), (KEY_END, false)]);
    }

    #[test]
    fn locked_fn_applies_to_every_arrow_until_unlatched() {
        let (mut engine, log) = setup();
        let func = id_of(&engine, Action::Mod(Mod::Func));
        let left = id_of(&engine, Action::Key(KEY_LEFT));

        engine.press(func);
        engine.press(func); // lock
        engine.press(left);
        engine.press(left);
        engine.press(func); // unlock
        engine.press(left);

        let home = vec![(KEY_HOME, true), (KEY_HOME, false)];
        let mut expected = home.clone();
        expected.extend(home);
        expected.extend(vec![(KEY_LEFT, true), (KEY_LEFT, false)]);
        assert_eq!(take(&log), expected);
    }

    #[test]
    fn fn_does_not_change_plain_keys() {
        let (mut engine, log) = setup();
        let func = id_of(&engine, Action::Mod(Mod::Func));
        let a = id_of(&engine, Action::Key(KEY_A));

        engine.press(func);
        engine.press(a);
        assert_eq!(take(&log), vec![(KEY_A, true), (KEY_A, false)]);
    }

    #[test]
    fn caps_locks_shift_until_toggled_off() {
        let (mut engine, log) = setup();
        let caps = id_of(&engine, Action::Caps);
        let a = id_of(&engine, Action::Key(KEY_A));

        engine.press(caps);
        engine.press(a);
        engine.press(a);
        let shifted = vec![
            (KEY_LEFTSHIFT, true),
            (KEY_A, true),
            (KEY_A, false),
            (KEY_LEFTSHIFT, false),
        ];
        let mut expected = shifted.clone();
        expected.extend(shifted);
        assert_eq!(take(&log), expected);

        engine.press(caps);
        engine.press(a);
        assert_eq!(take(&log), vec![(KEY_A, true), (KEY_A, false)]);
    }

    #[test]
    fn one_tap_of_shift_on_top_of_caps_keeps_the_lock() {
        let (mut engine, log) = setup();
        let caps = id_of(&engine, Action::Caps);
        let shift = id_of(&engine, Action::Mod(Mod::Shift));
        let a = id_of(&engine, Action::Key(KEY_A));

        engine.press(caps); // caps lock on
        engine.press(shift); // sticky Shift for the next key only
        engine.press(a); // shifted; the Once latch clears, caps stays on
        engine.press(a); // still shifted, by caps alone
        engine.press(caps); // caps lock off
        engine.press(a); // plain again

        let shifted = vec![
            (KEY_LEFTSHIFT, true),
            (KEY_A, true),
            (KEY_A, false),
            (KEY_LEFTSHIFT, false),
        ];
        let mut expected = shifted.clone();
        expected.extend(shifted);
        expected.extend(vec![(KEY_A, true), (KEY_A, false)]);
        assert_eq!(take(&log), expected);
    }

    #[test]
    fn caps_does_not_light_the_shift_keys() {
        let (mut engine, _log) = setup();
        let caps = id_of(&engine, Action::Caps);
        let shift = id_of(&engine, Action::Mod(Mod::Shift));

        engine.press(caps);
        let views = engine.views();
        let flat: Vec<&KeyView> = views.iter().flatten().collect();
        assert!(flat[caps].active && flat[caps].locked);
        assert!(!flat[shift].active && !flat[shift].locked);
    }

    #[test]
    fn stacked_modifiers_press_in_order_and_release_in_reverse() {
        let (mut engine, log) = setup();
        let ctrl = id_of(&engine, Action::Mod(Mod::Ctrl));
        let alt = id_of(&engine, Action::Mod(Mod::Alt));
        let meta = id_of(&engine, Action::Mod(Mod::Meta));
        let shift = id_of(&engine, Action::Mod(Mod::Shift));
        let a = id_of(&engine, Action::Key(KEY_A));

        engine.press(ctrl);
        engine.press(alt);
        engine.press(meta);
        engine.press(shift);
        engine.press(a);

        assert_eq!(
            take(&log),
            vec![
                (KEY_LEFTCTRL, true),
                (KEY_LEFTALT, true),
                (KEY_LEFTMETA, true),
                (KEY_LEFTSHIFT, true),
                (KEY_A, true),
                (KEY_A, false),
                (KEY_LEFTSHIFT, false),
                (KEY_LEFTMETA, false),
                (KEY_LEFTALT, false),
                (KEY_LEFTCTRL, false),
            ]
        );
    }

    #[test]
    fn out_of_range_presses_are_ignored() {
        let (mut engine, log) = setup();
        engine.press(9999);
        assert!(take(&log).is_empty());
    }
}
