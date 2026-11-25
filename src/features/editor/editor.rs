use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::buffer::InputBuffer;
use super::completion::{self, CompState};
use crate::features::core::Candidate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Delete,
    Change,
    Yank,
}

impl Op {
    fn key(self) -> char {
        match self {
            Op::Delete => 'd',
            Op::Change => 'c',
            Op::Yank => 'y',
        }
    }
}

#[derive(Debug, Default)]
struct Pending {
    count: Option<usize>,
    op_count: Option<usize>,
    op: Option<Op>,
    g: bool,
    replace: bool,
}

pub struct EditorConfig<'a> {
    pub auto_close: bool,
    pub candidates: &'a [Candidate],
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditorEvent {
    None,
    Submit,
    FocusHistory,
}

struct Motion {
    target: usize,
    inclusive: bool,
    linewise: bool,
}

impl Motion {
    fn to(target: usize, inclusive: bool) -> Self {
        Self {
            target,
            inclusive,
            linewise: false,
        }
    }
}

const UNDO_LIMIT: usize = 100;

pub struct Editor {
    pub buffer: InputBuffer,
    mode: Mode,
    anchor: usize,
    pending: Pending,
    register: String,
    register_written: bool,
    undo: Vec<(Vec<char>, usize)>,
    comp: Option<CompState>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: InputBuffer::new(),
            mode: Mode::Insert,
            anchor: 0,
            pending: Pending::default(),
            register: String::new(),
            register_written: false,
            undo: Vec::new(),
            comp: None,
        }
    }

    // --- accessors for the UI ----------------------------------------------

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn register(&self) -> &str {
        &self.register
    }

    pub fn set_register(&mut self, text: String) {
        self.register = text;
        self.register_written = true;
    }

    pub fn load_register(&mut self, text: String) {
        self.register = text;
    }

    pub fn take_register_write(&mut self) -> bool {
        std::mem::take(&mut self.register_written)
    }

    pub fn visual_range(&self) -> Option<Range<usize>> {
        (self.mode == Mode::Visual).then(|| {
            let (a, b) = (self.anchor, self.buffer.cursor());
            a.min(b)..(a.max(b) + 1).min(self.buffer.len().max(1))
        })
    }

    pub fn pending_text(&self) -> String {
        let p = &self.pending;
        let mut s = String::new();
        if let Some(c) = p.op_count {
            s.push_str(&c.to_string());
        }
        if let Some(op) = p.op {
            s.push(op.key());
        }
        if let Some(c) = p.count {
            s.push_str(&c.to_string());
        }
        if p.g {
            s.push('g');
        }
        if p.replace {
            s.push('r');
        }
        s
    }

    pub fn completion_matches<'a>(
        &self,
        cands: &'a [Candidate],
    ) -> (Vec<&'a Candidate>, Option<usize>) {
        if self.mode != Mode::Insert {
            return (Vec::new(), None);
        }
        let (prefix, active) = match &self.comp {
            Some(c) => (c.prefix.clone(), Some(c.idx)),
            None => (
                completion::current_word(self.buffer.chars(), self.buffer.cursor()).1,
                None,
            ),
        };
        let matches = completion::matching(cands, &prefix);
        if completion::has_longer_match(&matches, &prefix) || active.is_some() {
            (matches, active)
        } else {
            (Vec::new(), None)
        }
    }

    pub fn ghost(&self, cands: &[Candidate]) -> Option<String> {
        if self.mode != Mode::Insert
            || self.comp.is_some()
            || self.buffer.cursor() != self.buffer.len()
        {
            return None;
        }
        let (_, word) = completion::current_word(self.buffer.chars(), self.buffer.cursor());
        if word.is_empty() {
            return None;
        }
        let matches = completion::matching(cands, &word);
        let pick = matches
            .iter()
            .find(|m| m.name.len() > word.len())
            .or_else(|| matches.iter().find(|m| m.name == word && m.is_function()))?;
        let single = matches.len() == 1;
        let text = completion::expand(pick, single || pick.name == word).text;
        Some(text.chars().skip(word.chars().count()).collect())
    }

    // --- external mutations -------------------------------------------------

    pub fn reset_after_submit(&mut self) {
        self.buffer.clear();
        self.mode = Mode::Insert;
        self.pending = Pending::default();
        self.comp = None;
        self.undo.clear();
    }

    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
        self.pending = Pending::default();
    }

    pub fn leave(&mut self) {
        if self.mode == Mode::Insert {
            self.enter_normal();
        } else {
            self.mode = Mode::Normal;
            self.pending = Pending::default();
            self.clamp_normal();
        }
    }

    pub fn paste_and_insert(&mut self, text: &str) {
        self.snapshot();
        if !text.is_empty() {
            self.prepare_paste_target(self.mode != Mode::Insert);
        } else if self.mode != Mode::Insert && !self.buffer.is_empty() {
            self.buffer.set_cursor(self.buffer.cursor() + 1);
        }
        self.mode = Mode::Insert;
        self.pending = Pending::default();
        self.comp = None;
        if !text.is_empty() {
            self.buffer.insert_str(text);
        }
    }

    fn prepare_paste_target(&mut self, after: bool) {
        if self.buffer.placeholder().is_some() {
            return;
        }
        let cursor = self.buffer.cursor();
        if let Some(start) = self.buffer.placeholder_covering(cursor) {
            self.buffer.select_placeholder_at(start);
            return;
        }
        if after && !self.buffer.is_empty() {
            let next = cursor + 1;
            if self.buffer.is_placeholder_at(next) {
                self.buffer.select_placeholder_at(next);
            } else {
                self.buffer.set_cursor(next);
            }
        }
    }

    // --- key handling -------------------------------------------------------

    pub fn handle_key(&mut self, key: KeyEvent, cfg: &EditorConfig) -> EditorEvent {
        let continues_cycle =
            self.mode == Mode::Insert && matches!(key.code, KeyCode::Tab | KeyCode::BackTab);
        if !continues_cycle {
            self.comp = None;
        }
        match self.mode {
            Mode::Insert => self.insert_key(key, cfg),
            Mode::Normal => self.normal_key(key),
            Mode::Visual => self.visual_key(key),
        }
    }

    fn insert_key(&mut self, key: KeyEvent, cfg: &EditorConfig) -> EditorEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let cursor = self.buffer.cursor();
        match key.code {
            KeyCode::Esc => self.enter_normal(),
            KeyCode::Enter => return EditorEvent::Submit,
            KeyCode::Tab => self.tab(cfg, true),
            KeyCode::BackTab => self.tab(cfg, false),
            KeyCode::Backspace => self.buffer.backspace(cfg.auto_close),
            KeyCode::Delete => self.buffer.delete_forward(),
            KeyCode::Left => self.buffer.set_cursor(cursor.saturating_sub(1)),
            KeyCode::Right => {
                let to = self.buffer.placeholder().map_or(cursor + 1, |p| p.end);
                self.buffer.set_cursor(to);
            }
            KeyCode::Home => self.buffer.set_cursor(0),
            KeyCode::End => self.buffer.set_cursor(self.buffer.len()),
            KeyCode::Up => {
                self.enter_normal();
                return EditorEvent::FocusHistory;
            }
            KeyCode::Char('h') if ctrl => self.buffer.backspace(cfg.auto_close),
            KeyCode::Char('w') if ctrl => self.buffer.delete_word_back(),
            KeyCode::Char('u') if ctrl => self.buffer.delete_to_start(),
            KeyCode::Char('a') if ctrl => self.buffer.set_cursor(0),
            KeyCode::Char('e') if ctrl => self.buffer.set_cursor(self.buffer.len()),
            KeyCode::Char(c) if !ctrl && !alt => self.buffer.insert_char(c, cfg.auto_close),
            _ => {}
        }
        EditorEvent::None
    }

    fn tab(&mut self, cfg: &EditorConfig, forward: bool) {
        if let Some(c) = self.comp.clone() {
            let matches = completion::matching(cfg.candidates, &c.prefix);
            if !matches.is_empty() {
                let n = matches.len();
                let idx = if forward {
                    (c.idx + 1) % n
                } else {
                    (c.idx + n - 1) % n
                };
                let len = self.apply_candidate(c.start..c.start + c.len, matches[idx]);
                self.comp = Some(CompState { idx, len, ..c });
                return;
            }
        }
        if !forward {
            return;
        }

        let cursor = self.buffer.cursor();
        let (start, word) = completion::current_word(self.buffer.chars(), cursor);
        let matches = completion::matching(cfg.candidates, &word);

        if !word.is_empty() && completion::has_longer_match(&matches, &word) {
            if matches.len() == 1 {
                self.apply_candidate(start..cursor, matches[0]);
            } else {
                let common = completion::common_prefix(&matches);
                if common.len() > word.len() {
                    self.buffer.replace_range(start..cursor, &common);
                } else {
                    let len = self.apply_candidate(start..cursor, matches[0]);
                    self.comp = Some(CompState {
                        prefix: word,
                        idx: 0,
                        start,
                        len,
                    });
                }
            }
            return;
        }

        if let Some(exact) = matches.iter().find(|m| m.name == word && m.is_function()) {
            self.apply_candidate(start..cursor, exact);
            return;
        }

        if self.buffer.select_next_placeholder() {
            return;
        }
        self.buffer.jump_out();
    }

    fn apply_candidate(&mut self, range: Range<usize>, cand: &Candidate) -> usize {
        let already_called = self.buffer.char_at(range.end) == Some('(');
        let expansion = completion::expand(cand, !already_called);
        let len = expansion.text.chars().count();
        self.buffer.replace_range(range.clone(), &expansion.text);
        if let Some(offset) = expansion.first_placeholder {
            self.buffer.select_placeholder_at(range.start + offset);
        }
        len
    }

    // --- normal mode --------------------------------------------------------

    fn normal_key(&mut self, key: KeyEvent) -> EditorEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if self.pending.replace {
            self.pending = Pending::default();
            if let KeyCode::Char(c) = key.code
                && !self.buffer.is_empty()
            {
                self.snapshot();
                let i = self.buffer.cursor();
                self.buffer.replace_range(i..i + 1, &c.to_string());
                self.buffer.set_cursor(i);
            }
            return EditorEvent::None;
        }

        if self.take_count_digit(key, ctrl) {
            return EditorEvent::None;
        }

        if self.pending.g {
            self.pending.g = false;
            if key.code == KeyCode::Char('g') {
                self.apply_motion_or_move(Motion::to(0, false));
            } else {
                self.pending = Pending::default();
            }
            return EditorEvent::None;
        }

        if let Some(motion) = self.motion_for(key, ctrl, self.pending.op) {
            self.apply_motion_or_move(motion);
            return EditorEvent::None;
        }

        match key.code {
            KeyCode::Esc => self.pending = Pending::default(),
            KeyCode::Enter => return EditorEvent::Submit,
            KeyCode::Up | KeyCode::Char('k') if !ctrl => {
                self.pending = Pending::default();
                return EditorEvent::FocusHistory;
            }
            KeyCode::Char('g') if !ctrl => self.pending.g = true,
            KeyCode::Char(c @ ('d' | 'c' | 'y')) if !ctrl => {
                let op = match c {
                    'd' => Op::Delete,
                    'c' => Op::Change,
                    _ => Op::Yank,
                };
                if self.pending.op == Some(op) {
                    self.apply_motion_or_move(Motion {
                        target: self.buffer.len(),
                        inclusive: false,
                        linewise: true,
                    });
                } else if self.pending.op.is_none() {
                    self.pending.op = Some(op);
                    self.pending.op_count = self.pending.count.take();
                } else {
                    self.pending = Pending::default();
                }
            }
            KeyCode::Char(c) if !ctrl => {
                let n = self.take_count();
                self.simple_command(c, n);
            }
            _ => self.pending = Pending::default(),
        }
        EditorEvent::None
    }

    fn simple_command(&mut self, c: char, n: usize) {
        let len = self.buffer.len();
        let cursor = self.buffer.cursor();
        match c {
            'i' => self.start_insert(cursor),
            'a' => self.start_insert((cursor + 1).min(len)),
            'I' => self.start_insert(self.buffer.first_non_blank()),
            'A' | 'o' => self.start_insert(len),
            'O' => self.start_insert(0),
            'x' => self.delete_span(cursor..(cursor + n).min(len), false),
            'X' => self.delete_span(cursor.saturating_sub(n)..cursor, false),
            'D' => self.delete_span(cursor..len, false),
            'C' => self.delete_span(cursor..len, true),
            's' => self.delete_span(cursor..(cursor + n).min(len), true),
            'S' => self.delete_span(0..len, true),
            'p' => self.paste(true),
            'P' => self.paste(false),
            'u' => {
                if let Some(s) = self.undo.pop() {
                    self.buffer.restore(s);
                    self.clamp_normal();
                }
            }
            'v' => {
                if !self.buffer.is_empty() {
                    self.mode = Mode::Visual;
                    self.anchor = cursor;
                }
            }
            'r' => self.pending.replace = true,
            _ => {}
        }
    }

    fn delete_span(&mut self, range: Range<usize>, then_insert: bool) {
        if range.start >= range.end && !then_insert {
            return;
        }
        self.snapshot();
        self.register = self.buffer.delete_range(range);
        self.register_written = true;
        if then_insert {
            self.mode = Mode::Insert;
        } else {
            self.clamp_normal();
        }
    }

    fn paste(&mut self, after: bool) {
        if self.register.is_empty() {
            return;
        }
        self.snapshot();
        self.prepare_paste_target(after);
        let text = self.register.clone();
        self.buffer.insert_str(&text);
        self.buffer
            .set_cursor(self.buffer.cursor().saturating_sub(1));
        self.clamp_normal();
    }

    fn take_count_digit(&mut self, key: KeyEvent, ctrl: bool) -> bool {
        if ctrl || self.pending.replace {
            return false;
        }
        if let KeyCode::Char(c @ '0'..='9') = key.code
            && (c != '0' || self.pending.count.is_some())
        {
            let d = c.to_digit(10).unwrap() as usize;
            self.pending.count = Some(self.pending.count.unwrap_or(0).saturating_mul(10) + d);
            return true;
        }
        false
    }

    fn take_count(&mut self) -> usize {
        let n = self.pending.op_count.take().unwrap_or(1) * self.pending.count.take().unwrap_or(1);
        self.pending = Pending::default();
        n.max(1)
    }

    fn motion_for(&mut self, key: KeyEvent, ctrl: bool, op: Option<Op>) -> Option<Motion> {
        if ctrl {
            return None;
        }
        let b = &self.buffer;
        let cursor = b.cursor();
        let last = b.len().saturating_sub(1);
        let n = self.pending.op_count.unwrap_or(1) * self.pending.count.unwrap_or(1);
        let n = n.max(1);
        Some(match key.code {
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                Motion::to(cursor.saturating_sub(n), false)
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => {
                Motion::to(cursor + n, false)
            }
            KeyCode::Char('0') | KeyCode::Home => Motion::to(0, false),
            KeyCode::Char('^') => Motion::to(b.first_non_blank(), false),
            KeyCode::Char('$') | KeyCode::End => Motion::to(last, true),
            KeyCode::Char('w') if op == Some(Op::Change) => {
                Motion::to(b.word_end(cursor, n).max(cursor), true)
            }
            KeyCode::Char('w') => Motion::to(b.word_forward(cursor, n), false),
            KeyCode::Char('b') => Motion::to(b.word_back(cursor, n), false),
            KeyCode::Char('e') => Motion::to(b.word_end(cursor, n), true),
            KeyCode::Char('%') => Motion::to(b.matching_bracket(cursor)?, true),
            _ => return None,
        })
    }

    fn apply_motion_or_move(&mut self, m: Motion) {
        let Some(op) = self.pending.op else {
            self.pending = Pending::default();
            self.buffer.set_cursor(m.target);
            self.clamp_normal();
            return;
        };
        self.pending = Pending::default();

        let len = self.buffer.len();
        let cursor = self.buffer.cursor();
        let range = if m.linewise {
            0..len
        } else if m.target >= cursor {
            cursor..(m.target + usize::from(m.inclusive)).min(len)
        } else {
            m.target..cursor
        };

        match op {
            Op::Yank => {
                self.register = self.buffer.slice(range.clone());
                self.register_written = true;
                self.buffer.set_cursor(range.start);
                self.clamp_normal();
            }
            Op::Delete => self.delete_span(range, false),
            Op::Change => self.delete_span(range, true),
        }
    }

    // --- visual mode --------------------------------------------------------

    fn visual_key(&mut self, key: KeyEvent) -> EditorEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if self.take_count_digit(key, ctrl) {
            return EditorEvent::None;
        }
        if self.pending.g {
            self.pending.g = false;
            if key.code == KeyCode::Char('g') {
                self.buffer.set_cursor(0);
            }
            return EditorEvent::None;
        }
        if let Some(m) = self.motion_for(key, ctrl, None) {
            self.pending = Pending::default();
            self.buffer.set_cursor(m.target);
            self.clamp_normal();
            return EditorEvent::None;
        }

        let (a, b) = (self.anchor, self.buffer.cursor());
        let range = a.min(b)..(a.max(b) + 1).min(self.buffer.len());
        match key.code {
            KeyCode::Esc | KeyCode::Char('v') => self.leave_visual(),
            KeyCode::Enter => return EditorEvent::Submit,
            KeyCode::Char('g') => self.pending.g = true,
            KeyCode::Char('o') => {
                self.anchor = b;
                self.buffer.set_cursor(a);
            }
            KeyCode::Char('y') => {
                self.register = self.buffer.slice(range.clone());
                self.buffer.set_cursor(range.start);
                self.leave_visual();
            }
            KeyCode::Char('d') | KeyCode::Char('x') => {
                self.leave_visual();
                self.delete_span(range, false);
            }
            KeyCode::Char('c') | KeyCode::Char('s') => {
                self.leave_visual();
                self.delete_span(range, true);
            }
            _ => {}
        }
        EditorEvent::None
    }

    fn leave_visual(&mut self) {
        self.mode = Mode::Normal;
        self.pending = Pending::default();
        self.clamp_normal();
    }

    // --- helpers ------------------------------------------------------------

    fn enter_normal(&mut self) {
        self.mode = Mode::Normal;
        self.pending = Pending::default();
        self.comp = None;
        let cursor = self.buffer.cursor();
        self.buffer.set_cursor(cursor.saturating_sub(1));
        self.clamp_normal();
    }

    fn clamp_normal(&mut self) {
        let max = self.buffer.len().saturating_sub(1);
        if self.buffer.cursor() > max {
            self.buffer.set_cursor(max);
        }
    }

    fn start_insert(&mut self, cursor: usize) {
        self.snapshot();
        self.buffer.set_cursor(cursor);
        self.mode = Mode::Insert;
    }

    fn snapshot(&mut self) {
        self.undo.push(self.buffer.snapshot());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::{Environment, completions};
    use crossterm::event::KeyEventKind;

    fn keys(ed: &mut Editor, script: &str) -> Vec<EditorEvent> {
        let cands = completions(&Environment::new());
        let cfg = EditorConfig {
            auto_close: true,
            candidates: &cands,
        };
        let mut events = Vec::new();
        let mut chars = script.chars().peekable();
        while let Some(c) = chars.next() {
            let (code, mods) = if c == '<' {
                let mut name = String::new();
                for n in chars.by_ref() {
                    if n == '>' {
                        break;
                    }
                    name.push(n);
                }
                match name.as_str() {
                    "Esc" => (KeyCode::Esc, KeyModifiers::NONE),
                    "Tab" => (KeyCode::Tab, KeyModifiers::NONE),
                    "S-Tab" => (KeyCode::BackTab, KeyModifiers::SHIFT),
                    "CR" => (KeyCode::Enter, KeyModifiers::NONE),
                    "BS" => (KeyCode::Backspace, KeyModifiers::NONE),
                    "Up" => (KeyCode::Up, KeyModifiers::NONE),
                    "C-w" => (KeyCode::Char('w'), KeyModifiers::CONTROL),
                    other => panic!("unknown key <{other}>"),
                }
            } else {
                (KeyCode::Char(c), KeyModifiers::NONE)
            };
            let mut ev = KeyEvent::new(code, mods);
            ev.kind = KeyEventKind::Press;
            let r = ed.handle_key(ev, &cfg);
            if r != EditorEvent::None {
                events.push(r);
            }
        }
        events
    }

    fn typed(script: &str) -> Editor {
        let mut ed = Editor::new();
        keys(&mut ed, script);
        ed
    }

    fn text(ed: &Editor) -> String {
        ed.buffer.text()
    }

    // --- completion / snippets ----------------------------------------------

    #[test]
    fn tab_expands_function_to_snippet_with_tab_stops() {
        let mut ed = typed("fra<Tab>");
        assert_eq!(text(&ed), "frac(<>, <>)");
        assert_eq!(ed.buffer.placeholder(), Some(5..7));

        keys(&mut ed, "3");
        assert_eq!(text(&ed), "frac(3, <>)");
        keys(&mut ed, "<Tab>");
        assert_eq!(ed.buffer.placeholder(), Some(8..10));
        keys(&mut ed, "4");
        assert_eq!(text(&ed), "frac(3, 4)");

        keys(&mut ed, "<Tab>");
        assert_eq!(ed.buffer.cursor(), text(&ed).chars().count());
        keys(&mut ed, "+1");
        assert_eq!(text(&ed), "frac(3, 4)+1");
    }

    #[test]
    fn typing_close_before_existing_close_keeps_the_existing_one() {
        let ed = typed("sin<Tab>0)");
        assert_eq!(text(&ed), "sin(0)");
        assert_eq!(ed.buffer.cursor(), 6);
    }

    #[test]
    fn fully_typed_function_name_also_gets_its_snippet() {
        let ed = typed("frac<Tab>");
        assert_eq!(text(&ed), "frac(<>, <>)");
        let ed = typed("clear<Tab>");
        assert_eq!(text(&ed), "clear()");
    }

    #[test]
    fn tab_extends_to_common_prefix_then_cycles() {
        let mut ed = typed("s<Tab>");
        assert!(text(&ed).starts_with("sin"), "{}", text(&ed));
        keys(&mut ed, "<Tab>");
        assert!(text(&ed).starts_with("sqrt"), "{}", text(&ed));
        keys(&mut ed, "<S-Tab>");
        assert!(text(&ed).starts_with("sin"), "{}", text(&ed));
        keys(&mut ed, "2");
        assert_eq!(text(&ed), "sin(2)");

        let ed = typed("ac<Tab>");
        assert_eq!(text(&ed), "acos(<>)");
    }

    #[test]
    fn tab_does_not_double_the_paren() {
        let mut ed = Editor::new();
        ed.buffer = InputBuffer::from_text("sq(4)");
        ed.buffer.set_cursor(2);
        keys(&mut ed, "<Tab>");
        assert_eq!(text(&ed), "sqrt(4)");
    }

    #[test]
    fn ghost_shows_snippet_remainder() {
        let ed = typed("fra");
        let cands = completions(&Environment::new());
        assert_eq!(ed.ghost(&cands).as_deref(), Some("c(<>, <>)"));
    }

    // --- vim ------------------------------------------------------------------

    #[test]
    fn esc_goes_to_normal_and_back_with_i_a() {
        let mut ed = typed("12+3<Esc>");
        assert_eq!(ed.mode(), Mode::Normal);
        assert_eq!(ed.buffer.cursor(), 3);
        keys(&mut ed, "a4");
        assert_eq!(text(&ed), "12+34");
        keys(&mut ed, "<Esc>0iX");
        assert_eq!(text(&ed), "X12+34");
    }

    #[test]
    fn normal_mode_motions_and_deletes() {
        let mut ed = typed("foo + bar<Esc>0");
        keys(&mut ed, "w");
        assert_eq!(ed.buffer.cursor(), 4);
        keys(&mut ed, "dw");
        assert_eq!(text(&ed), "foo bar");
        keys(&mut ed, "0x");
        assert_eq!(text(&ed), "oo bar");
        keys(&mut ed, "$x");
        assert_eq!(text(&ed), "oo ba");
        keys(&mut ed, "u");
        assert_eq!(text(&ed), "oo bar");
    }

    #[test]
    fn operators_with_counts_and_lines() {
        let mut ed = typed("a+b+c+d<Esc>0");
        keys(&mut ed, "3x");
        assert_eq!(text(&ed), "+c+d");
        keys(&mut ed, "2dl");
        assert_eq!(text(&ed), "+d");
        keys(&mut ed, "dd");
        assert_eq!(text(&ed), "");
        keys(&mut ed, "p");
        assert_eq!(text(&ed), "+d");
    }

    #[test]
    fn change_and_yank_paste() {
        let mut ed = typed("one two<Esc>0");
        keys(&mut ed, "cwuno<Esc>");
        assert_eq!(text(&ed), "uno two");
        keys(&mut ed, "0yw$p");
        assert_eq!(text(&ed), "uno twouno ");
        keys(&mut ed, "yyP");
        assert_eq!(text(&ed), "uno twouno uno twouno ");
    }

    #[test]
    fn undo_restores_previous_state() {
        let mut ed = typed("123<Esc>0");
        keys(&mut ed, "xx");
        assert_eq!(text(&ed), "3");
        keys(&mut ed, "u");
        assert_eq!(text(&ed), "23");
        keys(&mut ed, "u");
        assert_eq!(text(&ed), "123");
    }

    #[test]
    fn percent_jumps_between_brackets_and_deletes_group() {
        let mut ed = typed("1+(2*3)+4<Esc>0");
        keys(&mut ed, "ll%");
        assert_eq!(ed.buffer.cursor(), 6);
        keys(&mut ed, "%");
        assert_eq!(ed.buffer.cursor(), 2);
        keys(&mut ed, "d%");
        assert_eq!(text(&ed), "1++4");
    }

    #[test]
    fn visual_mode_selects_and_operates() {
        let mut ed = typed("hello world<Esc>0");
        keys(&mut ed, "vey");
        assert_eq!(ed.register(), "hello");
        assert_eq!(ed.mode(), Mode::Normal);
        keys(&mut ed, "wvd");
        assert_eq!(text(&ed), "hello orld");
        keys(&mut ed, "0v$c!");
        assert_eq!(text(&ed), "!");
    }

    #[test]
    fn replace_char_with_r() {
        let mut ed = typed("a+b<Esc>0");
        keys(&mut ed, "r*");
        assert_eq!(text(&ed), "*+b");
    }

    #[test]
    fn k_and_up_leave_for_history() {
        let mut ed = typed("1<Esc>");
        assert_eq!(keys(&mut ed, "k"), vec![EditorEvent::FocusHistory]);
        let mut ed = typed("1");
        assert_eq!(keys(&mut ed, "<Up>"), vec![EditorEvent::FocusHistory]);
        assert_eq!(ed.mode(), Mode::Normal);
    }

    #[test]
    fn enter_submits_in_every_mode() {
        let mut ed = typed("1+1");
        assert_eq!(keys(&mut ed, "<CR>"), vec![EditorEvent::Submit]);
        keys(&mut ed, "<Esc>");
        assert_eq!(keys(&mut ed, "<CR>"), vec![EditorEvent::Submit]);
    }

    #[test]
    fn pasting_from_lists_goes_behind_the_cursor() {
        let mut ed = typed("x + <Esc>");
        ed.paste_and_insert("y");
        assert_eq!(text(&ed), "x + y");
        assert_eq!(ed.mode(), Mode::Insert);
        keys(&mut ed, " + z");
        assert_eq!(text(&ed), "x + y + z");

        let mut ed = Editor::new();
        ed.paste_and_insert("abc");
        assert_eq!(text(&ed), "abc");
    }

    #[test]
    fn pasting_replaces_placeholders_in_all_cursor_positions() {
        // Leaving from selected placeholder (e.g. Up to history or F3 to variables)
        let mut ed = typed("frac<Tab><Up>");
        ed.paste_and_insert("12");
        assert_eq!(text(&ed), "frac(12, <>)");
        assert_eq!(ed.mode(), Mode::Insert);
        keys(&mut ed, "<Tab>");
        ed.leave();
        ed.paste_and_insert("x");
        assert_eq!(text(&ed), "frac(12, x)");

        // Undo restores the placeholder
        keys(&mut ed, "<Esc>u");
        assert_eq!(text(&ed), "frac(12, <>)");

        // Normal mode cursor directly on '<' (would previously produce frac(<42>, <>))
        let mut ed = typed("frac<Tab><Esc>l");
        assert_eq!(ed.buffer.cursor(), 5);
        ed.paste_and_insert("42");
        assert_eq!(text(&ed), "frac(42, <>)");

        // Normal mode cursor directly on '>'
        let mut ed = typed("frac<Tab><Esc>ll");
        assert_eq!(ed.buffer.cursor(), 6);
        ed.paste_and_insert("42");
        assert_eq!(text(&ed), "frac(42, <>)");

        // Placeholder at index 0
        let mut ed = Editor::new();
        ed.buffer = InputBuffer::from_text("<> + 1");
        ed.buffer.select_placeholder_at(0);
        ed.leave();
        ed.paste_and_insert("9");
        assert_eq!(text(&ed), "9 + 1");

        // Bracketed paste while in insert mode on a placeholder
        let mut ed = typed("frac<Tab>");
        ed.paste_and_insert("7");
        assert_eq!(text(&ed), "frac(7, <>)");

        // Empty paste does not delete an existing placeholder
        let mut ed = typed("frac<Tab><Up>");
        ed.paste_and_insert("");
        assert_eq!(text(&ed), "frac(<>, <>)");
    }

    #[test]
    fn normal_mode_paste_replaces_targeted_placeholder() {
        // p right before placeholder (after leaving insert mode from placeholder)
        let mut ed = typed("frac<Tab><Esc>");
        ed.load_register("42".into());
        keys(&mut ed, "p");
        assert_eq!(text(&ed), "frac(42, <>)");

        // P before '(' does not eat the placeholder
        let mut ed = typed("frac<Tab><Esc>");
        ed.load_register("2".into());
        keys(&mut ed, "P");
        assert_eq!(text(&ed), "frac2(<>, <>)");

        // p and P directly on '<' or '>' replace the placeholder
        let mut ed = typed("frac<Tab><Esc>l");
        ed.load_register("42".into());
        keys(&mut ed, "p");
        assert_eq!(text(&ed), "frac(42, <>)");

        let mut ed = typed("frac<Tab><Esc>ll");
        ed.load_register("42".into());
        keys(&mut ed, "P");
        assert_eq!(text(&ed), "frac(42, <>)");
    }

    #[test]
    fn ctrl_w_deletes_a_word() {
        let ed = typed("foo bar<C-w>");
        assert_eq!(text(&ed), "foo ");
    }
}
