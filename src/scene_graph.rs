pub struct Node {}

pub struct Edge {}

pub struct SceneGraph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl SceneGraph {
    pub fn create() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }

    pub fn traverse(&self) {}

    pub fn destroy(self) {}
}
