// ToucHorizon's settings page.
fn touchorizon_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::touchorizon as m;
    if mod_id != m::ID {
        return;
    }
    buttons.extend(game.touchorizon_settings().labels());
}
