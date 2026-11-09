use std::collections::{BTreeMap, HashSet};

use super::core::names::{self, SEPARATOR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Section { count: usize, expanded: bool },
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub path: String,
    pub depth: usize,
    pub kind: NodeKind,
}

impl TreeRow {
    pub fn label(&self) -> &str {
        names::leaf(&self.path)
    }

    pub fn is_section(&self) -> bool {
        matches!(self.kind, NodeKind::Section { .. })
    }

    pub fn is_expanded(&self) -> bool {
        matches!(self.kind, NodeKind::Section { expanded: true, .. })
    }

    pub fn insert_text(&self) -> String {
        match self.kind {
            NodeKind::Section { .. } => format!("{}{SEPARATOR}", self.path),
            NodeKind::Variable => self.path.clone(),
        }
    }
}

#[derive(Default)]
struct Node {
    sections: BTreeMap<String, Node>,
    variables: Vec<String>,
    count: usize,
}

pub fn build(names: &[String], collapsed: &HashSet<String>, filter: &str) -> Vec<TreeRow> {
    let mut root = Node::default();
    for name in names.iter().filter(|n| n.contains(filter)) {
        let mut node = &mut root;
        let mut start = 0;
        for (i, _) in name.match_indices(SEPARATOR) {
            node = node.sections.entry(name[start..i].to_string()).or_default();
            node.count += 1;
            start = i + 1;
        }
        node.variables.push(name.clone());
    }

    let mut rows = Vec::new();
    emit(&root, "", 0, collapsed, !filter.is_empty(), &mut rows);
    rows
}

fn emit(
    node: &Node,
    prefix: &str,
    depth: usize,
    collapsed: &HashSet<String>,
    expand_all: bool,
    out: &mut Vec<TreeRow>,
) {
    for (segment, child) in &node.sections {
        let path = if prefix.is_empty() {
            segment.clone()
        } else {
            format!("{prefix}{SEPARATOR}{segment}")
        };
        let expanded = expand_all || !collapsed.contains(&path);
        out.push(TreeRow {
            path: path.clone(),
            depth,
            kind: NodeKind::Section {
                count: child.count,
                expanded,
            },
        });
        if expanded {
            emit(child, &path, depth + 1, collapsed, expand_all, out);
        }
    }
    let mut variables = node.variables.clone();
    variables.sort();
    out.extend(variables.into_iter().map(|path| TreeRow {
        path,
        depth,
        kind: NodeKind::Variable,
    }));
}

pub fn find(rows: &[TreeRow], path: &str) -> Option<usize> {
    rows.iter().position(|r| r.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn render(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let indent = "  ".repeat(r.depth);
                match r.kind {
                    NodeKind::Section { count, expanded } => format!(
                        "{indent}{} {} ({count})",
                        if expanded { "v" } else { ">" },
                        r.label()
                    ),
                    NodeKind::Variable => format!("{indent}{}", r.label()),
                }
            })
            .collect()
    }

    #[test]
    fn builds_nested_sections_before_variables() {
        let n = names(&["x", "physics.g", "physics.a1.a.v", "physics.a1.b", "math.x"]);
        let rows = build(&n, &HashSet::new(), "");
        assert_eq!(
            render(&rows),
            vec![
                "v math (1)",
                "  x",
                "v physics (3)",
                "  v a1 (2)",
                "    v a (1)",
                "      v",
                "    b",
                "  g",
                "x",
            ]
        );
        assert_eq!(rows[3].path, "physics.a1");
        assert_eq!(rows[3].insert_text(), "physics.a1.");
        assert_eq!(rows[5].insert_text(), "physics.a1.a.v");
    }

    #[test]
    fn collapsed_sections_hide_their_contents() {
        let n = names(&["physics.g", "physics.a1.b", "x"]);
        let collapsed: HashSet<String> = ["physics.a1".to_string()].into();
        assert_eq!(
            render(&build(&n, &collapsed, "")),
            vec!["v physics (2)", "  > a1 (1)", "  g", "x"]
        );
        let collapsed: HashSet<String> = ["physics".to_string()].into();
        assert_eq!(
            render(&build(&n, &collapsed, "")),
            vec!["> physics (2)", "x"]
        );
    }

    #[test]
    fn filter_expands_and_counts_matches_only() {
        let n = names(&["physics.g", "physics.a1.b", "x"]);
        let collapsed: HashSet<String> = ["physics".to_string()].into();
        assert_eq!(
            render(&build(&n, &collapsed, "b")),
            vec!["v physics (1)", "  v a1 (1)", "    b"]
        );
    }
}
