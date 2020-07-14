// That's about the behavior of the entities
pub struct Ai {
    pub entity: usize,
    pub ai_type: AiType,
}

#[derive(Debug, PartialEq, Clone)]
pub enum AiType {
    Goblin,
}
