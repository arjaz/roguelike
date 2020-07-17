// That's about the behavior of the entities
#[derive(Debug)]
pub struct Ai {
    entity: usize,
    pub ai_type: AiType,
}

#[derive(Debug, PartialEq, Clone)]
pub enum AiType {
    Goblin,
}

impl crate::components::Component for Ai {
    fn get_entity(&self) -> usize {
        self.entity
    }
}

impl Ai {
    pub fn goblin(entity: usize) -> Self {
        Ai {
            entity,
            ai_type: AiType::Goblin,
        }
    }
}
