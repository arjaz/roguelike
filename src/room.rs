use std::cmp;

use tcod::colors::*;

use rand::distributions::{IndependentSample, Weighted, WeightedChoice};
use rand::Rng;

use crate::ai::Ai;
use crate::equipment::{Equipment, Slot};
use crate::fighter::{DeathCallback, Fighter};
use crate::game::{
    from_dungeon_level, is_blocked, Map, Tile, Transition, MAP_HEIGHT, MAP_WIDTH, PLAYER,
};
use crate::item::Item;
use crate::object::Object;

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

// A rectangular object to represent a room
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
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

pub fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..cmp::max(x1, x2) + 1 {
        map[x as usize][y as usize] = Tile::empty();
    }
}

pub fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..cmp::max(y1, y2) + 1 {
        map[x as usize][y as usize] = Tile::empty();
    }
}

pub fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

// TODO: rewrite that shit completely
pub fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>, level: u32) {
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
                    let mut goblin = Object::new(x, y, 'g', "goblin", DESATURATED_GREEN, true);

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
                    let mut orc = Object::new(x, y, 'o', "orc", DARKER_GREEN, true);

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

pub fn make_map(objects: &mut Vec<Object>, level: u32) -> Map {
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
