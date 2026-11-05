//! OSK: a macOS-styled on-screen keyboard written in Rust with Slint.
//!
//! * `layout.rs` describes the keys as data.
//! * `engine.rs` holds the behaviour (sticky modifiers, labels, typing).
//! * `inject.rs` delivers key events to the system.
//! * `ui/app.slint` draws everything.

mod engine;
mod inject;
mod keys;
mod layout;

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

slint::include_modules!();

const HELP: &str = "\
OSK: on-screen keyboard (Rust + Slint)

Usage: osk [options]

Options:
  --light      start with the light theme
  --dry-run    do not create a virtual keyboard, just print key events
  -h, --help   show this help
  -V, --version
               print the version and exit
";

// Matches `min-width`/`min-height` in ui/app.slint: 585 x 260 has (almost
// exactly) the same aspect as the preferred 1040 x 462, so the smallest
// window still shows the keyboard with its intended proportions instead of
// stretching it.
const MIN_WIDTH: f32 = 585.0;
const MAX_WIDTH: f32 = 2400.0;

struct Args {
    light: bool,
    dry_run: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        light: false,
        dry_run: false,
    };
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--light" => args.light = true,
            "--dry-run" => args.dry_run = true,
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("osk {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => eprintln!("osk: ignoring unknown option {other}"),
        }
    }
    args
}

fn to_data(view: &engine::KeyView) -> KeyData {
    KeyData {
        id: view.id,
        label: view.label.as_str().into(),
        sub: view.sub.as_str().into(),
        icon: view.icon.into(),
        x: view.x,
        w: view.w,
        kind: view.kind,
        active: view.active,
        locked: view.locked,
        repeat: view.repeat,
        small: view.small,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let args = parse_args();

    let injector: Box<dyn inject::Injector> = if args.dry_run {
        Box::new(inject::DryRun)
    } else {
        inject::best_available()
    };
    let backend = injector.name();
    let live = injector.is_live();

    let engine = Rc::new(RefCell::new(engine::Engine::new(
        layout::us_qwerty(),
        injector,
    )));

    let ui = AppWindow::new()?;
    ui.set_status(backend.into());
    ui.set_live(live);
    ui.global::<Theme>().set_dark(!args.light);

    let (cols, total_h, geometry, views) = {
        let eng = engine.borrow();
        (eng.cols(), eng.total_h(), eng.row_geometry(), eng.views())
    };
    ui.set_cols(cols);
    ui.set_total_h(total_h);

    // One model per row, so a label change only touches the keys that changed.
    let key_models: Vec<Rc<VecModel<KeyData>>> = views
        .iter()
        .map(|row| {
            let keys: Vec<KeyData> = row.iter().map(to_data).collect();
            Rc::new(VecModel::from(keys))
        })
        .collect();

    let rows: Vec<RowData> = geometry
        .iter()
        .zip(key_models.iter())
        .map(|(&(y, h), model)| RowData {
            y,
            h,
            keys: ModelRc::from(model.clone()),
        })
        .collect();
    ui.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));

    // Re-reads every key's look from the engine and updates what changed.
    let refresh = {
        let engine = engine.clone();
        move || {
            let views = engine.borrow().views();
            for (model, row) in key_models.iter().zip(views.iter()) {
                for (index, view) in row.iter().enumerate() {
                    let data = to_data(view);
                    if model.row_data(index).as_ref() != Some(&data) {
                        model.set_row_data(index, data);
                    }
                }
            }
        }
    };

    ui.on_key_activated({
        let engine = engine.clone();
        move |id| {
            if id >= 0 {
                engine.borrow_mut().press(id as usize);
                refresh();
            }
        }
    });

    ui.on_toggle_theme({
        let ui = ui.as_weak();
        move || {
            if let Some(ui) = ui.upgrade() {
                let theme = ui.global::<Theme>();
                theme.set_dark(!theme.get_dark());
            }
        }
    });

    // Drag the title bar to move the window (not possible on some Wayland compositors).
    ui.on_drag_by({
        let ui = ui.as_weak();
        move |dx, dy| {
            if let Some(ui) = ui.upgrade() {
                let window = ui.window();
                let scale = window.scale_factor();
                let position = window.position();
                window.set_position(slint::PhysicalPosition::new(
                    position.x + (dx * scale).round() as i32,
                    position.y + (dy * scale).round() as i32,
                ));
            }
        }
    });

    // Grow or shrink the window, keeping its proportions. The top-left
    // corner stays put: re-positioning around the centre needs
    // set_position() right after set_size(), which compositors and
    // back-ends apply at different times and is not worth the risk.
    ui.on_resize_by({
        let ui = ui.as_weak();
        move |factor| {
            if let Some(ui) = ui.upgrade() {
                let window = ui.window();
                let size = window.size().to_logical(window.scale_factor());
                if size.width <= 0.0 || size.height <= 0.0 {
                    return;
                }
                let width = (size.width * factor).clamp(MIN_WIDTH, MAX_WIDTH);
                let height = width * size.height / size.width;
                window.set_size(slint::LogicalSize::new(width, height));
            }
        }
    });

    ui.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    ui.run()
}
