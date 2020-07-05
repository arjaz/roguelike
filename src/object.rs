use serde::{Deserialize, Serialize};

use tcod::colors::*;
use tcod::console::*;

use crate::ai::Ai;
use crate::equipment::Equipment;
use crate::fighter::Fighter;
use crate::game::{is_blocked, mut_two, Game, Map, Messages, PLAYER};
use crate::item::Item;
use crate::render::Tcod;

// A generic object inside the game
#[derive(Debug, Serialize, Deserialize)]
pub struct Object {
    pub x: i32,
    pub y: i32,
    pub char: char,
    pub color: Color,
    pub name: String,
    pub blocks: bool,
    pub alive: bool,
    pub fighter: Option<Fighter>,
    pub equipment: Option<Equipment>,
    pub ai: Option<Ai>,
    pub item: Option<Item>,
    pub always_visible: bool,
    pub level: i32,
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

pub fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    // Get position of object
    let (x, y) = objects[id].pos();

    // Chech if the tile is blocked and move the object accordingly
    if !is_blocked(x + dx, y + dy, &map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

pub fn player_move_attack(dx: i32, dy: i32, game: &mut Game, objects: &mut [Object]) {
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

pub fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
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

pub fn closest_monster(tcod: &Tcod, objects: &[Object], range: i32) -> Option<usize> {
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
