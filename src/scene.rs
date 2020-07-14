use crate::map::make_map;

pub struct Scene {
    pub map: crate::map::Map,
    pub player_id: usize,
    pub render_components: Vec<crate::components::render::Render>,
    pub position_components: Vec<crate::components::position::Position>,
    pub combat_components: Vec<crate::components::combat::Combat>,
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

        Scene {
            map: make_map(player_id, &mut position_components),
            player_id,
            render_components,
            position_components,
            combat_components,
        }
    }
}
