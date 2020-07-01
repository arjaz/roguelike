use std::cmp;

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

const INVENTORY_WIDTH: i32 = 50;

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

const MAX_ROOM_MONSTERS: i32 = 3;
const MAX_ROOM_ITEMS: i32 = 2;

const INVENTORY_SIZE: i32 = 26;

const HEAL_AMOUNT: i32 = 10;
const LIGHTNING_DAMAGE: i32 = 30;
const SPELL_RANGE: i32 = 10;
const CONFUSION_DURATION: i32 = 5;

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
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
    item: Option<Item>,
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
            ai: None,
            item: None,
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
    pub fn take_damage(&mut self, damage: i32, game: &mut Game) {
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, game);
            }
        }
    }

    pub fn attack(&self, target: &mut Object, game: &mut Game) {
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);

        if damage > 0 {
            game.messages.add(
                format!("{} gets {} damage from {}", target.name, damage, self.name),
                RED,
            );
            target.take_damage(damage, game);
        } else {
            game.messages.add(
                format!("{} failed to scratch {}", self.name, target.name),
                RED,
            );
        }
    }

    pub fn heal(&mut self, amount: i32) {
        if let Some(ref mut fighter) = self.fighter {
            fighter.hp += amount;
            if fighter.hp > fighter.max_hp {
                fighter.hp = fighter.max_hp;
            }
        }
    }
}

// combat-related properties and functions
#[derive(Debug, Clone, PartialEq, Copy)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback,
}

// Action to perform on fighter's death
#[derive(Clone, Copy, Debug, PartialEq)]
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
}

// Messages log
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
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Heal,
    Lightning,
    Confusion,
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, Copy)]
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

struct Game {
    map: Map,
    messages: Messages,
    inventory: Vec<Object>,
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

        (Key { code: Up, .. }, _, true) => {
            player_move_attack(0, -1, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Down, .. }, _, true) => {
            player_move_attack(0, 1, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Left, .. }, _, true) => {
            player_move_attack(-1, 0, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Right, .. }, _, true) => {
            player_move_attack(1, 0, &mut game, objects);
            PlayerAction::TookTurn
        }
        (Key { code: Text, .. }, "g", true) => {
            // Look for an item under tha player
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

        _ => PlayerAction::DidntTakeTurn,
    };
}

enum UseResult {
    UsedUp,
    Cancelled,
}

fn use_item(inventory_id: usize, tcod: &mut Tcod, game: &mut Game, objects: &mut [Object]) {
    use Item::*;

    if let Some(item) = &game.inventory[inventory_id].item {
        let on_use = match item {
            Heal => cast_heal,
            Lightning => cast_lightning,
            Confusion => cast_confusion,
        };
        match on_use(inventory_id, tcod, game, objects) {
            UseResult::UsedUp => {
                // Destroy the used item
                game.inventory.remove(inventory_id);
            }
            UseResult::Cancelled => {
                game.messages.add("Cancelled", WHITE);
            }
        }
    } else {
        game.messages.add(
            format!("{} cannot be used", game.inventory[inventory_id].name,),
            WHITE,
        );
    }
}

fn cast_heal(
    _inventory_id: usize,
    _tcod: &mut Tcod,
    game: &mut Game,
    objects: &mut [Object],
) -> UseResult {
    if let Some(fighter) = objects[PLAYER].fighter {
        if fighter.hp == fighter.max_hp {
            game.messages.add("HP is already full", WHITE);
            return UseResult::Cancelled;
        } else {
            game.messages.add("Your wounds heal", LIGHT_VIOLET);
            objects[PLAYER].heal(HEAL_AMOUNT);
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
        objects[monster_id].take_damage(LIGHTNING_DAMAGE, game);
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
    let monster_id = closest_monster(tcod, objects, SPELL_RANGE);
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

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    // Random number of monsters in a room
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        // Random spot
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, &map, &objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                // Goblin
                let mut goblin = Object::new(x, y, 'g', "goblin", colors::DESATURATED_GREEN, true);

                goblin.fighter = Some(Fighter {
                    max_hp: 10,
                    hp: 10,
                    defense: 0,
                    power: 3,
                    on_death: DeathCallback::Monster,
                });
                goblin.ai = Some(Ai::Basic);

                goblin
            } else {
                // Orc
                let mut orc = Object::new(x, y, 'o', "orc", colors::DARKER_GREEN, true);

                orc.fighter = Some(Fighter {
                    max_hp: 15,
                    hp: 15,
                    defense: 1,
                    power: 5,
                    on_death: DeathCallback::Monster,
                });
                orc.ai = Some(Ai::Basic);

                orc
            };
            monster.alive = true;
            objects.push(monster);
        }
    }

    // Random number of iterms in a room
    let num_items = rand::thread_rng().gen_range(0, MAX_ROOM_ITEMS + 1);

    for _ in 0..num_items {
        // Random spot
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        // Place if there is some space
        if !is_blocked(x, y, map, objects) {
            let dice = rand::random::<f32>();
            let item = if dice < 0.7 {
                // Place healing potion
                let mut potion = Object::new(x, y, '!', "healing potion", VIOLET, false);
                potion.item = Some(Item::Heal);
                potion
            } else if dice < 0.7 + 0.1 {
                // Place lighting scroll
                let mut scroll =
                    Object::new(x, y, '#', "scroll of lighting bolt", LIGHT_YELLOW, false);
                scroll.item = Some(Item::Lightning);
                scroll
            } else {
                // Place confusion scroll
                let mut scroll = Object::new(x, y, '#', "scroll of confusion", LIGHT_YELLOW, false);
                scroll.item = Some(Item::Confusion);
                scroll
            };
            objects.push(item);
        }
    }
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

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
            place_objects(new_room, &map, objects);

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
    let header_height = root.get_height_rect(0, 0, width, SCREEN_HEIGHT, header);
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
        inventory.iter().map(|item| item.name.clone()).collect()
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
        .filter(|o| tcod.fov.is_in_fov(o.x, o.y))
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
    let max_hp = objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar(
        &mut tcod.panel,
        1,
        1,
        BAR_WIDTH,
        "HP",
        hp,
        max_hp,
        LIGHT_RED,
        DARKER_RED,
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

fn main() {
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("RustySteel")
        .init();

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
        key: Default::default(),
        mouse: Default::default(),
    };

    tcod::system::set_fps(FPS_LIMIT);

    let mut player = Object::new(0, 0, '@', "player", WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {
        max_hp: 100,
        hp: 100,
        defense: 0,
        power: 25,
        on_death: DeathCallback::Player,
    });

    let mut objects = vec![player];
    let mut game = Game {
        map: make_map(&mut objects),
        messages: Messages::new(),
        inventory: vec![],
    };
    game.messages
        .add("Welcome to the always dark world of rust and steel", RED);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_sight,
                !game.map[x as usize][y as usize].blocked,
            )
        }
    }

    let mut previous_player_position = (-1, -1);

    // Main game loop
    while !tcod.root.window_closed() {
        tcod.root.clear();
        tcod.con.clear();

        let fov_recompute = previous_player_position != objects[PLAYER].pos();

        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => tcod.mouse = m,
            Some((_, Event::Key(k))) => tcod.key = k,
            _ => tcod.key = Default::default(),
        }

        render_all(&mut tcod, &mut game, &objects, fov_recompute);

        tcod.root.flush();

        let player = &mut objects[PLAYER];
        previous_player_position = (player.x, player.y);
        let player_action = handle_keys(&mut tcod, &mut game, &mut objects);
        if player_action == PlayerAction::Exit {
            break;
        }
        if objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, &tcod, &mut game, &mut objects);
                }
            }
        }
    }
}
