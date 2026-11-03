use std::ops::Range;

pub const PLACEHOLDER: &str = "<>";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputBuffer {
    chars: Vec<char>,
    cursor: usize,
    placeholder: Option<usize>,
}

fn class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

impl InputBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn from_text(s: &str) -> Self {
        let chars: Vec<char> = s.chars().collect();
        Self {
            cursor: chars.len(),
            chars,
            placeholder: None,
        }
    }

    // --- accessors ----------------------------------------------------------

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn char_at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    pub fn slice(&self, r: Range<usize>) -> String {
        self.chars[r.start.min(self.len())..r.end.min(self.len())]
            .iter()
            .collect()
    }

    pub fn placeholder(&self) -> Option<Range<usize>> {
        self.placeholder.map(|p| p..p + PLACEHOLDER.len())
    }

    pub fn snapshot(&self) -> (Vec<char>, usize) {
        (self.chars.clone(), self.cursor)
    }

    pub fn restore(&mut self, (chars, cursor): (Vec<char>, usize)) {
        self.chars = chars;
        self.cursor = cursor.min(self.chars.len());
        self.placeholder = None;
    }

    // --- cursor -------------------------------------------------------------

    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.chars.len());
        self.placeholder = None;
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
        self.placeholder = None;
    }

    // --- editing ------------------------------------------------------------

    pub fn replace_range(&mut self, r: Range<usize>, with: &str) {
        let end = r.end.min(self.len());
        let start = r.start.min(end);
        let new: Vec<char> = with.chars().collect();
        let new_len = new.len();
        self.chars.splice(start..end, new);
        self.cursor = start + new_len;
        self.placeholder = None;
    }

    pub fn delete_range(&mut self, r: Range<usize>) -> String {
        let removed = self.slice(r.clone());
        self.replace_range(r, "");
        removed
    }

    fn take_selected_placeholder(&mut self) -> bool {
        match self.placeholder.take() {
            Some(p) => {
                self.chars.drain(p..p + PLACEHOLDER.len());
                self.cursor = p;
                true
            }
            None => false,
        }
    }

    pub fn insert_str(&mut self, s: &str) {
        self.take_selected_placeholder();
        let new: Vec<char> = s.chars().collect();
        let n = new.len();
        self.chars.splice(self.cursor..self.cursor, new);
        self.cursor += n;
    }

    pub fn insert_char(&mut self, c: char, auto_close: bool) {
        self.take_selected_placeholder();
        if auto_close {
            match c {
                ')' if self.char_at(self.cursor) == Some(')') => {
                    self.cursor += 1;
                    return;
                }
                '(' => {
                    let close = self.smart_close_index(self.cursor);
                    self.chars.insert(close, ')');
                    self.chars.insert(self.cursor, '(');
                    self.cursor += 1;
                    return;
                }
                _ => {}
            }
        }
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self, auto_close: bool) {
        if self.take_selected_placeholder() || self.cursor == 0 {
            return;
        }
        let c = self.cursor;
        if auto_close && self.chars[c - 1] == '(' && self.char_at(c) == Some(')') {
            self.chars.drain(c - 1..c + 1);
            self.cursor -= 1;
        } else if c >= 2 && self.slice(c - 2..c) == PLACEHOLDER {
            self.chars.drain(c - 2..c);
            self.cursor -= 2;
        } else {
            self.chars.remove(c - 1);
            self.cursor -= 1;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.take_selected_placeholder() {
            return;
        }
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn delete_word_back(&mut self) {
        let start = self.word_back(self.cursor, 1);
        self.delete_range(start..self.cursor);
    }

    pub fn delete_to_start(&mut self) {
        self.delete_range(0..self.cursor);
    }

    // --- smart brackets -----------------------------------------------------

    pub fn smart_close_index(&self, from: usize) -> usize {
        let n = self.chars.len();
        let mut i = from;
        let mut depth = 0usize;
        let mut end = from;
        let mut expects_operand = true;

        while i < n {
            let c = self.chars[i];
            let mut next_expects_operand = expects_operand;
            match c {
                '(' => {
                    depth += 1;
                    end = i + 1;
                    next_expects_operand = true;
                }
                ')' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    end = i + 1;
                    next_expects_operand = false;
                }
                ',' | '=' | ':' if depth == 0 => break,
                '+' | '-' if depth == 0 && !expects_operand => break,
                '+' | '-' | '*' | '/' | '^' => {
                    end = i + 1;
                    next_expects_operand = true;
                }
                '<' if self.char_at(i + 1) == Some('>') => {
                    i += 1;
                    end = i + 1;
                    next_expects_operand = false;
                }
                c if c.is_whitespace() => {}
                _ => {
                    end = i + 1;
                    next_expects_operand = false;
                }
            }
            expects_operand = next_expects_operand;
            i += 1;
        }
        end
    }

    pub fn jump_out(&mut self) -> bool {
        let mut depth = 0usize;
        let mut open = None;
        for i in (0..self.cursor).rev() {
            match self.chars[i] {
                ')' => depth += 1,
                '(' if depth == 0 => {
                    open = Some(i);
                    break;
                }
                '(' => depth -= 1,
                _ => {}
            }
        }
        if open.is_none() {
            return false;
        }
        let mut depth = 0usize;
        for i in self.cursor..self.chars.len() {
            match self.chars[i] {
                '(' => depth += 1,
                ')' if depth == 0 => {
                    self.set_cursor(i + 1);
                    return true;
                }
                ')' => depth -= 1,
                _ => {}
            }
        }
        self.set_cursor(self.chars.len());
        true
    }

    pub fn matching_bracket(&self, idx: usize) -> Option<usize> {
        match self.char_at(idx)? {
            '(' => {
                let mut depth = 0usize;
                for i in idx + 1..self.chars.len() {
                    match self.chars[i] {
                        '(' => depth += 1,
                        ')' if depth == 0 => return Some(i),
                        ')' => depth -= 1,
                        _ => {}
                    }
                }
                None
            }
            ')' => {
                let mut depth = 0usize;
                for i in (0..idx).rev() {
                    match self.chars[i] {
                        ')' => depth += 1,
                        '(' if depth == 0 => return Some(i),
                        '(' => depth -= 1,
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    // --- placeholders -------------------------------------------------------

    fn find_placeholder(&self, from: usize) -> Option<usize> {
        let marker: Vec<char> = PLACEHOLDER.chars().collect();
        (from..self.chars.len().saturating_sub(marker.len() - 1))
            .find(|&i| self.chars[i..i + marker.len()] == marker[..])
    }

    pub fn has_placeholder(&self) -> bool {
        self.find_placeholder(0).is_some()
    }

    pub fn select_placeholder_at(&mut self, start: usize) {
        self.cursor = start;
        self.placeholder = Some(start);
    }

    pub fn select_next_placeholder(&mut self) -> bool {
        let skip = if self.placeholder.is_some() {
            PLACEHOLDER.len()
        } else {
            0
        };
        let found = self
            .find_placeholder(self.cursor + skip)
            .or_else(|| self.find_placeholder(0));
        match found {
            Some(p) => {
                self.select_placeholder_at(p);
                true
            }
            None => false,
        }
    }

    // --- motions (pure; used by the vim layer) --------------------------------

    pub fn first_non_blank(&self) -> usize {
        self.chars
            .iter()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0)
    }

    pub fn word_forward(&self, mut i: usize, count: usize) -> usize {
        let len = self.chars.len();
        for _ in 0..count {
            if i >= len {
                break;
            }
            let cls = class(self.chars[i]);
            if cls != 0 {
                while i < len && class(self.chars[i]) == cls {
                    i += 1;
                }
            }
            while i < len && class(self.chars[i]) == 0 {
                i += 1;
            }
        }
        i
    }

    pub fn word_back(&self, mut i: usize, count: usize) -> usize {
        for _ in 0..count {
            if i == 0 {
                break;
            }
            i -= 1;
            while i > 0 && class(self.chars[i]) == 0 {
                i -= 1;
            }
            let cls = class(self.chars[i]);
            while i > 0 && class(self.chars[i - 1]) == cls {
                i -= 1;
            }
        }
        i
    }

    pub fn word_end(&self, mut i: usize, count: usize) -> usize {
        let len = self.chars.len();
        for _ in 0..count {
            if i + 1 >= len {
                break;
            }
            i += 1;
            while i + 1 < len && class(self.chars[i]) == 0 {
                i += 1;
            }
            let cls = class(self.chars[i]);
            while i + 1 < len && class(self.chars[i + 1]) == cls {
                i += 1;
            }
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(s: &str, auto_close: bool) -> InputBuffer {
        let mut b = InputBuffer::new();
        for c in s.chars() {
            b.insert_char(c, auto_close);
        }
        b
    }

    #[test]
    fn typing_a_closed_group_ends_up_balanced() {
        let b = typed("(1+2)", true);
        assert_eq!(b.text(), "(1+2)");
        assert_eq!(b.cursor(), 5);
        let b = typed("sin(cos(0))", true);
        assert_eq!(b.text(), "sin(cos(0))");
        assert_eq!(b.cursor(), b.len());
    }

    #[test]
    fn typing_close_in_front_of_existing_close_reuses_it() {
        let mut b = typed("(1+2", true);
        assert_eq!(b.text(), "(1+2)");
        b.insert_char(')', true);
        assert_eq!(b.text(), "(1+2)");
        assert_eq!(b.cursor(), 5);
    }

    #[test]
    fn auto_close_can_be_disabled() {
        assert_eq!(typed("(1", false).text(), "(1");
        assert_eq!(typed("(1)", false).text(), "(1)");
    }

    #[test]
    fn open_bracket_wraps_the_following_term() {
        let mut b = InputBuffer::from_text("3*4+5");
        b.set_cursor(0);
        b.insert_char('(', true);
        assert_eq!(b.text(), "(3*4)+5");
        assert_eq!(b.cursor(), 1);

        let mut b = InputBuffer::from_text("2+3*sin(x)-1");
        b.set_cursor(2);
        b.insert_char('(', true);
        assert_eq!(b.text(), "2+(3*sin(x))-1");

        let mut b = InputBuffer::from_text("-3+1");
        b.set_cursor(0);
        b.insert_char('(', true);
        assert_eq!(b.text(), "(-3)+1");

        let mut b = InputBuffer::from_text("f(1,2)");
        b.set_cursor(4);
        b.insert_char('(', true);
        assert_eq!(b.text(), "f(1,(2))");

        let mut b = InputBuffer::from_text("f(1,2)");
        b.set_cursor(5);
        b.insert_char('(', true);
        assert_eq!(b.text(), "f(1,2())");
        assert_eq!(b.cursor(), 6);
    }

    #[test]
    fn backspace_removes_empty_pairs_and_placeholders() {
        let mut b = typed("sin(", true);
        assert_eq!(b.text(), "sin()");
        b.backspace(true);
        assert_eq!(b.text(), "sin");

        let mut b = InputBuffer::from_text("f(<>, <>)");
        b.set_cursor(4);
        b.backspace(true);
        assert_eq!(b.text(), "f(, <>)");
    }

    #[test]
    fn placeholders_are_replaced_by_typing() {
        let mut b = InputBuffer::from_text("frac(<>, <>)");
        assert!(b.select_next_placeholder());
        assert_eq!(b.placeholder(), Some(5..7));
        b.insert_char('3', true);
        assert_eq!(b.text(), "frac(3, <>)");
        assert!(b.select_next_placeholder());
        b.insert_char('4', true);
        assert_eq!(b.text(), "frac(3, 4)");
        assert!(!b.select_next_placeholder());
    }

    #[test]
    fn jump_out_leaves_the_innermost_group() {
        let mut b = InputBuffer::from_text("f(g(1, 2), 3)");
        b.set_cursor(6);
        assert!(b.jump_out());
        assert_eq!(b.cursor(), 9);
        assert!(b.jump_out());
        assert_eq!(b.cursor(), 13);
        assert!(!b.jump_out());
    }

    #[test]
    fn word_motions() {
        let b = InputBuffer::from_text("foo + bar2*x");
        assert_eq!(b.word_forward(0, 1), 4);
        assert_eq!(b.word_forward(4, 1), 6);
        assert_eq!(b.word_forward(6, 2), 11);
        assert_eq!(b.word_back(6, 1), 4);
        assert_eq!(b.word_back(4, 1), 0);
        assert_eq!(b.word_end(0, 1), 2);
        assert_eq!(b.word_end(2, 1), 4);
    }

    #[test]
    fn matching_brackets() {
        let b = InputBuffer::from_text("a(b(c))");
        assert_eq!(b.matching_bracket(1), Some(6));
        assert_eq!(b.matching_bracket(6), Some(1));
        assert_eq!(b.matching_bracket(3), Some(5));
        assert_eq!(b.matching_bracket(0), None);
    }
}
