mod components;
mod engine;
mod map;
mod scene;

use tcod::console::*;

fn main() {
    tcod::system::set_fps(engine::FPS_LIMIT);

    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(engine::SCREEN_WIDTH, engine::SCREEN_HEIGHT)
        .title("Rust and Steel")
        .init();

    let mut tcod = engine::Tcod {
        root,
        con: Offscreen::new(map::MAP_WIDTH, map::MAP_HEIGHT),
        panel: Offscreen::new(engine::SCREEN_WIDTH, engine::SCREEN_HEIGHT),
        key: Default::default(),
    };

    engine::main_menu(&mut tcod);
}
