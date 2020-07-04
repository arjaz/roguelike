use std::cmp;

use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use rand::distributions::{IndependentSample, Weighted, WeightedChoice};
use rand::Rng;

use tcod::colors;
use tcod::colors::*;
use tcod::console::*;
use tcod::input::{self, Event, Key, Mouse};
use tcod::map::{FovAlgorithm, Map as FovMap};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;

const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

const CHARACTER_SCREEN_WIDTH: i32 = 50;
const LEVEL_SCREEN_WIDTH: i32 = 50;

const INVENTORY_WIDTH: i32 = 40;

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

const INVENTORY_SIZE: i32 = 26;

const HEAL_AMOUNT: i32 = 10;
const LIGHTNING_DAMAGE: i32 = 30;
const FIRE_DAMAGE: i32 = 15;
const SPELL_RANGE: i32 = 10;
const CONFUSION_DURATION: i32 = 5;

const LEVEL_UP_BASE: i32 = 100;
const LEVEL_UP_FACTOR: i32 = 150;

const COLOR_LIGHT_WALL: Color = Color {
    r: 130,
    g: 110,
    b: 150,
};
const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_GROUND: Color = Color {
    r: 200,
    g: 180,
    b: 150,
};
const COLOR_DARK_GROUND: Color = Color {
    r: 50,
    g: 50,
    b: 150,
};

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const PLAYER: usize = 0;

const FPS_LIMIT: i32 = 60;

// A generic object inside the game
#[derive(Debug, Serialize, Deserialize)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    equipment: Option<Equipment>,
    ai: Option<Ai>,
    item: Option<Item>,
    always_visible: bool,
    level: i32,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Self {
        Object {
            x: x,
            y: y,
            char: char,
            color: color,
            name: name.into(),
            blocks: blocks,
            alive: false,
            fighter: None,
            equipment: None,
            ai: None,
            item: None,
            always_visible: false,
            level: 1,
        }
    }

    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    // Get distance to another object
    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx * dx + dy * dy) as f32).sqrt()
    }

    // Get damage
    pub fn take_damage(&mut self, damage: i32, game: &mut Game) -> Option<i32> {
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, game);
                return Some(fighter.xp);
            }
        }
        None
    }

    pub fn attack(&mut self, target: &mut Object, game: &mut Game) {
        let damage = self.power(game) - target.defense(game);

        if damage > 0 {
            game.messages.add(
                format!("{} gets {} damage from {}", target.name, damage, self.name),
                RED,
            );
            if let Some(xp) = target.take_damage(damage, game) {
                // Reward killer with experience
                self.fighter.as_mut().unwrap().xp += xp;
            }
        } else {
            game.messages.add(
                format!("{} failed to scratch {}", self.name, target.name),
                RED,
            );
        }
    }

    pub fn heal(&mut self, amount: i32, game: &mut Game) {
        let max_hp = self.max_hp(game);
        if let Some(ref mut fighter) = self.fighter {
            fighter.hp += amount;
            if fighter.hp > max_hp {
                fighter.hp = max_hp;
            }
        }
    }

    pub fn max_hp(&self, game: &mut Game) -> i32 {
        let base_max_hp = self.fighter.map_or(0, |f| f.base_max_hp);
        let bonus = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.max_hp_bonus)
            .sum::<i32>();

        base_max_hp + bonus
    }

    pub fn power(&self, game: &mut Game) -> i32 {
        let base_power = self.fighter.map_or(0, |f| f.base_power);
        let bonus = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.power_bonus)
            .sum::<i32>();

        base_power + bonus
    }

    pub fn defense(&self, game: &mut Game) -> i32 {
        let base_defense = self.fighter.map_or(0, |f| f.base_defense);
        let bonus = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.defense_bonus)
            .sum::<i32>();

        base_defense + bonus
    }

    pub fn get_all_equipped(&self, game: &mut Game) -> Vec<Equipment> {
        if self.name == "player" {
            game.inventory
                .iter()
                .filter(|item| item.equipment.map_or(false, |e| e.equipped))
                .map(|item| item.equipment.unwrap())
                .collect()
        } else {
            vec![]
        }
    }

    pub fn equip(&mut self, messages: &mut Messages) {
        if self.item.is_none() {
            messages.add(format!("Can't equip {:?} as it's not an item", self), RED);
            return;
        }
        if let Some(ref mut equipment) = self.equipment {
            if !equipment.equipped {
                equipment.equipped = true;
                messages.add(
                    format!("Equipped {} on {}", self.name, equipment.slot),
                    LIGHT_GREEN,
                );
            }
        } else {
            messages.add(
                format!("Can't equip {:?} as it's not an equipment", self),
                RED,
            );
        }
    }

    pub fn dequip(&mut self, messages: &mut Messages) {
        if self.item.is_none() {
            messages.add(format!("Can't dequip {:?} as it's not an item", self), RED);
        }
        if let Some(ref mut equipment) = self.equipment {
            if equipment.equipped {
                equipment.equipped = false;
                messages.add(
                    format!("Dequipped {} from {}", self.name, equipment.slot),
                    LIGHT_YELLOW,
                );
            }
        } else {
            messages.add(
                format!("Can't dequip {:?} as it's not an equipment", self),
                RED,
            );
        }
    }

    pub fn distance(&self, x: i32, y: i32) -> f32 {
        (((x - self.x).pow(2) + (y - self.y).pow(2)) as f32).sqrt()
    }
}

// Equipment of the character
#[derive(Copy, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Equipment {
    slot: Slot,
    equipped: bool,
    power_bonus: i32,
    defense_bonus: i32,
    max_hp_bonus: i32,
}

// Character slots
#[derive(Copy, Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Slot {
    LeftHand,
    RightHand,
    Head,
}

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Slot::LeftHand => write!(f, "left hand"),
            Slot::RightHand => write!(f, "right hand"),
            Slot::Head => write!(f, "head"),
        }
    }
}

// combat-related properties and functions
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
struct Fighter {
    base_max_hp: i32,
    hp: i32,
    base_defense: i32,
    base_power: i32,
    xp: i32,
    on_death: DeathCallback,
}

// Action to perform on fighter's death
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object, game: &mut Game) {
        use DeathCallback::*;
        let callback: fn(&mut Object, &mut Game) = match self {
            Player => player_death,
            Monster => monster_death,
        };
        callback(object, game);
    }
}

fn player_death(player: &mut Object, game: &mut Game) {
    game.messages.add("Your history ends here", RED);

    player.char = '%';
    player.color = DARK_RED;
}

fn monster_death(monster: &mut Object, game: &mut Game) {
    game.messages.add(format!("{} dies!", monster.name), RED);

    monster.char = '%';
    monster.color = DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
    // monster.always_visible = true;
}

struct Transition {
    level: u32,
    value: u32,
}

fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table
        .iter()
        .rev()
        .find(|transition| transition.level <= level)
        .map_or(0, |transition| transition.value)
}

// Messages log
#[derive(Serialize, Deserialize)]
struct Messages {
    messages: Vec<(String, Color)>,
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

// item properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Item {
    Heal,
    Lightning,
    Fireball,
    Confusion,
    Sword,
    Shield,
}

// Pick up an item to the inventory
fn pick_item(object_id: usize, game: &mut Game, objects: &mut Vec<Object>) {
    if game.inventory.len() >= INVENTORY_SIZE as usize {
        game.messages.add("Your inventory is full", DARK_RED);
    } else {
        let item = objects.swap_remove(object_id);
        game.messages
            .add(format!("You picked up an item: {}", item.name), LIGHT_GREY);
        game.inventory.push(item);
    }
}

// artificial intelligence for npcs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Ai {
    Basic,
    Confused {
        previous_ai: Box<Ai>,
        num_turns: i32,
    },
}

fn mut_two<T>(first: usize, second: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first != second);
    let split_at_index = cmp::max(first, second);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first < second {
        (&mut first_slice[first], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second])
    }
}

fn ai_take_turn(monster_id: usize, tcod: &Tcod, game: &mut Game, objects: &mut [Object]) {
    if let Some(ai) = objects[monster_id].ai.take() {
        let new_ai = match ai {
            Ai::Basic => ai_basic(monster_id, tcod, game, objects),
            Ai::Confused {
                previous_ai,
                num_turns,
            } => ai_confused(monster_id, tcod, game, objects, previous_ai, num_turns),
        };
        objects[monster_id].ai = Some(new_ai);
    }
}

fn ai_basic(monster_id: usize, tcod: &Tcod, game: &mut Game, objects: &mut [Object]) -> Ai {
    let (monster_x, monster_y) = objects[monster_id].pos();

    if tcod.fov.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            // Move towards the player
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, player_x, player_y, &game.map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            // Attack the player if he's alive
            let (monster, player) = mut_two(monster_id, PLAYER, objects);
            monster.attack(player, game);
        }
    }
    Ai::Basic
}

fn ai_confused(
    monster_id: usize,
    _tcod: &Tcod,
    game: &mut Game,
    objects: &mut [Object],
    previous_ai: Box<Ai>,
    num_turns: i32,
) -> Ai {
    if num_turns >= 0 {
        // Move around confused
        move_by(
            monster_id,
            rand::thread_rng().gen_range(-1, 2),
            rand::thread_rng().gen_range(-1, 2),
            &game.map,
            objects,
        );

        Ai::Confused {
            previous_ai: previous_ai,
            num_turns: num_turns - 1,
        }
    } else {
        game.messages.add(
            format!("{} is no longer confused", objects[monster_id].name),
            WHITE,
        );
        *previous_ai
    }
}

fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    // Get position of object
    let (x, y) = objects[id].pos();

    // Chech if the tile is blocked and move the object accordingly
    if !is_blocked(x + dx, y + dy, &map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn player_move_attack(dx: i32, dy: i32, game: &mut Game, objects: &mut [Object]) {
    // Coordinates of the player's direction
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    // Get id of the target
    let target_id = objects
        .iter()
        .position(|object| object.fighter.is_some() && object.pos() == (x, y));

    // Attack if there is a target, move otherwise
    match target_id {
        Some(id) => {
            // Attack the monster
            let (monster, player) = mut_two(id, PLAYER, objects);
            player.attack(monster, game);
        }
        None => {
            move_by(PLAYER, dx, dy, &game.map, objects);
        }
    }
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
    // vector from current object to the target
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;

    let distance = ((dx * dx + dy * dy) as f64).sqrt();

    // normalize distance to length of 1, preserving the direction
    // round and convert to integer
    let dx = (dx as f64 / distance).round() as i32;
    let dy = (dy as f64 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

// A rectangular object to represent a room
#[derive(Debug, Clone, Copy)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y + height,
        }
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }

    pub fn intersect(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }
}

// A tile object
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Tile {
    blocked: bool,
    explored: bool,
    block_sight: bool,
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

type Map = Vec<Vec<Tile>>;

#[derive(Serialize, Deserialize)]
struct Game {
    map: Map,
    messages: Messages,
    inventory: Vec<Object>,
    dungeon_level: u32,
}

struct Tcod {
    root: Root,
    con: Offscreen,
    panel: Offscreen,
    fov: FovMap,
    key: Key,
    mouse: Mouse,
}

fn names_under_mouse(mouse: Mouse, object: &[Object], fov_map: &FovMap) -> String {
    let (x, y) = (mouse.cx as i32, mouse.cy as i32);

    // Create a list with the names of the objects under the mouse's coordinates and in FOV
    let names = object
        .iter()
        .filter(|object| fov_map.is_in_fov(object.x, object.y) && object.pos() == (x, y))
        .map(|object| object.name.clone())
        .collect::<Vec<_>>();

    names.join(", ")
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
            let level = player.level;
            let level_up_xp = LEVEL_UP_BASE + player.level * LEVEL_UP_FACTOR;
            if let Some(fighter) = player.fighter.as_ref() {
                let msg = format!(
                    "Character information:
Level: {}
Experience: {}
Experience to level up: {}
Maximum HP: {}
Attack: {}
Defense: {}",
                    level,
                    fighter.xp,
                    level_up_xp,
                    player.max_hp(game),
                    player.power(game),
                    player.defense(game)
                );
                msgbox(&msg, CHARACTER_SCREEN_WIDTH, &mut tcod.root);
            }

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

enum UseResult {
    UsedUp,
    Cancelled,
    UsedAndKept,
}

fn use_item(inventory_id: usize, tcod: &mut Tcod, game: &mut Game, objects: &mut [Object]) {
    use Item::*;

    if let Some(item) = &game.inventory[inventory_id].item {
        let on_use = match item {
            Heal => cast_heal,
            Lightning => cast_lightning,
            Confusion => cast_confusion,
            Fireball => cast_fireball,
            Sword => toggle_equipment,
            Shield => toggle_equipment,
        };
        match on_use(inventory_id, tcod, game, objects) {
            UseResult::UsedUp => {
                // Destroy the used item
                game.inventory.remove(inventory_id);
            }
            UseResult::Cancelled => {
                game.messages.add("Cancelled", WHITE);
            }
            UseResult::UsedAndKept => {}
        }
    } else {
        game.messages.add(
            format!("{} cannot be used", game.inventory[inventory_id].name,),
            WHITE,
        );
    }
}

fn toggle_equipment(
    inventory_id: usize,
    _tcod: &mut Tcod,
    game: &mut Game,
    _objects: &mut [Object],
) -> UseResult {
    let equipment = match game.inventory[inventory_id].equipment {
        Some(equipment) => equipment,
        None => return UseResult::Cancelled,
    };

    if let Some(current) = get_equipped_in_slot(equipment.slot, &game.inventory) {
        game.inventory[current].dequip(&mut game.messages);
    }

    if equipment.equipped {
        game.inventory[inventory_id].dequip(&mut game.messages);
    } else {
        game.inventory[inventory_id].equip(&mut game.messages);
    }
    UseResult::UsedAndKept
}

fn cast_heal(
    _inventory_id: usize,
    _tcod: &mut Tcod,
    game: &mut Game,
    objects: &mut [Object],
) -> UseResult {
    if let Some(fighter) = objects[PLAYER].fighter {
        if fighter.hp == objects[PLAYER].max_hp(game) {
            game.messages.add("HP is already full", WHITE);
            return UseResult::Cancelled;
        } else {
            game.messages.add("Your wounds heal", LIGHT_VIOLET);
            objects[PLAYER].heal(HEAL_AMOUNT, game);
            return UseResult::UsedUp;
        }
    }
    UseResult::Cancelled
}

fn cast_lightning(
    _inventory_id: usize,
    tcod: &mut Tcod,
    game: &mut Game,
    objects: &mut [Object],
) -> UseResult {
    let monster_id = closest_monster(tcod, objects, SPELL_RANGE);
    if let Some(monster_id) = monster_id {
        game.messages.add(
            format!(
                "A lightning bolt strikes {} for {} damage",
                objects[monster_id].name, LIGHTNING_DAMAGE
            ),
            LIGHT_BLUE,
        );
        if let Some(xp) = objects[monster_id].take_damage(LIGHTNING_DAMAGE, game) {
            objects[PLAYER].fighter.as_mut().unwrap().xp += xp;
        }
        UseResult::UsedUp
    } else {
        game.messages.add("There is no one to strike", WHITE);
        UseResult::Cancelled
    }
}

fn cast_confusion(
    _inventory_id: usize,
    tcod: &mut Tcod,
    game: &mut Game,
    objects: &mut [Object],
) -> UseResult {
    // let monster_id = closest_monster(tcod, objects, SPELL_RANGE);
    game.messages.add("Choose an enemy to confuse", LIGHT_GREY);
    let monster_id = target_monster(tcod, game, objects, Some(SPELL_RANGE as f32));

    if let Some(monster_id) = monster_id {
        game.messages.add(
            format!("{} gets confused", objects[monster_id].name),
            LIGHT_BLUE,
        );
        // Fill fail if no ai found
        let old_ai = objects[monster_id].ai.take().unwrap();
        // let old_ai = objects[monster_id].ai.take().unwrap_or(Ai::Basic);

        objects[monster_id].ai = Some(Ai::Confused {
            previous_ai: Box::new(old_ai),
            num_turns: CONFUSION_DURATION,
        });
        UseResult::UsedUp
    } else {
        game.messages.add("There is no one to confused", WHITE);
        UseResult::Cancelled
    }
}

fn cast_fireball(
    _inventory_id: usize,
    tcod: &mut Tcod,
    game: &mut Game,
    objects: &mut [Object],
) -> UseResult {
    // Ask to choose a tile
    game.messages
        .add("Choose a tile to cast infernal flames to", LIGHT_GREY);
    let (x, y) = match target_tile(tcod, game, objects, None) {
        Some(tile_pos) => tile_pos,
        None => return UseResult::Cancelled,
    };

    game.messages.add(
        "The fireball explodes and burnes everything it can touch",
        ORANGE,
    );

    let mut gained_xp = 0;
    for (id, obj) in objects.iter_mut().enumerate() {
        if obj.distance(x, y) <= (SPELL_RANGE / 2) as f32 && obj.fighter.is_some() {
            game.messages.add(
                format!("{} is burnt by the infernal spell!", obj.name),
                ORANGE,
            );
            if let Some(xp) = obj.take_damage(FIRE_DAMAGE, game) {
                if id != PLAYER {
                    gained_xp += xp;
                }
            }
        }
    }
    objects[PLAYER].fighter.as_mut().unwrap().xp += gained_xp;

    UseResult::UsedUp
}

// Return the position of the clicked tile, or (None, None) if right clicked
fn target_tile(
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
fn target_monster(
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

fn drop_item(inventory_id: usize, game: &mut Game, objects: &mut Vec<Object>) {
    let mut item = game.inventory.remove(inventory_id);
    if item.equipment.is_some() {
        item.dequip(&mut game.messages);
    }
    item.set_pos(objects[PLAYER].x, objects[PLAYER].y);
    game.messages
        .add(format!("Yout dropped {}", item.name), LIGHT_GREY);
    objects.push(item);
}

fn get_equipped_in_slot(slot: Slot, inventory: &[Object]) -> Option<usize> {
    for (inventory_id, item) in inventory.iter().enumerate() {
        if item
            .equipment
            .as_ref()
            .map_or(false, |e| e.equipped && e.slot == slot)
        {
            return Some(inventory_id);
        }
    }
    None
}

fn closest_monster(tcod: &Tcod, objects: &[Object], range: i32) -> Option<usize> {
    let mut closest_enemy = None;
    let mut closest_dist = (range + 1) as f32;

    for (id, object) in objects.iter().enumerate() {
        if (id != PLAYER)
            && object.fighter.is_some()
            && object.ai.is_some()
            && tcod.fov.is_in_fov(object.x, object.y)
        {
            let dist = objects[PLAYER].distance_to(&objects[id]);
            if dist < closest_dist {
                closest_enemy = Some(id);
                closest_dist = dist;
            }
        }
    }
    closest_enemy
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>, level: u32) {
    // maximum number of monsters in a room
    let max_monsters = from_dungeon_level(
        &[
            Transition { level: 1, value: 2 },
            Transition { level: 4, value: 3 },
            Transition { level: 6, value: 5 },
        ],
        level,
    );

    // Random number of monsters in a room
    let num_monsters = rand::thread_rng().gen_range(0, max_monsters + 1);

    let monster_table = &mut [
        Weighted {
            weight: 80,
            item: "goblin",
        },
        Weighted {
            weight: 20,
            item: "orc",
        },
    ];

    let monster_choice = WeightedChoice::new(monster_table);

    for _ in 0..num_monsters {
        // Random spot
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, &map, &objects) {
            let mut monster = match monster_choice.ind_sample(&mut rand::thread_rng()) {
                "goblin" => {
                    let mut goblin =
                        Object::new(x, y, 'g', "goblin", colors::DESATURATED_GREEN, true);

                    goblin.fighter = Some(Fighter {
                        base_max_hp: 10,
                        hp: 10,
                        base_defense: 0,
                        base_power: 3,
                        xp: 25,
                        on_death: DeathCallback::Monster,
                    });
                    goblin.ai = Some(Ai::Basic);

                    goblin
                }

                "orc" => {
                    // Orc
                    let mut orc = Object::new(x, y, 'o', "orc", colors::DARKER_GREEN, true);

                    orc.fighter = Some(Fighter {
                        base_max_hp: 15,
                        hp: 15,
                        base_defense: 1,
                        base_power: 5,
                        xp: 80,
                        on_death: DeathCallback::Monster,
                    });
                    orc.ai = Some(Ai::Basic);

                    orc
                }
                _ => unreachable!(),
            };
            monster.alive = true;
            objects.push(monster);
        }
    }

    // Max number of iterms in a room
    let max_items = from_dungeon_level(
        &[
            Transition { level: 1, value: 1 },
            Transition { level: 4, value: 2 },
        ],
        level,
    );

    // Random number of iterms in a room
    let num_items = rand::thread_rng().gen_range(0, max_items + 1);

    let item_table = &mut [
        Weighted {
            weight: 70,
            item: Item::Heal,
        },
        Weighted {
            weight: 10,
            item: Item::Fireball,
        },
        Weighted {
            weight: 10,
            item: Item::Lightning,
        },
        Weighted {
            weight: 10,
            item: Item::Confusion,
        },
        Weighted {
            weight: from_dungeon_level(&[Transition { level: 4, value: 5 }], level),
            item: Item::Sword,
        },
        Weighted {
            weight: from_dungeon_level(
                &[Transition {
                    level: 8,
                    value: 15,
                }],
                level,
            ),
            item: Item::Shield,
        },
    ];

    let item_choice = WeightedChoice::new(item_table);

    for _ in 0..num_items {
        // Random spot
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        // Place if there is some space
        if !is_blocked(x, y, map, objects) {
            let item = match item_choice.ind_sample(&mut rand::thread_rng()) {
                Item::Heal => {
                    let mut potion = Object::new(x, y, '!', "healing potion", VIOLET, false);
                    potion.item = Some(Item::Heal);
                    potion
                }
                Item::Fireball => {
                    let mut scroll = Object::new(x, y, '#', "fireball scroll", ORANGE, false);
                    scroll.item = Some(Item::Fireball);
                    scroll
                }
                Item::Lightning => {
                    let mut scroll =
                        Object::new(x, y, '#', "lightning scroll", LIGHT_YELLOW, false);
                    scroll.item = Some(Item::Lightning);
                    scroll
                }
                Item::Confusion => {
                    let mut scroll =
                        Object::new(x, y, '#', "confusion scroll", LIGHT_YELLOW, false);
                    scroll.item = Some(Item::Confusion);
                    scroll
                }
                Item::Sword => {
                    let mut sword = Object::new(x, y, '/', "sword", SKY, false);
                    sword.item = Some(Item::Sword);
                    sword.equipment = Some(Equipment {
                        equipped: false,
                        slot: Slot::RightHand,
                        power_bonus: 5,
                        defense_bonus: 0,
                        max_hp_bonus: 0,
                    });
                    sword
                }
                Item::Shield => {
                    let mut shield = Object::new(x, y, '0', "shield", SKY, false);
                    shield.item = Some(Item::Shield);
                    shield.equipment = Some(Equipment {
                        equipped: false,
                        slot: Slot::LeftHand,
                        power_bonus: 0,
                        defense_bonus: 5,
                        max_hp_bonus: 4,
                    });
                    shield
                }
            };
            objects.push(item);
        }
    }
}

fn make_map(objects: &mut Vec<Object>, level: u32) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    // Remove every object except for the player
    assert_eq!(&objects[PLAYER] as *const _, &objects[0] as *const _);
    objects.truncate(1);

    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // Random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);

        // Random position of the room with regards to the boundaries
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        // Check intersections with existing rooms
        let failed = rooms.iter().any(|room| new_room.intersect(room));

        if !failed {
            create_room(new_room, &mut map);
            place_objects(new_room, &map, objects, level);

            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                objects[PLAYER].set_pos(new_x, new_y);
            } else {
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                if rand::random() {
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    // create stairs at the center of the last room
    let (last_room_x, last_room_y) = rooms[rooms.len() - 1].center();
    let mut stairs = Object::new(last_room_x, last_room_y, '>', "stairs", WHITE, false);
    stairs.always_visible = true;
    objects.push(stairs);

    map
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..cmp::max(x1, x2) + 1 {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..cmp::max(y1, y2) + 1 {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

fn menu<T: AsRef<str>>(header: &str, options: &[T], width: i32, root: &mut Root) -> Option<usize> {
    assert!(
        options.len() <= INVENTORY_SIZE as usize,
        "Cannot have such a big menu"
    );

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

    // Print all the options
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

fn inventory_menu(inventory: &[Object], header: &str, root: &mut Root) -> Option<usize> {
    let options = if inventory.len() == 0 {
        vec!["Inventory is empty".into()]
    } else {
        // inventory.iter().map(|item| item.name.clone()).collect()
        inventory
            .iter()
            .map(|item| match item.equipment {
                Some(equipment) if equipment.equipped => {
                    format!("{} (on {})", item.name, equipment.slot)
                }
                _ => item.name.clone(),
            })
            .collect()
    };

    let inventory_index = menu(header, &options, INVENTORY_WIDTH, root);

    // Return the item if it was selected
    if inventory.len() > 0 {
        inventory_index
    } else {
        None
    }
}

fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    bar_color: Color,
    back_color: Color,
) {
    // Get width of the bar (of HP, exp, etc.)
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // Render the background
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // Render the bar
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);
    }

    // Centered text with values
    panel.set_default_foreground(WHITE);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        &format!("{}: {}/{}", name, value, maximum),
    )
}

fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool) {
    if fov_recompute {
        let player = &objects[PLAYER];
        tcod.fov
            .compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;
            let color = match (visible, wall) {
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };
            let explored = &mut game.map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }
            if *explored {
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    // Get objects to draw
    let mut to_draw: Vec<_> = objects
        .iter()
        .filter(|o| {
            tcod.fov.is_in_fov(o.x, o.y)
                || (o.always_visible && game.map[o.x as usize][o.y as usize].explored)
        })
        .collect();

    // Show non-blocking on top
    to_draw.sort_by(|o1, o2| o1.blocks.cmp(&o2.blocks));

    // Draw
    for object in &to_draw {
        object.draw(&mut tcod.con);
    }

    blit(
        &tcod.con,
        (0, 0),
        (MAP_WIDTH, MAP_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );

    tcod.panel.set_default_background(BLACK);
    tcod.panel.clear();

    let hp = objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let base_max_hp = objects[PLAYER].max_hp(game);
    render_bar(
        &mut tcod.panel,
        1,
        1,
        BAR_WIDTH,
        "HP",
        hp,
        base_max_hp,
        LIGHT_RED,
        DARKER_RED,
    );

    // Show current dungeon level
    tcod.panel.print_ex(
        1,
        3,
        BackgroundFlag::None,
        TextAlignment::Left,
        format!("Dungeon level: {}", game.dungeon_level),
    );

    // Display names of objects under the mouse
    tcod.panel.set_default_foreground(LIGHT_GREY);
    tcod.panel.print_ex(
        1,
        0,
        BackgroundFlag::None,
        TextAlignment::Left,
        names_under_mouse(tcod.mouse, objects, &tcod.fov),
    );

    let mut y = MSG_HEIGHT as i32;
    for &(ref msg, color) in game.messages.iter().rev() {
        let msg_height = tcod.panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        y -= msg_height;
        if y < 0 {
            break;
        }
        tcod.panel.set_default_foreground(color);
        tcod.panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
    }

    blit(
        &tcod.panel,
        (0, 0),
        (SCREEN_WIDTH, PANEL_HEIGHT),
        &mut tcod.root,
        (0, PANEL_Y),
        1.0,
        1.0,
    );
}

fn next_level(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) {
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

fn new_game(tcod: &mut Tcod) -> (Game, Vec<Object>) {
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

fn initialize_fov(tcod: &mut Tcod, map: &Map) {
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

fn save_game(game: &Game, objects: &[Object]) -> Result<(), Box<dyn Error>> {
    let save_data = serde_json::to_string(&(game, objects))?;
    let mut file = File::create("savegame")?;
    file.write_all(save_data.as_bytes())?;
    Ok(())
}

fn load_game() -> Result<(Game, Vec<Object>), Box<dyn Error>> {
    let mut json_save_state = String::new();
    let mut file = File::open("savegame")?;
    file.read_to_string(&mut json_save_state)?;
    let result = serde_json::from_str::<(Game, Vec<Object>)>(&json_save_state)?;
    Ok(result)
}

fn play_game(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) {
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

fn msgbox(text: &str, width: i32, root: &mut Root) {
    let options: &[&str] = &[];
    menu(text, options, width, root);
}

fn main_menu(tcod: &mut Tcod) {
    let img = tcod::image::Image::from_file("menu_background.png")
        .ok()
        .expect("Background image not found");

    tcod.root.set_default_foreground(LIGHT_RED);
    tcod.root.print_ex(
        SCREEN_WIDTH / 2,
        SCREEN_HEIGHT / 2 - 4,
        BackgroundFlag::None,
        TextAlignment::Center,
        "World of Rust and Steel",
    );
    tcod.root.print_ex(
        SCREEN_WIDTH / 2,
        SCREEN_HEIGHT / 2 - 2,
        BackgroundFlag::None,
        TextAlignment::Center,
        "By Eugene Rossokha",
    );

    while !tcod.root.window_closed() {
        // Show the image at twice the regular console resolution
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut tcod.root, (0, 0));

        // Show options and waitt for the player to choose
        let choices = &["Play a new game", "Continue", "Quit"];
        let choice = menu("", choices, 24, &mut tcod.root);

        match choice {
            Some(0) => {
                let (mut game, mut objects) = new_game(tcod);
                play_game(tcod, &mut game, &mut objects);
            }
            Some(1) => match load_game() {
                Ok((mut game, mut objects)) => {
                    initialize_fov(tcod, &game.map);
                    play_game(tcod, &mut game, &mut objects);
                }
                Err(_e) => {
                    msgbox("\nNo saved game to load.\n", 24, &mut tcod.root);
                    continue;
                }
            },
            Some(2) => {
                break;
            }
            _ => {}
        }
    }
}

fn main() {
    tcod::system::set_fps(FPS_LIMIT);

    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust and Steel")
        .init();

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
        key: Default::default(),
        mouse: Default::default(),
    };

    main_menu(&mut tcod);
}
