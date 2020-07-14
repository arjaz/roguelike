use serde::{Deserialize, Serialize};

use crate::map::Map;

// Position related properties adn functions
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub entity: usize,
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(entity: usize, x: i32, y: i32) -> Self {
        Position { entity, x, y }
    }

    pub fn try_move(&mut self, map: &Map, dx: i32, dy: i32) {
        let (x, y) = (self.x + dx, self.y + dy);
        if !map[x as usize][y as usize].blocked {
            self.set_pos(x, y);
        }
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    // Get distance to another object
    pub fn distance(&self, other: &Position) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        // ((x1 - x2) ^ 2 + (y1 - y2) ^ 2) ^ (1/2)
        ((dx * dx + dy * dy) as f32).sqrt()
    }
}
