use serde::{Deserialize, Serialize};

use tcod::colors::*;

use crate::game::Game;
use crate::object::Object;

// combat-related properties and functions
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub struct Fighter {
    pub base_max_hp: i32,
    pub hp: i32,
    pub base_defense: i32,
    pub base_power: i32,
    pub xp: i32,
    pub on_death: DeathCallback,
}

// Action to perform on fighter's death
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    pub fn callback(self, object: &mut Object, game: &mut Game) {
        use DeathCallback::*;
        let callback: fn(&mut Object, &mut Game) = match self {
            Player => player_death,
            Monster => monster_death,
        };
        callback(object, game);
    }
}

pub fn player_death(player: &mut Object, game: &mut Game) {
    game.messages.add("Your history ends here", RED);

    player.char = '%';
    player.color = DARK_RED;
}

pub fn monster_death(monster: &mut Object, game: &mut Game) {
    game.messages.add(format!("{} dies!", monster.name), RED);

    monster.char = '%';
    monster.color = DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}
