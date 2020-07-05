use serde::{Deserialize, Serialize};

use tcod::colors::*;

use crate::ai::Ai;
use crate::equipment::Slot;
use crate::game::{target_monster, target_tile, Game, PLAYER};
use crate::object::{closest_monster, Object};

use crate::render::Tcod;

pub const INVENTORY_SIZE: i32 = 26;

const HEAL_AMOUNT: i32 = 10;
const LIGHTNING_DAMAGE: i32 = 30;
const FIRE_DAMAGE: i32 = 15;
const SPELL_RANGE: i32 = 10;
const CONFUSION_DURATION: i32 = 5;

// Item properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Heal,
    Lightning,
    Fireball,
    Confusion,
    Sword,
    Shield,
}

// Enum to represent the outcome of the item being used
enum UseResult {
    UsedUp,
    Cancelled,
    UsedAndKept,
}

// Pick up an item to the inventory
pub fn pick_item(object_id: usize, game: &mut Game, objects: &mut Vec<Object>) {
    if game.inventory.len() >= INVENTORY_SIZE as usize {
        game.messages.add("Your inventory is full", DARK_RED);
    } else {
        let item = objects.swap_remove(object_id);
        game.messages
            .add(format!("You picked up an item: {}", item.name), LIGHT_GREY);
        game.inventory.push(item);
    }
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

pub fn use_item(inventory_id: usize, tcod: &mut Tcod, game: &mut Game, objects: &mut [Object]) {
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

pub fn drop_item(inventory_id: usize, game: &mut Game, objects: &mut Vec<Object>) {
    let mut item = game.inventory.remove(inventory_id);
    if item.equipment.is_some() {
        item.dequip(&mut game.messages);
    }
    item.set_pos(objects[PLAYER].x, objects[PLAYER].y);
    game.messages
        .add(format!("Yout dropped {}", item.name), LIGHT_GREY);
    objects.push(item);
}
