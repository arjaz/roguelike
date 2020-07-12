use serde::{Deserialize, Serialize};

use tcod::colors::*;
use tcod::console::*;

use crate::components::position::Position;

// A component responsible for displaying an entity
#[derive(Debug, Serialize, Deserialize)]
pub struct Render {
    pub entity: usize,
    pub char: char,
    pub color: Color,
}

impl Render {
    pub fn new(entity: usize, char: char, color: Color) -> Self {
        Render {
            entity,
            char,
            color,
        }
    }

    pub fn draw(&self, console: &mut dyn Console, position: &Position) {
        console.set_default_foreground(self.color);
        console.put_char(position.x, position.y, self.char, BackgroundFlag::None);
    }
}
