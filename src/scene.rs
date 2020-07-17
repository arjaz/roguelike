use crate::map::make_map;

use crate::components::combat::Combat;
use crate::components::fov::Fov;
use crate::components::position::Position;
use crate::components::render::Render;
use crate::components::Component;

pub struct Scene {
    pub map: crate::map::Map,
    pub player_id: usize,
    pub render_components: Vec<Render>,
    pub position_components: Vec<Position>,
    pub combat_components: Vec<Combat>,
    pub fov_components: Vec<Fov>,
}

pub fn find_component<'a, T: Component>(id: usize, collection: &'a [T]) -> Option<&'a T> {
    collection.iter().find(|c| c.get_entity() == id)
}

pub fn find_component_mut<'a, T: Component>(
    id: usize,
    collection: &'a mut [T],
) -> Option<&'a mut T> {
    collection.iter_mut().find(|c| c.get_entity() == id)
}

impl Scene {
    pub fn new(_tcod: &crate::engine::Tcod) -> Scene {
        let player_id = crate::engine::get_unique_id();
        let player_position = crate::components::position::Position::new(player_id, 0, 0);

        let player_max_hp = 100;
        let player_defense = 100;
        let player_damage = 100;
        let player_combat = {
            let mut component = crate::components::combat::Combat::new(
                player_id,
                player_max_hp,
                player_defense,
                player_damage,
            );
            component.alive = true;
            component.on_death = Some(crate::components::combat::DeathCallback::Player);
            component
        };

        let player_render =
            crate::components::render::Render::new(player_id, '@', tcod::colors::WHITE);

        let render_components = vec![player_render];
        let mut position_components = vec![player_position];
        let combat_components = vec![player_combat];

        let map = make_map(player_id, &mut position_components);

        let player_view_radius = 15;
        let player_fov = Fov::new(player_id, player_view_radius, &map);
        let fov_components = vec![player_fov];

        Scene {
            map,
            player_id,
            render_components,
            position_components,
            combat_components,
            fov_components,
        }
    }
}
