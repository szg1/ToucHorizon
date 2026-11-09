// ToucHorizon's row in Options > Mods.
fn touchorizon_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::touchorizon as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("ToucHorizon {}", m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
