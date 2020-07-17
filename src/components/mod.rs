pub mod ai;
pub mod combat;
pub mod fov;
pub mod position;
pub mod render;

pub trait Component {
    fn get_entity(&self) -> usize;
}
