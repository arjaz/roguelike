use tcod::console::*;
use tcod::map::Map as FovMap;

mod ai;
mod equipment;
mod fighter;
mod game;
mod item;
mod object;
mod render;
mod room;
mod save;

const FPS_LIMIT: i32 = 60;

fn main() {
    tcod::system::set_fps(FPS_LIMIT);

    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(render::SCREEN_WIDTH, render::SCREEN_HEIGHT)
        .title("Rust and Steel")
        .init();

    let mut tcod = render::Tcod {
        root,
        con: Offscreen::new(game::MAP_WIDTH, game::MAP_HEIGHT),
        panel: Offscreen::new(render::SCREEN_WIDTH, render::PANEL_HEIGHT),
        fov: FovMap::new(game::MAP_WIDTH, game::MAP_HEIGHT),
        key: Default::default(),
        mouse: Default::default(),
    };

    render::main_menu(&mut tcod);
}
