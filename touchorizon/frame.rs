// Every frame, on the main thread (where AppKit lives): ToucHorizon applies the Touch Bar's taps
// and shows the inventory as it is now.
fn touchorizon_on_frame(game: &mut crate::game::Game<'_>) {
    game.touchorizon_frame();
}
