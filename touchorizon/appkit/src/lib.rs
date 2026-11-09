//! ToucHorizon's Touch Bar, apart from the game: the game hands it a [`View`] of the player's
//! inventory each frame and takes back the [`Action`]s the player tapped. Everything here runs on
//! the main thread, where the game's window and AppKit live. Off macOS it does nothing.
use std::path::Path;

pub mod icons;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use mac::probe;

/// The hotbar's slots: inventory indices 0..9.
pub const HOTBAR: usize = 9;
/// The hotbar and the main inventory: inventory indices 0..36.
pub const SLOTS: usize = 36;

/// One stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    /// `minecraft:oak_planks`.
    pub item: String,
    pub count: u32,
}

/// What the Touch Bar shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// The player's inventory in Minecraft's order: the hotbar (0..9), then the main inventory
    /// (9..36, top row first).
    pub slots: Vec<Option<Slot>>,
    /// The selected hotbar slot, 0..9.
    pub selected: usize,
    /// Show the main inventory after the hotbar (scroll to reach it), not just the hotbar.
    pub inventory: bool,
    /// Show the swap-with-off-hand button.
    pub offhand_button: bool,
}

/// What a tap asks the game to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Select hotbar slot 0..9, as its number key does.
    Select(usize),
    /// Swap main-inventory slot 9..36 with the selected hotbar slot, as pressing the selected
    /// slot's number key over it in the inventory screen does.
    Swap(usize),
    /// Swap the selected hotbar slot with the off hand, as F does.
    SwapOffhand,
}

/// The action for a tap on the Touch Bar's item `index` (an inventory index).
pub fn action_for(index: usize) -> Option<Action> {
    match index {
        i if i < HOTBAR => Some(Action::Select(i)),
        i if i < SLOTS => Some(Action::Swap(i)),
        _ => None,
    }
}

/// The items the Touch Bar lists for `view`: the hotbar, then the main inventory when shown.
pub fn shown(view: &View) -> &[Option<Slot>] {
    let n = if view.inventory { SLOTS } else { HOTBAR };
    &view.slots[..n.min(view.slots.len())]
}

/// Shows `view` on the Touch Bar of the game's window, building it the first time; `resources`
/// is the game's prepared resource folder (holding `assets/`), for the item icons. Cheap when
/// nothing changed: call it every frame.
pub fn show(view: &View, resources: &Path) {
    #[cfg(target_os = "macos")]
    mac::show(view, resources);
    #[cfg(not(target_os = "macos"))]
    let _ = (view, resources);
}

/// Takes the Touch Bar off the window (the mod was turned off, or the player left the world).
pub fn hide() {
    #[cfg(target_os = "macos")]
    mac::hide();
}

/// The taps since the last call, oldest first.
pub fn take_actions() -> Vec<Action> {
    #[cfg(target_os = "macos")]
    return mac::take_actions();
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taps_select_the_hotbar_and_swap_the_inventory() {
        assert_eq!(action_for(0), Some(Action::Select(0)));
        assert_eq!(action_for(8), Some(Action::Select(8)));
        assert_eq!(action_for(9), Some(Action::Swap(9)));
        assert_eq!(action_for(35), Some(Action::Swap(35)));
        assert_eq!(action_for(36), None);
    }

    #[test]
    fn hotbar_only_lists_nine() {
        let mut view = View {
            slots: vec![None; SLOTS],
            selected: 0,
            inventory: false,
            offhand_button: true,
        };
        assert_eq!(shown(&view).len(), 9);
        view.inventory = true;
        assert_eq!(shown(&view).len(), 36);
    }
}
