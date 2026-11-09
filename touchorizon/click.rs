// A click on ToucHorizon's settings page: change the setting and keep it.
fn touchorizon_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::touchorizon as m;
    if mod_id != m::ID {
        return;
    }
    let mut settings = game.touchorizon_settings();
    settings.click(button);
    game.touchorizon_set_settings(settings);
}
