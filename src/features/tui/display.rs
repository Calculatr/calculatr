use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::theme::Theme;
use super::typst::markup;
use crate::features::core::parser::parse_stmt;
use crate::features::core::{Answer, NumberFormat};
use crate::features::history::HistoryEntry;

const SYNC_LIMIT_BITS: u64 = 1 << 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayKey {
    pub numbers: NumberFormat,
    pub typst: bool,
}

#[derive(Debug, Clone)]
pub struct Rendered {
    pub text: String,
    pub typst: Option<String>,
}

pub enum View<'a> {
    Ready(&'a Rendered),
    Stale(&'a Rendered),
    Pending,
}

enum Slot {
    Ready(DisplayKey, Rendered),
    Pending {
        key: DisplayKey,
        epoch: u64,
        previous: Option<Rendered>,
    },
}

pub struct DisplayCache {
    results: HashMap<u64, Slot>,
    inputs: HashMap<u64, Option<String>>,
    key: Option<DisplayKey>,
    epoch: Arc<AtomicU64>,
    worker: Option<Worker>,
    background: bool,
    in_flight: usize,
}

impl Default for DisplayCache {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayCache {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            inputs: HashMap::new(),
            key: None,
            epoch: Arc::new(AtomicU64::new(0)),
            worker: None,
            background: false,
            in_flight: 0,
        }
    }

    pub fn enable_background(&mut self) {
        self.background = true;
    }

    pub fn is_busy(&self) -> bool {
        self.in_flight > 0
    }

    pub fn result(&mut self, entry: &HistoryEntry, key: DisplayKey, theme: &Theme) -> View<'_> {
        if self.key != Some(key) {
            self.key = Some(key);
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
        let epoch = self.epoch.load(Ordering::Relaxed);

        let up_to_date = match self.results.get(&entry.id) {
            Some(Slot::Ready(k, _)) => *k == key,
            Some(Slot::Pending {
                key: k, epoch: e, ..
            }) => *k == key && *e == epoch,
            None => false,
        };
        if !up_to_date {
            let previous = match self.results.remove(&entry.id) {
                Some(Slot::Ready(_, r)) => Some(r),
                Some(Slot::Pending { previous, .. }) => previous,
                None => None,
            };
            let slot = if self.background
                && answer_bits(&entry.answer) > SYNC_LIMIT_BITS
                && self.dispatch(entry, key, epoch, theme)
            {
                Slot::Pending {
                    key,
                    epoch,
                    previous,
                }
            } else {
                Slot::Ready(key, render(&entry.answer, &key, theme))
            };
            self.results.insert(entry.id, slot);
        }

        match self.results.get(&entry.id) {
            Some(Slot::Ready(_, r)) => View::Ready(r),
            Some(Slot::Pending {
                previous: Some(r), ..
            }) => View::Stale(r),
            _ => View::Pending,
        }
    }

    pub fn input_source(&mut self, entry: &HistoryEntry, theme: &Theme) -> Option<&str> {
        self.inputs
            .entry(entry.id)
            .or_insert_with(|| {
                parse_stmt(&entry.input)
                    .ok()
                    .map(|stmt| markup::input_source(&stmt, theme))
            })
            .as_deref()
    }

    pub fn poll(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let mut changed = false;
        while let Ok(done) = worker.done.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            let Some(Done { id, key, rendered }) = done else {
                continue;
            };
            if let Some(slot) = self.results.get_mut(&id)
                && matches!(slot, Slot::Pending { key: k, .. } if *k == key)
            {
                *slot = Slot::Ready(key, rendered);
                changed = true;
            }
        }
        changed
    }

    pub fn retain(&mut self, history: &[HistoryEntry]) {
        if self.results.len() <= history.len() && self.inputs.len() <= history.len() {
            return;
        }
        let live: std::collections::HashSet<u64> = history.iter().map(|e| e.id).collect();
        self.results.retain(|id, _| live.contains(id));
        self.inputs.retain(|id, _| live.contains(id));
    }

    fn dispatch(
        &mut self,
        entry: &HistoryEntry,
        key: DisplayKey,
        epoch: u64,
        theme: &Theme,
    ) -> bool {
        if self.worker.is_none() {
            self.worker = Worker::spawn(Arc::clone(&self.epoch));
        }
        let Some(worker) = &self.worker else {
            return false;
        };
        let job = Job {
            id: entry.id,
            key,
            epoch,
            answer: entry.answer.clone(),
            theme: theme.clone(),
        };
        if worker.jobs.send(job).is_err() {
            self.worker = None;
            return false;
        }
        self.in_flight += 1;
        true
    }
}

fn render(answer: &Answer, key: &DisplayKey, theme: &Theme) -> Rendered {
    Rendered {
        text: answer.text_with(&key.numbers),
        typst: if key.typst {
            markup::answer_source(answer, theme, &key.numbers)
        } else {
            None
        },
    }
}

fn answer_bits(answer: &Answer) -> u64 {
    match answer {
        Answer::Value { expr, unit, .. } => expr
            .num_bits()
            .saturating_add(unit.as_ref().map_or(0, |u| u.num_bits())),
        Answer::Message(_) | Answer::Error(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// Background worker
// ---------------------------------------------------------------------------

struct Job {
    id: u64,
    key: DisplayKey,
    epoch: u64,
    answer: Answer,
    theme: Theme,
}

struct Done {
    id: u64,
    key: DisplayKey,
    rendered: Rendered,
}

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<Option<Done>>,
}

impl Worker {
    fn spawn(epoch: Arc<AtomicU64>) -> Option<Self> {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel();
        thread::Builder::new()
            .name("display-format".into())
            .spawn(move || {
                for job in job_rx {
                    let result = (job.epoch == epoch.load(Ordering::Relaxed)).then(|| Done {
                        id: job.id,
                        key: job.key,
                        rendered: render(&job.answer, &job.key, &job.theme),
                    });
                    if done_tx.send(result).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self { jobs, done })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::notation::Notation;
    use crate::features::core::{Environment, Evaluation, RenderMode, evaluate};
    use std::time::{Duration, Instant};

    fn entry(src: &str, mode: RenderMode) -> HistoryEntry {
        let mut env = Environment::new();
        let Evaluation::Answer(a) = evaluate(&mut env, src, mode) else {
            panic!("no answer for {src}");
        };
        HistoryEntry::new(src, a)
    }

    fn key(notation: Notation) -> DisplayKey {
        DisplayKey {
            numbers: NumberFormat {
                notation,
                ..NumberFormat::default()
            },
            typst: true,
        }
    }

    fn text(view: View<'_>) -> Option<String> {
        match view {
            View::Ready(r) => Some(r.text.clone()),
            _ => None,
        }
    }

    #[test]
    fn output_is_cached_per_format() {
        let theme = Theme::cyberpunk();
        let mut cache = DisplayCache::new();
        let e = entry("1500", RenderMode::Decimal);
        assert_eq!(
            text(cache.result(&e, key(Notation::Standard), &theme)).unwrap(),
            "1500"
        );
        assert_eq!(
            text(cache.result(&e, key(Notation::E), &theme)).unwrap(),
            "1.5e3"
        );
        assert_eq!(
            text(cache.result(&e, key(Notation::Standard), &theme)).unwrap(),
            "1500"
        );
    }

    #[test]
    fn large_results_are_formatted_in_the_background() {
        let theme = Theme::cyberpunk();
        let mut cache = DisplayCache::new();
        cache.enable_background();
        let e = entry("10000! * 10000! * 10000!", RenderMode::Symbolic);
        assert!(answer_bits(&e.answer) > SYNC_LIMIT_BITS);

        assert!(matches!(
            cache.result(&e, key(Notation::Standard), &theme),
            View::Pending
        ));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cache.poll() {
            assert!(Instant::now() < deadline, "worker did not finish");
            thread::sleep(Duration::from_millis(1));
        }
        let first = text(cache.result(&e, key(Notation::Standard), &theme)).unwrap();
        assert!(first.ends_with("e106978"), "{first}");

        assert!(matches!(
            cache.result(&e, key(Notation::E), &theme),
            View::Stale(_)
        ));
        while cache.is_busy() {
            cache.poll();
            thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(
            cache.result(&e, key(Notation::E), &theme),
            View::Ready(_)
        ));
    }

    #[test]
    fn retain_forgets_removed_entries() {
        let theme = Theme::cyberpunk();
        let mut cache = DisplayCache::new();
        let a = entry("1", RenderMode::Decimal);
        let b = entry("2", RenderMode::Decimal);
        cache.result(&a, key(Notation::Standard), &theme);
        cache.result(&b, key(Notation::Standard), &theme);
        cache.retain(std::slice::from_ref(&b));
        assert_eq!(cache.results.len(), 1);
    }
}
