use serde::{Deserialize, Serialize};

// Combat-related properties and functions
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Combat {
    pub entity: usize,
    pub alive: bool,
    base_max_hp: i32,
    pub hp: i32,
    base_defense: i32,
    base_damage: i32,
    pub on_death: Option<DeathCallback>,
}

impl Combat {
    pub fn new(entity: usize, base_max_hp: i32, base_defense: i32, base_damage: i32) -> Self {
        Combat {
            entity,
            alive: false,
            base_max_hp,
            hp: base_max_hp,
            base_defense,
            base_damage,
            on_death: None,
        }
    }

    // Returns damage with all the modifiers applied
    // TODO: modifiers
    pub fn get_damage(&self) -> i32 {
        self.base_damage
    }

    // Returns defense with all the modifiers applied
    // TODO: modifiers
    pub fn get_defense(&self) -> i32 {
        self.base_defense
    }

    // Returns maximum health with all the modifiers applied
    // TODO: modifiers
    pub fn get_max_hp(&self) -> i32 {
        self.base_max_hp
    }

    // Attack another combat component
    pub fn attack(&self, target: &mut Combat) {
        let damage = self.get_damage() - target.get_defense();

        if damage > 0 {
            target.take_damage(damage);
        }
    }

    pub fn get_healed(&mut self, amount: i32) {
        let max_hp = self.get_max_hp();
        self.hp += amount;
        if self.hp > max_hp {
            self.hp = max_hp;
        }
    }

    // Take damage from some source
    pub fn take_damage(&mut self, damage: i32) {
        if damage > 0 {
            self.hp -= damage;
        }

        if self.hp <= 0 {
            self.alive = false;
            match self.on_death {
                Some(callback) => callback.call(self.entity),
                None => return,
            }
        }
    }
}

// Callback called on death of the Entity with Combat component
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn call(self, entity: usize) {
        let callback: fn(usize) = match self {
            DeathCallback::Player => Self::player_death,
            DeathCallback::Monster => Self::monster_death,
        };
        callback(entity);
    }

    // TODO: update the world
    fn player_death(entity: usize) {
        println!("Player ({}) is dead", entity);
    }

    // TODO: update the world
    fn monster_death(entity: usize) {
        println!("Monster ({}) is dead", entity);
    }
}
