use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

pub struct SceneNode {
    pub id: NodeId,
    pub name: String,
    pub visible: bool,
    pub children: Vec<NodeId>,
    pub transform: [f32; 16],
}

pub struct SceneGraph {
    nodes: HashMap<NodeId, SceneNode>,
    root: NodeId,
    next_id: usize,
}

impl SceneGraph {
    pub fn new() -> Self {
        let root = NodeId(0);
        let mut nodes = HashMap::new();
        nodes.insert(
            root,
            SceneNode {
                id: root,
                name: "root".into(),
                visible: true,
                children: Vec::new(),
                transform: [
                    1.0, 0.0, 0.0, 0.0,
                    0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                    0.0, 0.0, 0.0, 1.0,
                ],
            },
        );
        Self {
            nodes,
            root,
            next_id: 1,
        }
    }

    pub fn add_child(&mut self, parent: NodeId, name: &str) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;

        let node = SceneNode {
            id,
            name: name.into(),
            visible: true,
            children: Vec::new(),
            transform: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
        };
        self.nodes.insert(id, node);

        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.children.push(id);
        }

        id
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn get(&self, id: NodeId) -> Option<&SceneNode> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        self.nodes.get_mut(&id)
    }
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
    }
}
