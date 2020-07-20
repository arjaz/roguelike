// That component represents one-sided relative of one entity to another
// That also can represent knowledge per se, but I'm not sure
#[derive(Debug)]
pub struct Relationship {
    entity: usize,
    target: usize,
    // from -1 to 1; represents bad-good relationship
    pub level: f64,
}

impl crate::components::Component for Relationship {
    fn get_entity(&self) -> usize {
        self.entity
    }
}

impl Relationship {
    pub fn neutral(entity: usize, target: usize) -> Self {
        Relationship {
            entity,
            target,
            level: 0.0,
        }
    }

    pub fn love(entity: usize, target: usize) -> Self {
        Relationship {
            entity,
            target,
            level: 1.0,
        }
    }

    pub fn hate(entity: usize, target: usize) -> Self {
        Relationship {
            entity,
            target,
            level: -1.0,
        }
    }

    pub fn new(entity: usize, target: usize, level: f64) -> Self {
        Relationship {
            entity,
            target,
            level,
        }
    }
}
