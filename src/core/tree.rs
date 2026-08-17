use super::GenerationalId;

/// Compact tree topology stored alongside an arena node.
///
/// No relation owns another node; all links retain generation validation when
/// resolved through the corresponding arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeLinks<I> {
    parent: Option<I>,
    first_child: Option<I>,
    next_sibling: Option<I>,
}

impl<I: Copy> TreeLinks<I> {
    pub const fn new() -> Self {
        Self {
            parent: None,
            first_child: None,
            next_sibling: None,
        }
    }

    pub const fn parent(self) -> Option<I> {
        self.parent
    }

    pub const fn first_child(self) -> Option<I> {
        self.first_child
    }

    pub const fn next_sibling(self) -> Option<I> {
        self.next_sibling
    }

    pub fn set_parent(&mut self, parent: Option<I>) {
        self.parent = parent;
    }

    pub fn set_first_child(&mut self, child: Option<I>) {
        self.first_child = child;
    }

    pub fn set_next_sibling(&mut self, sibling: Option<I>) {
        self.next_sibling = sibling;
    }

    pub fn detach(&mut self) {
        self.parent = None;
        self.next_sibling = None;
    }
}

impl<I: Copy> Default for TreeLinks<I> {
    fn default() -> Self {
        Self::new()
    }
}

/// AoS node shape used by synthetic tests and as the P1 element-tree base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode<T, I: GenerationalId> {
    pub value: T,
    pub links: TreeLinks<I>,
}

impl<T, I: GenerationalId> TreeNode<T, I> {
    pub const fn new(value: T) -> Self {
        Self {
            value,
            links: TreeLinks::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ElementId;

    #[test]
    fn relationships_are_ids_not_owned_allocations() {
        let parent = ElementId::from_parts(1, 2);
        let child = ElementId::from_parts(8, 4);
        let sibling = ElementId::from_parts(9, 1);
        let mut links = TreeLinks::new();
        links.set_parent(Some(parent));
        links.set_first_child(Some(child));
        links.set_next_sibling(Some(sibling));

        assert_eq!(links.parent(), Some(parent));
        assert_eq!(links.first_child(), Some(child));
        assert_eq!(links.next_sibling(), Some(sibling));
    }
}
