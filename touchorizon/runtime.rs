// ToucHorizon's game side: a module of `game` (horizon-mixin.toml's [[module]]), so it reads the
// player's inventory and clicks the inventory menu as the game's own keys do. The Touch Bar is
// the `touchorizon_appkit` crate.
use std::cell::Cell;

use super::{Game, Screen};
use touchorizon_appkit as bar;

/// The mod's id: its settings folder and its row in Options > Mods.
pub const ID: &str = "touchorizon";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "Your hotbar and inventory on the Touch Bar: tap a hotbar item to \
     select it, tap an inventory item to swap it into the selected slot.";

/// The settings page's choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The main inventory after the hotbar (scroll to it), or the hotbar alone.
    pub inventory: bool,
    /// The swap-with-off-hand button.
    pub offhand: bool,
}

impl Settings {
    /// From the stored values; both on when missing.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let flag = |key: &str| get(key).is_none_or(|v| v.trim() != "false");
        Self {
            inventory: flag("inventory"),
            offhand: flag("offhand"),
        }
    }
    pub fn labels(&self) -> Vec<String> {
        vec![
            format!(
                "Touch Bar shows: {}",
                if self.inventory { "Hotbar + inventory" } else { "Hotbar only" }
            ),
            format!("Off-hand button: {}", if self.offhand { "ON" } else { "OFF" }),
        ]
    }
    pub fn click(&mut self, i: usize) {
        match i {
            0 => self.inventory = !self.inventory,
            1 => self.offhand = !self.offhand,
            _ => {}
        }
    }
    pub fn stored(&self) -> [(&'static str, String); 2] {
        let flag = |b: bool| if b { "true" } else { "false" }.to_owned();
        [("inventory", flag(self.inventory)), ("offhand", flag(self.offhand))]
    }
}

thread_local! {
    /// Read on the first frame, then kept.
    static SETTINGS: Cell<Option<Settings>> = const { Cell::new(None) };
}

impl Game<'_> {
    pub(super) fn touchorizon_settings(&self) -> Settings {
        if let Some(s) = SETTINGS.get() {
            return s;
        }
        let s = Settings::read(|key| self.native_setting(ID, key));
        SETTINGS.set(Some(s));
        s
    }

    pub(super) fn touchorizon_set_settings(&self, settings: Settings) {
        SETTINGS.set(Some(settings));
        for (key, value) in settings.stored() {
            self.native_set_setting(ID, key, &value);
        }
    }

    /// Each frame: the taps since the last one, then the inventory as it is now. Off the
    /// Touch Bar outside a world, or when the mod is turned off in Options > Mods.
    pub(super) fn touchorizon_frame(&mut self) {
        let in_world = self.player.is_some() && self.title.is_none();
        if !self.native_window() || !self.native_on(ID) || !in_world {
            bar::take_actions();
            bar::hide();
            return;
        }
        for action in bar::take_actions() {
            self.touchorizon_apply(action);
        }
        let settings = self.touchorizon_settings();
        let slots = (0..bar::SLOTS)
            .map(|i| {
                self.inventory.get(i).and_then(Option::as_ref).map(|s| bar::Slot {
                    item: s.item.to_string(),
                    count: s.count,
                })
            })
            .collect();
        let view = bar::View {
            slots,
            selected: self.selected.min(bar::HOTBAR - 1),
            inventory: settings.inventory,
            offhand_button: settings.offhand,
        };
        bar::show(&view, &self.vanilla_root());
    }

    /// A tap, as the keys would do it: playing, or with the inventory (or a chest...) open, and
    /// never while a stack hangs on the cursor.
    fn touchorizon_apply(&mut self, action: bar::Action) {
        let menu_open = self.screen == Screen::Inventory && self.title.is_none();
        if !(self.native_playing() || menu_open) || self.carried.is_some() {
            return;
        }
        match action {
            // The number keys (`Inventory.setSelectedSlot`); the next tick tells a server.
            bar::Action::Select(i) if i < bar::HOTBAR => self.selected = i,
            // The selected slot's number key over slot `i` in the inventory screen.
            bar::Action::Swap(i) if (bar::HOTBAR..bar::SLOTS).contains(&i) => {
                let button = self.selected as i32;
                self.touchorizon_swap(i, button);
            }
            // F over the selected slot.
            bar::Action::SwapOffhand => {
                let selected = self.selected;
                self.touchorizon_swap(selected, 40);
            }
            _ => {}
        }
    }

    /// A SWAP click of the player's inventory menu on `slot` with `button` (a hotbar slot, or
    /// 40 for the off hand): on a server it is predicted and sent as a `ContainerClick` on the
    /// open menu (the player's own, id 0, when none is open); in single player the menu engine
    /// does it.
    fn touchorizon_swap(&mut self, slot: usize, button: i32) {
        use horizon_client::menu::Input;
        let ops = [(Some(slot), button, Input::Swap)];
        let done = if self.remote.is_some() && self.screen != Screen::Inventory {
            self.remote_menu_ops(&ops)
        } else {
            self.menu_ops(&ops)
        };
        if done && let Some(key) = self.container {
            self.container_changed(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_on_and_round_trip() {
        let none = |_: &str| None;
        let mut s = Settings::read(none);
        assert_eq!(s, Settings { inventory: true, offhand: true });
        s.click(0);
        assert_eq!(s.labels()[0], "Touch Bar shows: Hotbar only");
        let stored = s.stored();
        let back = Settings::read(|k| stored.iter().find(|p| p.0 == k).map(|p| p.1.clone()));
        assert_eq!(back, s);
    }
}
