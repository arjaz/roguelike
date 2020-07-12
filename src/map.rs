use rand::Rng;
use std::cmp;

pub const MAP_WIDTH: i32 = 80;
pub const MAP_HEIGHT: i32 = 43;

#[derive(Debug, Clone)]
pub struct Tile {
    pub blocked: bool,
    pub block_sight: bool,
    pub explored: bool,
}

impl Tile {
    pub fn wall() -> Tile {
        Tile {
            blocked: true,
            block_sight: true,
            explored: false,
        }
    }

    pub fn empty() -> Tile {
        Tile {
            blocked: false,
            block_sight: false,
            explored: false,
        }
    }
}

// Alias for the Map type
pub type Map = Vec<Vec<Tile>>;

// An object to represent a room
#[derive(Debug, Clone, Copy)]
pub struct Room {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Room {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Room {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y + height,
        }
    }

    pub fn intersects(&self, other: &Room) -> bool {
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }

    pub fn center(&self) -> (i32, i32) {
        ((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
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

fn create_room(room: &Room, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

pub fn make_map(
    player_id: usize,
    position_components: &mut Vec<crate::components::position::Position>,
) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // Random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);

        // Random position of the room with regards to the boundaries
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        // Create a room
        let new_room = Room::new(x, y, w, h);

        // Check intersections with other rooms
        let failed = rooms.iter().any(|room| new_room.intersects(room));

        if !failed {
            create_room(&new_room, &mut map);

            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                // TODO: replace this search with a function
                // Moreover, that's O(n), I need to research a bit on how to implement this
                if let Some(player_position) = position_components
                    .iter_mut()
                    .find(|c| c.entity == player_id)
                {
                    player_position.set_pos(new_x, new_y);
                }
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
