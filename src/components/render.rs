use tcod::colors::*;
use tcod::console::*;

use crate::components::position::Position;

// A component responsible for displaying an entity
#[derive(Debug)]
pub struct Render {
    entity: usize,
    pub char: char,
    pub color: Color,
}

impl crate::components::Component for Render {
    fn get_entity(&self) -> usize {
        self.entity
    }
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
