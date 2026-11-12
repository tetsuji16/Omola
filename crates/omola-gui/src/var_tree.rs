//! Variable names as a tree (G09).
//!
//! `getModelInstance`/the MAT file give flat names (`inertia1.phi`, `der(inertia1.w)`,
//! `x[1]`). Dymola's Variable Browser nests them, so the same names become a tree here:
//! dots are hierarchy, a derivative hangs under the signal it differentiates, and an
//! array element hangs under the array.

/// One node of the variable tree.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VarNode {
    /// Text shown for this node (`inertia1`, `der(w)`, `[1]`).
    pub label: String,
    /// Index into the result's variable list; `Some` for a variable, `None` for a group.
    pub leaf: Option<usize>,
    pub children: Vec<VarNode>,
}

impl VarNode {
    fn child(&mut self, label: &str) -> &mut VarNode {
        if let Some(i) = self.children.iter().position(|c| c.label == label) {
            return &mut self.children[i];
        }
        self.children.push(VarNode {
            label: label.to_owned(),
            ..VarNode::default()
        });
        self.children.last_mut().expect("just pushed")
    }

    /// True when the node only groups other nodes.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_group(&self) -> bool {
        self.leaf.is_none()
    }

    /// Total number of variables below this node.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn leaves(&self) -> usize {
        usize::from(self.leaf.is_some()) + self.children.iter().map(VarNode::leaves).sum::<usize>()
    }
}

/// Split a name into its tree path.
///
/// - `a.b.c` -> `["a", "b", "c"]`
/// - `der(a.b.w)` -> `["a", "b", "der(w)"]`
/// - `x[1]` -> `["x", "[1]"]`
/// - `der(x[1])` -> `["x", "der(x[1])"]` (the element keeps its own node)
/// - `a.b[2].c` -> `["a", "b[2]", "c"]` (only a trailing index becomes its own node)
fn path(name: &str) -> Vec<String> {
    if let Some(inner) = name.strip_prefix("der(").and_then(|n| n.strip_suffix(')')) {
        let mut segments: Vec<String> = inner.split('.').map(str::to_owned).collect();
        let last = segments.pop().unwrap_or_default();
        let (base, index) = split_index(&last);
        if index.is_some() {
            // `der(x[1])` hangs under the array element's array, as `x > der(x[1])`.
            segments.push(base);
            segments.push(format!("der({last})"));
        } else {
            segments.push(format!("der({base})"));
        }
        return segments;
    }
    let mut segments: Vec<String> = name.split('.').map(str::to_owned).collect();
    if let Some(last) = segments.pop() {
        let (base, index) = split_index(&last);
        segments.push(base);
        if let Some(index) = index {
            segments.push(index);
        }
    }
    segments
}

/// `x[1]` -> (`x`, Some(`[1]`)); `x` -> (`x`, None).
fn split_index(segment: &str) -> (String, Option<String>) {
    match segment.rfind('[') {
        Some(i) if segment.ends_with(']') && i > 0 => {
            (segment[..i].to_owned(), Some(segment[i..].to_owned()))
        }
        _ => (segment.to_owned(), None),
    }
}

/// Build the tree of `(variable index, name)` in the given order.
pub fn build_variable_tree(names: &[(usize, &str)]) -> VarNode {
    let mut root = VarNode::default();
    for (index, name) in names {
        let mut node = &mut root;
        for segment in path(name) {
            node = node.child(&segment);
        }
        node.leaf = Some(*index);
    }
    root
}

/// Flat `(depth, node)` list of a tree, for rendering without recursion.
#[cfg_attr(not(test), allow(dead_code))]
pub fn walk<'a>(node: &'a VarNode, depth: usize, out: &mut Vec<(usize, &'a VarNode)>) {
    for child in &node.children {
        out.push((depth, child));
        walk(child, depth + 1, out);
    }
}

/// Whether a node is drawn expanded before the user touches it: a top-level group
/// with at most 20 children. Deeper nodes start closed so a component hierarchy does
/// not unfold into hundreds of rows.
pub fn starts_open(depth: usize, child_count: usize) -> bool {
    depth == 0 && (1..=20).contains(&child_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[(usize, &'static str)]) -> Vec<String> {
        let tree = build_variable_tree(list);
        let mut flat = Vec::new();
        walk(&tree, 0, &mut flat);
        flat.iter()
            .map(|(depth, node)| format!("{}{}", "  ".repeat(*depth), node.label))
            .collect()
    }

    #[test]
    fn dots_become_nesting() {
        assert_eq!(
            names(&[(0, "a.b"), (1, "a.c")]),
            ["a", "  b", "  c"],
            "two children under one parent"
        );
        let tree = build_variable_tree(&[(0, "a.b"), (1, "a.c")]);
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].children.len(), 2);
        assert_eq!(tree.children[0].children[0].leaf, Some(0));
        assert_eq!(tree.children[0].children[1].leaf, Some(1));
    }

    #[test]
    fn derivatives_hang_under_their_signal() {
        assert_eq!(
            names(&[(0, "a.b.x"), (1, "der(a.b.x)")]),
            ["a", "  b", "    x", "    der(x)"]
        );
        // The derivative of an array element keeps the element name.
        assert_eq!(
            names(&[(0, "x[1]"), (1, "der(x[1])")]),
            ["x", "  [1]", "  der(x[1])"]
        );
    }

    #[test]
    fn array_elements_hang_under_the_array() {
        assert_eq!(names(&[(0, "x[1]"), (1, "x[2]")]), ["x", "  [1]", "  [2]"]);
        // A middle index stays part of its segment.
        assert_eq!(names(&[(0, "a.b[2].c")]), ["a", "  b[2]", "    c"]);
    }

    #[test]
    fn a_name_can_be_a_variable_and_a_group() {
        let tree = build_variable_tree(&[(0, "a"), (1, "a.b")]);
        let a = &tree.children[0];
        assert_eq!(a.label, "a");
        assert_eq!(a.leaf, Some(0));
        assert!(!a.is_group());
        assert_eq!(a.children.len(), 1);
        assert_eq!(a.leaves(), 2);
    }

    #[test]
    fn declaration_order_is_kept_and_duplicates_merge() {
        assert_eq!(
            names(&[(0, "z"), (1, "a"), (2, "z.y"), (3, "a.y")]),
            ["z", "  y", "a", "  y"]
        );
    }

    #[test]
    fn top_level_groups_start_open_when_they_are_small() {
        assert!(starts_open(0, 3), "a small top-level group is open");
        assert!(starts_open(0, 20), "20 children is still small enough");
        assert!(!starts_open(0, 21), "more than 20 stays closed");
        assert!(
            !starts_open(0, 0),
            "a node without children has no expander"
        );
        assert!(!starts_open(1, 3), "nested groups stay closed");
    }

    #[test]
    fn odd_names_do_not_panic() {
        assert_eq!(names(&[(0, "")]), [""]);
        assert_eq!(names(&[(0, "der(")]), ["der("]);
        assert_eq!(names(&[(0, "a..b")]), ["a", "  ", "    b"]);
        assert_eq!(names(&[(0, "[1]")]), ["[1]"]);
    }
}
