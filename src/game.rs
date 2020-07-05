use std::cmp;

use serde::{Deserialize, Serialize};

use tcod::colors::*;
use tcod::console::*;
use tcod::input::{self, Event, Key};

use crate::ai::ai_take_turn;
use crate::equipment::{Equipment, Slot};
use crate::fighter::{DeathCallback, Fighter};
use crate::item::{drop_item, pick_item, use_item, Item};
use crate::object::{player_move_attack, Object};
use crate::render::{
    character_info_box, inventory_menu, menu, render_all, Tcod, LEVEL_SCREEN_WIDTH,
};
use crate::room::make_map;
use crate::save::save_game;

pub const MAP_WIDTH: i32 = 80;
pub const MAP_HEIGHT: i32 = 43;

pub const PLAYER: usize = 0;

pub const LEVEL_UP_BASE: i32 = 100;
pub const LEVEL_UP_FACTOR: i32 = 150;

// A tile object
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tile {
    pub blocked: bool,
    pub explored: bool,
    pub block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            explored: false,
            block_sight: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            explored: false,
            block_sight: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

// Alias for the Map type
pub type Map = Vec<Vec<Tile>>;

// A struct to represent the state of the game
#[derive(Serialize, Deserialize)]
pub struct Game {
    pub map: Map,
    pub messages: Messages,
    pub inventory: Vec<Object>,
    pub dungeon_level: u32,
}

// Messages log
#[derive(Serialize, Deserialize)]
pub struct Messages {
    pub messages: Vec<(String, Color)>,
}

impl Messages {
    pub fn new() -> Self {
        Messages { messages: vec![] }
    }

    // Add a new message
    pub fn add<T: Into<String>>(&mut self, message: T, color: Color) {
        self.messages.push((message.into(), color));
    }

    // Double-ended iterator over the messages
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &(String, Color)> {
        self.messages.iter()
    }
}

// Used to determine some value based on the current level
pub struct Transition {
    pub level: u32,
    pub value: u32,
}

pub fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table
        .iter()
        .rev()
        .find(|transition| transition.level <= level)
        .map_or(0, |transition| transition.value)
}

pub fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

pub fn mut_two<T>(first: usize, second: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first != second);
    let split_at_index = cmp::max(first, second);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first < second {
        (&mut first_slice[first], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second])
    }
}

pub fn initialize_fov(tcod: &mut Tcod, map: &Map) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !map[x as usize][y as usize].block_sight,
                !map[x as usize][y as usize].blocked,
            );
        }
    }
    tcod.con.clear();
}

pub fn new_game(tcod: &mut Tcod) -> (Game, Vec<Object>) {
    // Create player object
    let player = {
        let mut res = Object::new(0, 0, '@', "player", WHITE, true);
        res.alive = true;
        res.fighter = Some(Fighter {
            base_max_hp: 100,
            hp: 100,
            base_defense: 0,
            base_power: 5,
            xp: 0,
            on_death: DeathCallback::Player,
        });
        res
    };

    // List of game objects
    let mut objects = vec![player];

    const INITIAL_LEVEL: u32 = 1;
    let mut game = Game {
        map: make_map(&mut objects, INITIAL_LEVEL),
        messages: Messages::new(),
        inventory: vec![],
        dungeon_level: INITIAL_LEVEL,
    };

    let dagger = {
        let mut res = Object::new(0, 0, '-', "dagger", SKY, false);
        res.item = Some(Item::Sword);
        res.equipment = Some(Equipment {
            equipped: true,
            slot: Slot::LeftHand,
            max_hp_bonus: 0,
            power_bonus: 5,
            defense_bonus: 1,
        });
        res
    };
    game.inventory.push(dagger);

    initialize_fov(tcod, &game.map);

    game.messages
        .add("Prepare yourself to the world of rust and steel", RED);

    (game, objects)
}

pub fn play_game(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) {
    // Recompute the fov
    let mut previous_player_position = (-1, -1);

    while !tcod.root.window_closed() {
        // Clear previous frame
        tcod.con.clear();

        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => tcod.mouse = m,
            Some((_, Event::Key(k))) => tcod.key = k,
            _ => tcod.key = Default::default(),
        }

        // render the screen
        let fov_recompute = previous_player_position != (objects[PLAYER].pos());
        render_all(tcod, game, &objects, fov_recompute);

        tcod.root.flush();

        // check leveling up
        level_up(tcod, game, objects);

        // handle keys
        previous_player_position = objects[PLAYER].pos();
        let player_action = handle_keys(tcod, game, objects);
        if player_action == PlayerAction::Exit {
            save_game(game, objects).unwrap();
            break;
        }

        // Let monsters tke turn
        if objects[PLAYER].alive && player_action == PlayerAction::TookTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, tcod, game, objects);
                }
            }
        }
    }
}

// Return the position of the clicked tile, or (None, None) if right clicked
pub fn target_tile(
    tcod: &mut Tcod,
    game: &mut Game,
    objects: &[Object],
    max_range: Option<f32>,
) -> Option<(i32, i32)> {
    use tcod::input::KeyCode::Escape;
    tcod.mouse = Default::default();
    loop {
        // render the screen -> erase inventory and show the names under the cursor
        tcod.root.flush();

        let event = input::check_for_event(input::KEY_PRESS | input::MOUSE).map(|e| e.1);
        match event {
            Some(Event::Mouse(m)) => tcod.mouse = m,
            Some(Event::Key(k)) => tcod.key = k,
            None => tcod.key = Default::default(),
        }
        render_all(tcod, game, objects, false);

        let (x, y) = (tcod.mouse.cx as i32, tcod.mouse.cy as i32);

        // Chech if visible and in range
        let in_fov = (x < MAP_WIDTH) && (y < MAP_HEIGHT) && tcod.fov.is_in_fov(x, y);
        let in_range = max_range.map_or(true, |range| objects[PLAYER].distance(x, y) <= range);
        if tcod.mouse.lbutton_pressed && in_fov && in_range {
            return Some((x, y));
        }
        if tcod.mouse.rbutton_pressed || tcod.key.code == Escape {
            return None;
        }
    }
}

// Return the id of the clicked monster or None if no selected
pub fn target_monster(
    tcod: &mut Tcod,
    game: &mut Game,
    objects: &[Object],
    max_range: Option<f32>,
) -> Option<usize> {
    loop {
        match target_tile(tcod, game, objects, max_range) {
            Some((x, y)) => {
                for (id, obj) in objects.iter().enumerate() {
                    if obj.pos() == (x, y) && obj.fighter.is_some() && id != PLAYER {
                        return Some(id);
                    }
                }
            }
            None => return None,
        }
    }
}

pub fn next_level(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) {
    game.messages.add("You go deeper...", VIOLET);
    let heal_hp = objects[PLAYER].fighter.map_or(0, |f| f.base_max_hp / 2);
    objects[PLAYER].heal(heal_hp, game);

    game.dungeon_level += 1;
    game.map = make_map(objects, game.dungeon_level);
    initialize_fov(tcod, &game.map);
}

fn level_up(tcod: &mut Tcod, game: &mut Game, objects: &mut [Object]) {
    let player = &mut objects[PLAYER];
    let level_up_xp = LEVEL_UP_BASE + player.level * LEVEL_UP_FACTOR;

    if player.fighter.as_ref().map_or(0, |f| f.xp) >= level_up_xp {
        player.level += 1;
        game.messages.add("Your powers grow stronger", YELLOW);

        let fighter = player.fighter.as_mut().unwrap();
        let mut choice = None;

        while choice.is_none() {
            choice = menu(
                "Level up! Choose a stat to raise:\n",
                &[
                    format!(
                        "Constitution (+20hp {} -> {})",
                        fighter.base_max_hp,
                        fighter.base_max_hp + 20
                    ),
                    format!(
                        "Strength (+1 damage {} -> {})",
                        fighter.base_power,
                        fighter.base_power + 1
                    ),
                    format!(
                        "Agility (+1 defense {} -> {})",
                        fighter.base_defense,
                        fighter.base_defense + 1
                    ),
                ],
                LEVEL_SCREEN_WIDTH,
                &mut tcod.root,
            );
        }
        fighter.xp -= level_up_xp;
        match choice.unwrap() {
            0 => {
                fighter.base_max_hp += 20;
                fighter.hp += 20;
            }
            1 => {
                fighter.base_power += 1;
            }
            2 => {
                fighter.base_defense += 1;
            }
            _ => {
                unreachable!();
            }
        }
    }
}

fn handle_keys(tcod: &mut Tcod, mut game: &mut Game, objects: &mut Vec<Object>) -> PlayerAction {
    use tcod::input::KeyCode::*;

    let player_alive = objects[PLAYER].alive;
    return match (tcod.key, tcod.key.text(), player_alive) {
        (
            Key {
                code: Enter,
                alt: true,
                ..
            },
            _,
            _,
        ) => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            PlayerAction::DidntTakeTurn
        }
        (Key { code: Escape, .. }, _, _) => PlayerAction::Exit,

        (Key { code: Up, .. }, _, true) | (Key { code: NumPad8, .. }, _, true) => {
            player_move_attack(0, -1, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Down, .. }, _, true) | (Key { code: NumPad2, .. }, _, true) => {
            player_move_attack(0, 1, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Left, .. }, _, true) | (Key { code: NumPad4, .. }, _, true) => {
            player_move_attack(-1, 0, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Right, .. }, _, true) | (Key { code: NumPad6, .. }, _, true) => {
            player_move_attack(1, 0, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: NumPad9, .. }, _, true) => {
            player_move_attack(1, -1, game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: NumPad7, .. }, _, true) => {
            player_move_attack(-1, -1, game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: NumPad1, .. }, _, true) => {
            player_move_attack(-1, 1, game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: NumPad3, .. }, _, true) => {
            player_move_attack(1, 1, game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: NumPad5, .. }, _, true) => {
            game.messages.add("You rest...", VIOLET);
            objects[PLAYER].heal(1, game);
            PlayerAction::TookTurn
        }
        (Key { code: Text, .. }, "g", true) => {
            // Look for an item under the player
            let item = objects
                .iter()
                .position(|o| o.pos() == objects[PLAYER].pos() && o.item.is_some());
            if let Some(id) = item {
                pick_item(id, game, objects);
            }
            PlayerAction::TookTurn
        }
        (Key { code: Text, .. }, "i", true) => {
            let chosen_item_id = inventory_menu(
                &game.inventory as &[Object],
                "Press the key to apply the item\n",
                &mut tcod.root,
            );
            if let Some(inventory_index) = chosen_item_id {
                use_item(inventory_index, tcod, game, objects);
            }
            PlayerAction::TookTurn
        }
        (Key { code: Text, .. }, "d", true) => {
            let chosen_item_id = inventory_menu(
                &game.inventory as &[Object],
                "Press the key to drop the item\n",
                &mut tcod.root,
            );
            if let Some(inventory_index) = chosen_item_id {
                drop_item(inventory_index, game, objects);
            }
            PlayerAction::TookTurn
        }
        (Key { code: Text, .. }, "c", true) => {
            // Show character information
            let player = &objects[PLAYER];

            character_info_box(player, game, &mut tcod.root);

            PlayerAction::DidntTakeTurn
        }
        (Key { code: Text, .. }, ">", true) => {
            // Go down stairs, if the player is on them
            let on_stairs = objects
                .iter()
                .any(|object| object.pos() == objects[PLAYER].pos() && object.name == "stairs");
            if on_stairs {
                next_level(tcod, game, objects);
            }
            PlayerAction::TookTurn
        }

        _ => PlayerAction::DidntTakeTurn,
    };
}
