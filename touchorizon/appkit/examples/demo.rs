//! The Touch Bar without the game: a window with a made-up inventory.
//!
//!   cargo run --example demo -- RESOURCES            probe it: cells, taps, cell PNGs, then exit
//!   cargo run --example demo -- RESOURCES --run      keep it open; tap the Touch Bar to play
//!
//! RESOURCES is a folder holding `assets/minecraft/...` (Horizon's prepared resources, or the
//! Minecraft client jar unzipped).
#[cfg(target_os = "macos")]
fn main() {
    use std::{cell::RefCell, path::PathBuf, rc::Rc};

    use block2::RcBlock;
    use objc2::{
        Encode, Encoding, MainThreadMarker, RefEncode, class, msg_send,
        rc::{Allocated, Retained},
        runtime::AnyObject,
    };
    use touchorizon_appkit::{Action, Slot, View, probe};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect([f64; 4]);
    unsafe impl Encode for Rect {
        const ENCODING: Encoding = Encoding::Struct(
            "CGRect",
            &[
                Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]),
                Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]),
            ],
        );
    }
    unsafe impl RefEncode for Rect {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    let args: Vec<String> = std::env::args().collect();
    let resources = PathBuf::from(args.get(1).expect("usage: demo RESOURCES [--run]"));
    let run = args.iter().any(|a| a == "--run");
    let _mtm = MainThreadMarker::new().expect("the main thread");

    let items = [
        ("minecraft:diamond_sword", 1),
        ("minecraft:diamond_pickaxe", 1),
        ("minecraft:oak_planks", 64),
        ("minecraft:cobblestone", 37),
        ("minecraft:torch", 16),
        ("minecraft:cooked_beef", 12),
        ("minecraft:water_bucket", 1),
        ("minecraft:oak_leaves", 8),
        ("minecraft:grass_block", 3),
        ("minecraft:iron_ingot", 21),
        ("minecraft:redstone", 64),
        ("minecraft:ender_pearl", 4),
        ("minecraft:chest", 2),
        ("minecraft:bow", 1),
        ("minecraft:arrow", 48),
        ("minecraft:not_an_item", 5),
    ];
    let mut slots: Vec<Option<Slot>> = vec![None; 36];
    for (i, (item, count)) in items.iter().enumerate() {
        let at = if i < 9 { i } else { i + 3 };
        slots[at] = Some(Slot { item: (*item).into(), count: *count });
    }
    let state = Rc::new(RefCell::new(View {
        slots,
        selected: 0,
        inventory: true,
        offhand_button: true,
    }));
    let offhand: Rc<RefCell<Option<Slot>>> = Rc::new(RefCell::new(None));

    unsafe {
        let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
        let _: bool = msg_send![&app, setActivationPolicy: 0isize];
        let alloc: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
        let window: Retained<AnyObject> = msg_send![
            alloc,
            initWithContentRect: Rect([200.0, 200.0, 480.0, 200.0]),
            styleMask: 15usize,
            backing: 2usize,
            defer: false
        ];
        let title: Retained<AnyObject> = msg_send![class!(NSString), stringWithUTF8String: c"ToucHorizon demo".as_ptr()];
        let _: () = msg_send![&window, setTitle: &*title];
        let _: () = msg_send![&window, setReleasedWhenClosed: false];
        let _: () = msg_send![&window, makeKeyAndOrderFront: None::<&AnyObject>];
        let _: () = msg_send![&app, activateIgnoringOtherApps: true];
        std::mem::forget(window);
    }

    // Apply a tap to the made-up inventory, as the game does.
    let apply = {
        let state = state.clone();
        let offhand = offhand.clone();
        move |action: Action| {
            let mut v = state.borrow_mut();
            let sel = v.selected;
            match action {
                Action::Select(i) => v.selected = i,
                Action::Swap(i) => v.slots.swap(i, sel),
                Action::SwapOffhand => std::mem::swap(&mut v.slots[sel], &mut *offhand.borrow_mut()),
            }
            println!("tap: {action:?}");
        }
    };

    // Show it twice: the second call must attach (it checks the window on its first call).
    touchorizon_appkit::show(&state.borrow(), &resources);
    touchorizon_appkit::show(&state.borrow(), &resources);

    if !run {
        let (n, views) = probe::cells();
        println!("cells: {n}, views returned: {}", views.iter().filter(|v| **v).count());
        let out = std::env::temp_dir().join("touchorizon-cells");
        let _ = std::fs::create_dir_all(&out);
        for i in [0usize, 2, 7, 8, 9, 12, 24] {
            let ok = probe::save_cell(i, &out.join(format!("cell-{i:02}.png")));
            println!("cell {i} saved: {ok}");
        }
        // Tap hotbar slot 3, then inventory slot 12 (swaps it into slot 3), then the off hand.
        probe::tap(2);
        probe::tap(12);
        for a in touchorizon_appkit::take_actions() {
            apply(a);
        }
        apply(Action::SwapOffhand);
        touchorizon_appkit::show(&state.borrow(), &resources);
        let v = state.borrow();
        println!(
            "selected {} holds {:?}; slot 13 holds {:?}; off hand {:?}",
            v.selected + 1,
            v.slots[2],
            v.slots[12],
            offhand.borrow()
        );
        let (n, _) = probe::cells();
        println!("cells after: {n}");
        touchorizon_appkit::hide();
        println!("hidden");
        return;
    }

    println!("Running: tap the Touch Bar (close the window or Ctrl-C to quit).");
    let tick = RcBlock::new(move |_timer: *mut AnyObject| {
        for a in touchorizon_appkit::take_actions() {
            apply(a);
        }
        touchorizon_appkit::show(&state.borrow(), &resources);
    });
    unsafe {
        let _: Retained<AnyObject> = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: 1.0f64 / 30.0,
            repeats: true,
            block: &*tick
        ];
        let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![&app, run];
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The Touch Bar is a Mac's.");
}
