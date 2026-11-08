pub const SEPARATOR: char = '.';

pub fn leaf(path: &str) -> &str {
    path.rsplit_once(SEPARATOR).map_or(path, |(_, l)| l)
}

pub fn parent(path: &str) -> Option<&str> {
    path.rsplit_once(SEPARATOR).map(|(p, _)| p)
}

pub fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices(SEPARATOR).map(move |(i, _)| &path[..i])
}

pub fn is_within(name: &str, section: &str) -> bool {
    name.len() > section.len()
        && name.starts_with(section)
        && name[section.len()..].starts_with(SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_helpers() {
        assert_eq!(leaf("physics.a1.g"), "g");
        assert_eq!(leaf("x"), "x");
        assert_eq!(parent("physics.a1.g"), Some("physics.a1"));
        assert_eq!(parent("x"), None);
        assert_eq!(ancestors("a.b.c").collect::<Vec<_>>(), vec!["a", "a.b"]);
    }

    #[test]
    fn containment_respects_segment_boundaries() {
        assert!(is_within("physics.g", "physics"));
        assert!(is_within("physics.a.g", "physics"));
        assert!(!is_within("physics", "physics"));
        assert!(!is_within("physicsx.g", "physics"));
        assert!(!is_within("phys.g", "physics"));
    }
}
