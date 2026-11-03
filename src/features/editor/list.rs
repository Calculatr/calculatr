use std::ops::RangeInclusive;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    Down,
    Left,
    Right,
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListEvent {
    None,
    Insert(RangeInclusive<usize>),
    Yank(RangeInclusive<usize>),
    Delete(RangeInclusive<usize>),
    Leave(Leave),
    StartFilter,
}

pub struct ListSpec<'a> {
    pub len: usize,
    pub selectable: &'a dyn Fn(usize) -> bool,
    pub allow_filter: bool,
}

impl ListSpec<'_> {
    fn ok(&self, i: usize) -> bool {
        i < self.len && (self.selectable)(i)
    }

    pub fn first(&self) -> Option<usize> {
        (0..self.len).find(|&i| self.ok(i))
    }

    pub fn last(&self) -> Option<usize> {
        (0..self.len).rev().find(|&i| self.ok(i))
    }

    fn next(&self, from: usize) -> Option<usize> {
        (from + 1..self.len).find(|&i| self.ok(i))
    }

    fn prev(&self, from: usize) -> Option<usize> {
        (0..from).rev().find(|&i| self.ok(i))
    }
}

const HALF_PAGE: usize = 5;

#[derive(Debug, Default, Clone)]
pub struct ListNav {
    pub cursor: Option<usize>,
    anchor: Option<usize>,
    count: Option<usize>,
    prefix: Option<char>,
}

impl ListNav {
    pub fn is_visual(&self) -> bool {
        self.anchor.is_some()
    }

    pub fn highlighted(&self) -> Option<RangeInclusive<usize>> {
        let c = self.cursor?;
        Some(match self.anchor {
            Some(a) => a.min(c)..=a.max(c),
            None => c..=c,
        })
    }

    pub fn pending_text(&self) -> String {
        let mut s = self.count.map(|c| c.to_string()).unwrap_or_default();
        if let Some(p) = self.prefix {
            s.push(p);
        }
        s
    }

    pub fn reset(&mut self) {
        self.anchor = None;
        self.count = None;
        self.prefix = None;
    }

    pub fn ensure_cursor(&mut self, spec: &ListSpec, prefer_last: bool) {
        if self.cursor.is_some_and(|c| spec.ok(c)) {
            return;
        }
        self.cursor = if prefer_last {
            spec.last()
        } else {
            spec.first()
        };
    }

    pub fn clamp(&mut self, spec: &ListSpec) {
        self.anchor = None;
        self.count = None;
        self.prefix = None;
        if let Some(c) = self.cursor {
            self.cursor = if spec.ok(c) {
                Some(c)
            } else {
                spec.prev(c.min(spec.len)).or_else(|| spec.first())
            };
        }
    }

    fn move_by(&mut self, spec: &ListSpec, n: usize, down: bool) -> bool {
        let Some(mut c) = self.cursor else {
            return false;
        };
        let mut moved = false;
        for _ in 0..n {
            match if down { spec.next(c) } else { spec.prev(c) } {
                Some(next) => {
                    c = next;
                    moved = true;
                }
                None => break,
            }
        }
        self.cursor = Some(c);
        moved
    }

    pub fn handle_key(&mut self, key: KeyEvent, spec: &ListSpec) -> ListEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if let KeyCode::Char(d @ '0'..='9') = key.code
            && !ctrl
            && (d != '0' || self.count.is_some())
        {
            let digit = d.to_digit(10).unwrap() as usize;
            self.count = Some(self.count.unwrap_or(0).saturating_mul(10) + digit);
            return ListEvent::None;
        }
        let n = self.count.take().unwrap_or(1).max(1);
        let prefix = self.prefix.take();

        let Some(cursor) = self.cursor else {
            return match key.code {
                KeyCode::Esc => ListEvent::Leave(Leave::Escape),
                _ => ListEvent::None,
            };
        };
        let visual = self.anchor.is_some();
        let range = || match self.anchor {
            Some(a) => a.min(cursor)..=a.max(cursor),
            None => cursor..=cursor,
        };

        match key.code {
            KeyCode::Char('j') | KeyCode::Down if !ctrl => {
                if !self.move_by(spec, n, true) && !visual {
                    return ListEvent::Leave(Leave::Down);
                }
            }
            KeyCode::Char('k') | KeyCode::Up if !ctrl => {
                self.move_by(spec, n, false);
            }
            KeyCode::Char('d') if ctrl => {
                self.move_by(spec, HALF_PAGE * n, true);
            }
            KeyCode::Char('u') if ctrl => {
                self.move_by(spec, HALF_PAGE * n, false);
            }
            KeyCode::Char('G') => self.cursor = spec.last().or(self.cursor),
            KeyCode::Char('g') if !ctrl => {
                if prefix == Some('g') {
                    self.cursor = spec.first().or(self.cursor);
                } else {
                    self.prefix = Some('g');
                }
            }
            KeyCode::Char('h') | KeyCode::Left if !visual => {
                return ListEvent::Leave(Leave::Left);
            }
            KeyCode::Char('l') | KeyCode::Right if !visual => {
                return ListEvent::Leave(Leave::Right);
            }
            KeyCode::Esc => {
                if visual {
                    self.anchor = None;
                } else {
                    return ListEvent::Leave(Leave::Escape);
                }
            }
            KeyCode::Char('v') | KeyCode::Char('V') if !ctrl => {
                self.anchor = if visual { None } else { Some(cursor) };
            }
            KeyCode::Char('o') if visual => {
                let a = self.anchor.replace(cursor);
                self.cursor = a;
            }
            KeyCode::Enter => {
                let r = range();
                self.anchor = None;
                return ListEvent::Insert(r);
            }
            KeyCode::Char('y') if !ctrl => {
                if visual || prefix == Some('y') {
                    let r = range();
                    self.anchor = None;
                    return ListEvent::Yank(r);
                }
                self.prefix = Some('y');
            }
            KeyCode::Char('d') | KeyCode::Char('x') if !ctrl => {
                if visual || prefix == Some('d') || key.code == KeyCode::Char('x') {
                    let r = range();
                    self.anchor = None;
                    return ListEvent::Delete(r);
                }
                self.prefix = Some('d');
            }
            KeyCode::Delete => {
                let r = range();
                self.anchor = None;
                return ListEvent::Delete(r);
            }
            KeyCode::Char('/') if spec.allow_filter && !visual => return ListEvent::StartFilter,
            _ => {}
        }
        ListEvent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(nav: &mut ListNav, spec: &ListSpec, keys: &str) -> Vec<ListEvent> {
        let mut out = Vec::new();
        for c in keys.chars() {
            let key = match c {
                '\n' => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                '\x1b' => KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                '\x04' => KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                c => KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
            };
            let e = nav.handle_key(key, spec);
            if e != ListEvent::None {
                out.push(e);
            }
        }
        out
    }

    fn all(len: usize) -> impl Fn(usize) -> bool {
        move |i| i < len
    }

    #[test]
    fn jk_move_and_skip_unselectable() {
        let sel = |i: usize| i.is_multiple_of(2);
        let spec = ListSpec {
            len: 7,
            selectable: &sel,
            allow_filter: false,
        };
        let mut nav = ListNav::default();
        nav.ensure_cursor(&spec, true);
        assert_eq!(nav.cursor, Some(6));
        press(&mut nav, &spec, "k");
        assert_eq!(nav.cursor, Some(4));
        press(&mut nav, &spec, "gg");
        assert_eq!(nav.cursor, Some(0));
        press(&mut nav, &spec, "2j");
        assert_eq!(nav.cursor, Some(4));
        press(&mut nav, &spec, "G");
        assert_eq!(nav.cursor, Some(6));
    }

    #[test]
    fn j_at_the_bottom_leaves_the_list() {
        let sel = all(3);
        let spec = ListSpec {
            len: 3,
            selectable: &sel,
            allow_filter: false,
        };
        let mut nav = ListNav::default();
        nav.ensure_cursor(&spec, true);
        assert_eq!(
            press(&mut nav, &spec, "j"),
            vec![ListEvent::Leave(Leave::Down)]
        );
        assert_eq!(
            press(&mut nav, &spec, "h"),
            vec![ListEvent::Leave(Leave::Left)]
        );
        assert_eq!(
            press(&mut nav, &spec, "l"),
            vec![ListEvent::Leave(Leave::Right)]
        );
    }

    #[test]
    fn enter_inserts_cursor_entry_and_visual_selects_range() {
        let sel = all(5);
        let spec = ListSpec {
            len: 5,
            selectable: &sel,
            allow_filter: false,
        };
        let mut nav = ListNav::default();
        nav.ensure_cursor(&spec, true);
        assert_eq!(press(&mut nav, &spec, "\n"), vec![ListEvent::Insert(4..=4)]);
        assert_eq!(
            press(&mut nav, &spec, "vkk\n"),
            vec![ListEvent::Insert(2..=4)]
        );
        assert!(!nav.is_visual());
        assert_eq!(press(&mut nav, &spec, "vky"), vec![ListEvent::Yank(1..=2)]);
    }

    #[test]
    fn dd_yy_and_x() {
        let sel = all(5);
        let spec = ListSpec {
            len: 5,
            selectable: &sel,
            allow_filter: false,
        };
        let mut nav = ListNav::default();
        nav.ensure_cursor(&spec, true);
        assert_eq!(press(&mut nav, &spec, "dd"), vec![ListEvent::Delete(4..=4)]);
        assert_eq!(press(&mut nav, &spec, "yy"), vec![ListEvent::Yank(4..=4)]);
        assert_eq!(press(&mut nav, &spec, "d"), vec![]);
        assert_eq!(press(&mut nav, &spec, "k"), vec![]);
        assert_eq!(press(&mut nav, &spec, "x"), vec![ListEvent::Delete(3..=3)]);
    }

    #[test]
    fn clamp_after_delete_moves_cursor_up() {
        let sel = all(2);
        let spec = ListSpec {
            len: 2,
            selectable: &sel,
            allow_filter: false,
        };
        let mut nav = ListNav {
            cursor: Some(4),
            ..Default::default()
        };
        nav.clamp(&spec);
        assert_eq!(nav.cursor, Some(1));
        let empty = ListSpec {
            len: 0,
            selectable: &sel,
            allow_filter: false,
        };
        nav.clamp(&empty);
        assert_eq!(nav.cursor, None);
    }

    #[test]
    fn half_page_and_filter() {
        let sel = all(20);
        let spec = ListSpec {
            len: 20,
            selectable: &sel,
            allow_filter: true,
        };
        let mut nav = ListNav::default();
        nav.ensure_cursor(&spec, false);
        press(&mut nav, &spec, "\x04");
        assert_eq!(nav.cursor, Some(5));
        assert_eq!(press(&mut nav, &spec, "/"), vec![ListEvent::StartFilter]);
    }
}
