use crate::map::Map;
use tcod::map::Map as FovMap;

// FOV of the entity
pub struct Fov {
    entity: usize,
    pub fov_map: FovMap,
    pub view_radius: i32,
}

impl crate::components::Component for Fov {
    fn get_entity(&self) -> usize {
        self.entity
    }
}

impl Fov {
    pub fn new(entity: usize, view_radius: i32, map: &Map) -> Self {
        let mut fov_map = FovMap::new(map.len() as i32, map[0].len() as i32);
        for x in 0..map.len() {
            for y in 0..map[x].len() {
                fov_map.set(
                    x as i32,
                    y as i32,
                    !map[x][y].block_sight,
                    !map[x][y].blocked,
                );
            }
        }
        Fov {
            entity,
            fov_map,
            view_radius,
        }
    }
}
