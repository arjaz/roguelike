use tcod::colors::*;
use tcod::console::*;
use tcod::input::{self, Event, Key};

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::map::{Map, MAP_HEIGHT, MAP_WIDTH};
use crate::scene::{new_scene, Scene};

pub const FPS_LIMIT: i32 = 60;
pub const SCREEN_WIDTH: i32 = 80;
pub const SCREEN_HEIGHT: i32 = 50;

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayerAction {
    Turn,
    Exit,
}

pub struct Tcod {
    pub root: Root,
    pub con: Offscreen,
    pub panel: Offscreen,
    pub key: Key,
}

pub fn get_unique_id() -> usize {
    static UNIQUE_ID: AtomicUsize = AtomicUsize::new(0);
    UNIQUE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn main_menu(tcod: &mut Tcod) {
    let img = tcod::image::Image::from_file("menu_background.png")
        .ok()
        .expect("Background image not found");

    tcod.root.set_default_foreground(LIGHT_RED);
    tcod.root.print_ex(
        SCREEN_WIDTH / 2,
        SCREEN_HEIGHT / 2,
        BackgroundFlag::None,
        TextAlignment::Center,
        "World of Rust and Steel",
    );

    while !tcod.root.window_closed() {
        // Show the image at twice the regular console resolution
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut tcod.root, (0, 0));

        // Show the available options and wait for the input
        let choices = &["Play a new game", "Exit"];
        let choice = menu("", choices, 24, &mut tcod.root);

        match choice {
            Some(0) => {
                println!("Start the game");
                let mut scene = new_scene(tcod);
                game_loop(tcod, &mut scene);
            }
            Some(1) => {
                println!("Exit the game");
                break;
            }
            _ => {}
        }
    }
}

// TODO: Split this mess into separate functions
fn tick(
    tcod: &mut Tcod,
    mut scene: &mut Scene,
    previous_player_position: &mut (i32, i32),
) -> Option<PlayerAction> {
    // Clear previous frame
    tcod.con.clear();

    // Get user input
    match input::check_for_event(input::KEY_PRESS) {
        Some((_, Event::Key(k))) => tcod.key = k,
        _ => tcod.key = Default::default(),
    }

    // Render the map
    render_map(tcod, &scene.map);

    // Render every render component
    for render_component in scene.render_components.as_mut_slice() {
        // TODO: split apart map position and render position
        // Find position
        if let Some(position) = scene
            .position_components
            .iter()
            .find(|c| c.entity == render_component.entity)
        {
            render_component.draw(&mut tcod.con, &position);
        }
    }

    // Update the the root screen with the con
    blit(
        &tcod.con,
        (0, 0),
        (MAP_WIDTH, MAP_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );

    // TODO: rework that
    // Render the panel
    tcod.panel.set_default_background(BLACK);
    tcod.panel.clear();

    pub const PANEL_HEIGHT: i32 = 7;
    pub const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;
    blit(
        &tcod.panel,
        (0, 0),
        (SCREEN_WIDTH, PANEL_HEIGHT),
        &mut tcod.root,
        (0, PANEL_Y),
        1.0,
        1.0,
    );

    tcod.root.flush();

    // Handle user input
    *previous_player_position = scene
        .position_components
        .iter()
        .find(|c| c.entity == scene.player_id)
        .expect("No player found. How did you even get here?")
        .pos();

    let player_action = handle_keys(&tcod, &mut scene);
    player_action
}

pub fn game_loop(tcod: &mut Tcod, scene: &mut Scene) {
    let mut previous_player_position = scene
        .position_components
        .iter()
        .find(|c| c.entity == scene.player_id)
        .expect("No player found. How did you even get here?")
        .pos();
    while !tcod.root.window_closed() {
        let player_action = tick(tcod, scene, &mut previous_player_position);
        if player_action == Some(PlayerAction::Exit) {
            break;
        }
    }
}

fn handle_keys(tcod: &Tcod, scene: &mut Scene) -> Option<PlayerAction> {
    use tcod::input::KeyCode::*;

    let player_alive = scene
        .combat_components
        .iter()
        .find(|c| c.entity == scene.player_id)
        .expect("No player found. How did you even get here")
        .alive;

    // let player_position = &mut scene
    //     .position_components
    //     .iter_mut()
    //     .find(|c| c.entity == scene.player_id)
    //     .expect("No player found. How did you even get here");
    let player_position = {
        let id = scene.player_id;
        &mut scene
            .position_components
            .iter_mut()
            .find(|c| c.entity == id)
            .expect("No player found. How did you even get here")
    };

    return match (tcod.key, tcod.key.text(), player_alive) {
        (Key { code: Escape, .. }, _, _) => Some(PlayerAction::Exit),
        (Key { code: Text, .. }, "h", true) | (Key { code: NumPad4, .. }, _, true) => {
            player_position.try_move(&scene.map, -1, 0);
            Some(PlayerAction::Turn)
        }
        (Key { code: Text, .. }, "j", true) | (Key { code: NumPad2, .. }, _, true) => {
            player_position.try_move(&scene.map, 0, 1);
            Some(PlayerAction::Turn)
        }
        (Key { code: Text, .. }, "k", true) | (Key { code: NumPad8, .. }, _, true) => {
            player_position.try_move(&scene.map, 0, -1);
            Some(PlayerAction::Turn)
        }
        (Key { code: Text, .. }, "l", true) | (Key { code: NumPad6, .. }, _, true) => {
            player_position.try_move(&scene.map, 1, 0);
            Some(PlayerAction::Turn)
        }
        _ => None,
    };
}

fn render_map(tcod: &mut Tcod, map: &Map) {
    const WALL_COLOR: Color = Color {
        r: 0x2e,
        g: 0x34,
        b: 0x40,
    };
    const FLOOR_COLOR: Color = Color {
        r: 0x4c,
        g: 0x56,
        b: 0x6a,
    };

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let wall = map[x as usize][y as usize].block_sight;
            let color = match wall {
                true => WALL_COLOR,
                false => FLOOR_COLOR,
            };

            tcod.con
                .set_char_background(x, y, color, BackgroundFlag::Set);
        }
    }
}

// Provides the user with a menu of options to select from
// Returns the index of the selected option or None if no option was selected
pub fn menu<T: AsRef<str>>(
    header: &str,
    options: &[T],
    width: i32,
    root: &mut Root,
) -> Option<usize> {
    // We use 26 letters of the alphabet to provide options
    assert!(options.len() <= 26, "There aren't enough letters");

    // Calculate the total height of the header (with auto-wrap) and one line per option
    let header_height = if header.is_empty() {
        0
    } else {
        root.get_height_rect(0, 0, width, SCREEN_HEIGHT, header)
    };
    let height = options.len() as i32 + header_height;

    // Create an offscreen console to represent the menu
    let mut window = Offscreen::new(width, height);

    // Print the header
    window.set_default_foreground(WHITE);
    window.print_rect_ex(
        0,
        0,
        width,
        height,
        BackgroundFlag::None,
        TextAlignment::Left,
        header,
    );

    // Print the options
    for (index, option_text) in options.iter().enumerate() {
        let menu_letter = (b'a' + index as u8) as char;
        let text = format!("({}) {}", menu_letter, option_text.as_ref());
        window.print_ex(
            0,
            header_height + index as i32,
            BackgroundFlag::None,
            TextAlignment::Left,
            text,
        );
    }

    // Blit to the root screen
    let x = SCREEN_WIDTH / 2 - width / 2;
    let y = SCREEN_HEIGHT / 2 - height / 2;
    blit(&window, (0, 0), (width, height), root, (x, y), 1.0, 0.7);

    root.flush();
    let key = root.wait_for_keypress(true);

    // Convert an ASCII key to index
    if key.printable.is_alphabetic() {
        let index = key.printable.to_ascii_lowercase() as usize - 'a' as usize;
        if index < options.len() {
            Some(index)
        } else {
            None
        }
    } else {
        None
    }
}
