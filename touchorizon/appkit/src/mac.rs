//! The NSTouchBar itself: one scrolling NSScrubber of item cells (the hotbar, then the main
//! inventory) and an off-hand button, set as the game window's `touchBar`. AppKit is reached with
//! plain messages (objc2's `msg_send!`), so no objc2-app-kit features are needed.
//!
//! The scrubber asks its data source for cells whenever it likes (inside `reloadData` or later),
//! so the cells it reads live in their own thread local, never borrowed across a message to
//! AppKit, and the bar's state is taken out of its cell while it is synced.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    path::{Path, PathBuf},
    ptr,
};

use block2::RcBlock;
use objc2::{
    ClassType, Encode, Encoding, MainThreadMarker, MainThreadOnly, RefEncode, class, define_class,
    msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject, AnyProtocol, Bool, NSObject},
    sel,
};

use crate::{Action, HOTBAR, Slot, View, action_for, icons, shown};

/// The cell's size in points: the Touch Bar is 30 points high.
const CELL_W: f64 = 36.0;
const CELL_H: f64 = 30.0;
/// The scrubber's width: about fifteen cells, the rest scrolls.
const SCRUBBER_W: f64 = 560.0;
const ITEM_VIEW: &str = "dev.touchorizon.slot";
const SLOTS_ITEM: &str = "dev.touchorizon.slots";
const OFFHAND_ITEM: &str = "dev.touchorizon.offhand";

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Size {
    w: f64,
    h: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Rect {
    origin: Point,
    size: Size,
}
unsafe impl Encode for Point {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl RefEncode for Point {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
unsafe impl Encode for Size {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl RefEncode for Size {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
unsafe impl Encode for Rect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
}
unsafe impl RefEncode for Rect {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        origin: Point { x, y },
        size: Size { w, h },
    }
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSFontAttributeName: *const AnyObject;
    static NSForegroundColorAttributeName: *const AnyObject;
}
unsafe extern "C" {
    fn class_addProtocol(cls: *const AnyClass, protocol: *const AnyProtocol) -> Bool;
}

fn ns_string(s: &str) -> Retained<AnyObject> {
    unsafe {
        let alloc: Allocated<AnyObject> = msg_send![class!(NSString), alloc];
        msg_send![
            alloc,
            initWithBytes: s.as_ptr().cast::<c_void>(),
            length: s.len(),
            encoding: 4usize // NSUTF8StringEncoding
        ]
    }
}

fn ns_array(objects: &[&AnyObject]) -> Retained<AnyObject> {
    let ptrs: Vec<*const AnyObject> = objects.iter().map(|o| ptr::from_ref(*o)).collect();
    unsafe { msg_send![class!(NSArray), arrayWithObjects: ptrs.as_ptr(), count: ptrs.len()] }
}

thread_local! {
    /// The scrubber's cells (NSImage each), in item order.
    static CELLS: RefCell<Vec<Retained<AnyObject>>> = const { RefCell::new(Vec::new()) };
    /// Taps not yet taken by the game.
    static ACTIONS: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
    /// Set while the bar changes its own selection, which is not a tap.
    static SUPPRESS: Cell<bool> = const { Cell::new(false) };
    /// A tap moved the scrubber's selection: put it back on the selected hotbar slot.
    static RESELECT: Cell<bool> = const { Cell::new(false) };
    static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
}

define_class!(
    // The scrubber's data source and delegate, and the off-hand button's target.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ToucHorizonTarget"]
    struct Target;

    impl Target {
        #[unsafe(method(numberOfItemsForScrubber:))]
        fn number_of_items(&self, _scrubber: *mut AnyObject) -> isize {
            CELLS.with_borrow(|c| c.len() as isize)
        }

        #[unsafe(method(scrubber:viewForItemAtIndex:))]
        fn view_for_item(&self, scrubber: *mut AnyObject, index: isize) -> *mut AnyObject {
            let Some(scrubber) = (unsafe { scrubber.as_ref() }) else {
                return ptr::null_mut();
            };
            let id = ns_string(ITEM_VIEW);
            let view: Option<Retained<AnyObject>> =
                unsafe { msg_send![scrubber, makeItemWithIdentifier: &*id, owner: None::<&AnyObject>] };
            let Some(view) = view else {
                return ptr::null_mut();
            };
            let image = CELLS.with_borrow(|c| c.get(index.max(0) as usize).cloned());
            if let Some(image) = image {
                let _: () = unsafe { msg_send![&view, setImage: &*image] };
            }
            Retained::autorelease_return(view)
        }

        #[unsafe(method(scrubber:didSelectItemAtIndex:))]
        fn did_select(&self, _scrubber: *mut AnyObject, index: isize) {
            if SUPPRESS.get() || index < 0 {
                return;
            }
            if let Some(action) = action_for(index as usize) {
                ACTIONS.with_borrow_mut(|a| a.push(action));
                RESELECT.set(true);
            }
        }

        #[unsafe(method(swapOffhand:))]
        fn swap_offhand(&self, _sender: *mut AnyObject) {
            ACTIONS.with_borrow_mut(|a| a.push(Action::SwapOffhand));
        }
    }
);

impl Target {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let cls = Self::class();
        for name in [c"NSScrubberDataSource", c"NSScrubberDelegate"] {
            if let Some(p) = AnyProtocol::get(name) {
                unsafe { class_addProtocol(cls, p) };
            }
        }
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

struct Bar {
    touch_bar: Retained<AnyObject>,
    scrubber: Retained<AnyObject>,
    _target: Retained<Target>,
    /// The window the bar was set on, compared by address only.
    window: *const AnyObject,
    /// Calls since the window was last checked.
    since_check: u32,
    shown: Vec<Option<Slot>>,
    selected: Option<usize>,
    offhand_button: Option<bool>,
    resources: PathBuf,
    /// Item icons (NSImage), None for an item the resources have nothing for.
    icons: HashMap<String, Option<Retained<AnyObject>>>,
}

impl Bar {
    fn new(mtm: MainThreadMarker) -> Self {
        let target = Target::new(mtm);
        unsafe {
            let alloc: Allocated<AnyObject> = msg_send![class!(NSScrubber), alloc];
            let scrubber: Retained<AnyObject> =
                msg_send![alloc, initWithFrame: rect(0.0, 0.0, SCRUBBER_W, CELL_H)];
            let view_class: &AnyClass = class!(NSScrubberImageItemView);
            let _: () = msg_send![
                &scrubber,
                registerClass: view_class,
                forItemIdentifier: &*ns_string(ITEM_VIEW)
            ];
            let layout: Retained<AnyObject> = msg_send![class!(NSScrubberFlowLayout), new];
            let _: () = msg_send![&layout, setItemSize: Size { w: CELL_W, h: CELL_H }];
            let _: () = msg_send![&layout, setItemSpacing: 2.0f64];
            let _: () = msg_send![&scrubber, setScrubberLayout: &*layout];
            let _: () = msg_send![&scrubber, setMode: 1isize]; // NSScrubberModeFree: it scrolls
            let style: Retained<AnyObject> =
                msg_send![class!(NSScrubberSelectionStyle), outlineOverlayStyle];
            let _: () = msg_send![&scrubber, setSelectionOverlayStyle: &*style];
            let _: () = msg_send![&scrubber, setShowsAdditionalContentIndicators: true];
            let _: () = msg_send![&scrubber, setDataSource: &*target];
            let _: () = msg_send![&scrubber, setDelegate: &*target];

            let alloc: Allocated<AnyObject> = msg_send![class!(NSCustomTouchBarItem), alloc];
            let slots: Retained<AnyObject> =
                msg_send![alloc, initWithIdentifier: &*ns_string(SLOTS_ITEM)];
            let _: () = msg_send![&slots, setView: &*scrubber];

            let symbol: Option<Retained<AnyObject>> = msg_send![
                class!(NSImage),
                imageWithSystemSymbolName: &*ns_string("arrow.left.arrow.right"),
                accessibilityDescription: &*ns_string("Swap with off hand")
            ];
            let button: Retained<AnyObject> = match &symbol {
                Some(image) => msg_send![
                    class!(NSButton),
                    buttonWithImage: &**image,
                    target: &*target,
                    action: sel!(swapOffhand:)
                ],
                None => msg_send![
                    class!(NSButton),
                    buttonWithTitle: &*ns_string("Off hand"),
                    target: &*target,
                    action: sel!(swapOffhand:)
                ],
            };
            let alloc: Allocated<AnyObject> = msg_send![class!(NSCustomTouchBarItem), alloc];
            let offhand: Retained<AnyObject> =
                msg_send![alloc, initWithIdentifier: &*ns_string(OFFHAND_ITEM)];
            let _: () = msg_send![&offhand, setView: &*button];

            let touch_bar: Retained<AnyObject> = msg_send![class!(NSTouchBar), new];
            let items = ns_array(&[&slots, &offhand]);
            let set: Retained<AnyObject> = msg_send![class!(NSSet), setWithArray: &*items];
            let _: () = msg_send![&touch_bar, setTemplateItems: &*set];

            Self {
                touch_bar,
                scrubber,
                _target: target,
                window: ptr::null(),
                since_check: u32::MAX,
                shown: Vec::new(),
                selected: None,
                offhand_button: None,
                resources: PathBuf::new(),
                icons: HashMap::new(),
            }
        }
    }

    fn set_items(&self, offhand_button: bool) {
        let slots = ns_string(SLOTS_ITEM);
        let offhand = ns_string(OFFHAND_ITEM);
        let ids = if offhand_button {
            ns_array(&[&slots, &offhand])
        } else {
            ns_array(&[&slots])
        };
        let _: () = unsafe { msg_send![&self.touch_bar, setDefaultItemIdentifiers: &*ids] };
    }

    /// The game's window: the key window, else the main one.
    fn game_window() -> Option<Retained<AnyObject>> {
        unsafe {
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            let key: Option<Retained<AnyObject>> = msg_send![&app, keyWindow];
            key.or_else(|| msg_send![&app, mainWindow])
        }
    }

    /// Sets the bar on the game's window when it isn't there (a new window, or something
    /// replaced it). Checked twice a second or so, not every frame.
    fn attach(&mut self) {
        self.since_check = self.since_check.saturating_add(1);
        if self.since_check < 30 {
            return;
        }
        self.since_check = 0;
        let Some(window) = Self::game_window() else {
            return;
        };
        let current: Option<Retained<AnyObject>> = unsafe { msg_send![&window, touchBar] };
        let ours = current
            .as_deref()
            .is_some_and(|c| ptr::eq(c, &*self.touch_bar));
        if !ours {
            let _: () = unsafe { msg_send![&window, setTouchBar: &*self.touch_bar] };
        }
        self.window = Retained::as_ptr(&window);
    }

    fn detach(&self) {
        let Some(window) = Self::game_window() else {
            return;
        };
        let current: Option<Retained<AnyObject>> = unsafe { msg_send![&window, touchBar] };
        if current
            .as_deref()
            .is_some_and(|c| ptr::eq(c, &*self.touch_bar))
        {
            let _: () = unsafe { msg_send![&window, setTouchBar: None::<&AnyObject>] };
        }
    }

    fn icon(&mut self, item: &str) -> Option<Retained<AnyObject>> {
        if let Some(icon) = self.icons.get(item) {
            return icon.clone();
        }
        let image = icons::load(&self.resources, item).and_then(|i| ns_image(&i));
        self.icons.insert(item.to_owned(), image.clone());
        image
    }

    fn sync(&mut self, view: &View, resources: &Path) {
        if self.resources != resources {
            self.resources = resources.to_owned();
            self.icons.clear();
            self.shown.clear();
        }
        if self.offhand_button != Some(view.offhand_button) {
            self.offhand_button = Some(view.offhand_button);
            self.set_items(view.offhand_button);
        }
        let shown = shown(view);
        let reload = self.shown != shown;
        if reload {
            let cells: Vec<Retained<AnyObject>> = shown
                .iter()
                .enumerate()
                .map(|(i, slot)| {
                    let icon = slot.as_ref().and_then(|s| self.icon(&s.item));
                    cell_image(i, slot.as_ref(), icon, i == view.selected)
                })
                .collect();
            CELLS.set(cells);
            self.shown = shown.to_vec();
        }
        let selected = view.selected.min(HOTBAR - 1);
        if reload || RESELECT.replace(false) || self.selected != Some(selected) {
            if !reload && self.selected != Some(selected) && !self.shown.is_empty() {
                // The selected slot's cell is drawn brighter: redraw the two that changed.
                let changed: Vec<usize> = [self.selected, Some(selected)]
                    .into_iter()
                    .flatten()
                    .filter(|&i| i < self.shown.len())
                    .collect();
                for &i in &changed {
                    let slot = self.shown[i].clone();
                    let icon = slot.as_ref().and_then(|s| self.icon(&s.item));
                    let image = cell_image(i, slot.as_ref(), icon, i == selected);
                    CELLS.with_borrow_mut(|c| {
                        if let Some(cell) = c.get_mut(i) {
                            *cell = image;
                        }
                    });
                }
                self.reload_items(&changed);
            }
            SUPPRESS.set(true);
            unsafe {
                if reload {
                    let _: () = msg_send![&self.scrubber, reloadData];
                }
                let _: () = msg_send![&self.scrubber, setSelectedIndex: selected as isize];
            }
            SUPPRESS.set(false);
            self.selected = Some(selected);
        }
        self.attach();
    }

    fn reload_items(&self, items: &[usize]) {
        unsafe {
            let set: Retained<AnyObject> = msg_send![class!(NSMutableIndexSet), new];
            for &i in items {
                let _: () = msg_send![&set, addIndex: i];
            }
            SUPPRESS.set(true);
            let _: () = msg_send![&self.scrubber, reloadItemsAtIndexes: &*set];
            SUPPRESS.set(false);
        }
    }
}

/// An NSImage of the icon's pixels, through PNG (AppKit decodes it; no bitmap planes to fill).
fn ns_image(icon: &icons::Icon) -> Option<Retained<AnyObject>> {
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, icon.width, icon.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&icon.rgba).ok()?;
    }
    unsafe {
        let data: Retained<AnyObject> = msg_send![
            class!(NSData),
            dataWithBytes: png_bytes.as_ptr().cast::<c_void>(),
            length: png_bytes.len()
        ];
        let alloc: Allocated<AnyObject> = msg_send![class!(NSImage), alloc];
        msg_send![alloc, initWithData: &*data]
    }
}

fn font(size: f64, bold: bool) -> Retained<AnyObject> {
    unsafe {
        if bold {
            msg_send![class!(NSFont), boldSystemFontOfSize: size]
        } else {
            msg_send![class!(NSFont), systemFontOfSize: size]
        }
    }
}

fn color(white: f64, alpha: f64) -> Retained<AnyObject> {
    unsafe { msg_send![class!(NSColor), colorWithWhite: white, alpha: alpha] }
}

/// Draws `text` with its lower-left corner at `at`, in `font` and `color`; returns its size.
fn draw_text(text: &str, at: Point, font: &AnyObject, color: &AnyObject) -> Size {
    unsafe {
        let keys = ns_array(&[&*NSFontAttributeName, &*NSForegroundColorAttributeName]);
        let values = ns_array(&[font, color]);
        let attrs: Retained<AnyObject> =
            msg_send![class!(NSDictionary), dictionaryWithObjects: &*values, forKeys: &*keys];
        let s = ns_string(text);
        let size: Size = msg_send![&s, sizeWithAttributes: &*attrs];
        let _: () = msg_send![&s, drawAtPoint: at, withAttributes: &*attrs];
        size
    }
}

fn text_size(text: &str, font: &AnyObject) -> Size {
    unsafe {
        let keys = ns_array(&[&*NSFontAttributeName]);
        let values = ns_array(&[font]);
        let attrs: Retained<AnyObject> =
            msg_send![class!(NSDictionary), dictionaryWithObjects: &*values, forKeys: &*keys];
        msg_send![&ns_string(text), sizeWithAttributes: &*attrs]
    }
}

/// The cell for inventory slot `index`: the hotbar's on a lighter tile with its number, the
/// selected one lighter still; the icon (or the item's name when it has none) and the count.
/// Drawn when AppKit renders it, at the Touch Bar's own scale.
fn cell_image(
    index: usize,
    slot: Option<&Slot>,
    icon: Option<Retained<AnyObject>>,
    selected: bool,
) -> Retained<AnyObject> {
    let slot = slot.cloned();
    let hotbar = index < HOTBAR;
    let handler = RcBlock::new(move |_dst: Rect| -> Bool {
        unsafe {
            let ctx: Option<Retained<AnyObject>> =
                msg_send![class!(NSGraphicsContext), currentContext];
            if let Some(ctx) = &ctx {
                let _: () = msg_send![&**ctx, setImageInterpolation: 1usize]; // None: crisp pixels
            }
            let tile = match (hotbar, selected) {
                (true, true) => 0.34,
                (true, false) => 0.18,
                (false, _) => 0.08,
            };
            let _: () = msg_send![&color(1.0, tile), setFill];
            let path: Retained<AnyObject> = msg_send![
                class!(NSBezierPath),
                bezierPathWithRoundedRect: rect(0.0, 0.0, CELL_W, CELL_H),
                xRadius: 5.0f64,
                yRadius: 5.0f64
            ];
            let _: () = msg_send![&path, fill];
            if hotbar {
                let small = font(7.0, false);
                draw_text(
                    &(index + 1).to_string(),
                    Point { x: 3.0, y: CELL_H - 10.0 },
                    &small,
                    &color(1.0, 0.55),
                );
            }
            if let Some(slot) = &slot {
                match &icon {
                    Some(icon) => {
                        let side = 22.0;
                        let _: () = msg_send![
                            &**icon,
                            drawInRect: rect((CELL_W - side) / 2.0, (CELL_H - side) / 2.0, side, side),
                            fromRect: Rect::default(),
                            operation: 2usize, // NSCompositingOperationSourceOver
                            fraction: 1.0f64
                        ];
                    }
                    None => {
                        let name = icons::pretty_name(&slot.item);
                        let short: String = name.chars().filter(|c| *c != ' ').take(5).collect();
                        let f = font(8.0, false);
                        let size = text_size(&short, &f);
                        draw_text(
                            &short,
                            Point { x: (CELL_W - size.w) / 2.0, y: (CELL_H - size.h) / 2.0 },
                            &f,
                            &color(1.0, 0.9),
                        );
                    }
                }
                if slot.count > 1 {
                    let f = font(10.0, true);
                    let text = slot.count.to_string();
                    let size = text_size(&text, &f);
                    let at = Point { x: CELL_W - size.w - 2.0, y: -1.0 };
                    // A shadow first, as the game draws counts.
                    draw_text(&text, Point { x: at.x + 0.8, y: at.y - 0.8 }, &f, &color(0.0, 0.9));
                    draw_text(&text, at, &f, &color(1.0, 1.0));
                }
            }
        }
        Bool::YES
    });
    unsafe {
        msg_send![
            class!(NSImage),
            imageWithSize: Size { w: CELL_W, h: CELL_H },
            flipped: false,
            drawingHandler: &*handler
        ]
    }
}

pub(crate) fn show(view: &View, resources: &Path) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let mut bar = BAR.take().unwrap_or_else(|| Bar::new(mtm));
    bar.sync(view, resources);
    BAR.set(Some(bar));
}

pub(crate) fn hide() {
    if MainThreadMarker::new().is_none() {
        return;
    }
    if let Some(bar) = BAR.take() {
        bar.detach();
    }
    CELLS.set(Vec::new());
    ACTIONS.set(Vec::new());
}

pub(crate) fn take_actions() -> Vec<Action> {
    ACTIONS.take()
}

/// For the demo binary: the cells as the scrubber would ask for them, and taps by index.
#[doc(hidden)]
pub mod probe {
    use super::*;

    /// The number of cells and, for each, the view the data source returns.
    pub fn cells() -> (isize, Vec<bool>) {
        let Some(bar) = BAR.take() else {
            return (0, Vec::new());
        };
        let target: &Target = &bar._target;
        let n: isize = unsafe { msg_send![target, numberOfItemsForScrubber: &*bar.scrubber] };
        let views = (0..n)
            .map(|i| {
                let v: *mut AnyObject = unsafe {
                    msg_send![target, scrubber: &*bar.scrubber, viewForItemAtIndex: i]
                };
                !v.is_null()
            })
            .collect();
        BAR.set(Some(bar));
        (n, views)
    }

    /// A tap on cell `index`, as the scrubber reports it.
    pub fn tap(index: isize) {
        let Some(bar) = BAR.take() else { return };
        let _: () = unsafe {
            msg_send![&*bar._target, scrubber: &*bar.scrubber, didSelectItemAtIndex: index]
        };
        BAR.set(Some(bar));
    }

    /// Writes cell `index`'s image, at 2x, as a PNG at `path`.
    pub fn save_cell(index: usize, path: &Path) -> bool {
        let Some(image) = CELLS.with_borrow(|c| c.get(index).cloned()) else {
            return false;
        };
        unsafe {
            let cg_rect = rect(0.0, 0.0, CELL_W * 2.0, CELL_H * 2.0);
            let alloc: Allocated<AnyObject> = msg_send![class!(NSImage), alloc];
            let big: Retained<AnyObject> =
                msg_send![alloc, initWithSize: Size { w: CELL_W * 2.0, h: CELL_H * 2.0 }];
            let _: () = msg_send![&big, lockFocus];
            let _: () = msg_send![
                &image,
                drawInRect: cg_rect,
                fromRect: Rect::default(),
                operation: 2usize,
                fraction: 1.0f64
            ];
            let _: () = msg_send![&big, unlockFocus];
            let tiff: Option<Retained<AnyObject>> = msg_send![&big, TIFFRepresentation];
            let Some(tiff) = tiff else { return false };
            let rep: Option<Retained<AnyObject>> =
                msg_send![class!(NSBitmapImageRep), imageRepWithData: &*tiff];
            let Some(rep) = rep else { return false };
            let props: Retained<AnyObject> = msg_send![class!(NSDictionary), dictionary];
            let png: Option<Retained<AnyObject>> =
                msg_send![&rep, representationUsingType: 4usize, properties: &*props]; // PNG
            let Some(png) = png else { return false };
            let ok: bool = msg_send![
                &png,
                writeToFile: &*ns_string(&path.to_string_lossy()),
                atomically: true
            ];
            ok
        }
    }
}
