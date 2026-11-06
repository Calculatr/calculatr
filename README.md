# Calculatr

A keyboard-driven terminal calculator with exact arithmetic, a small computer algebra core,
typeset math output and Vim-style editing.

Written in Rust with [ratatui](https://ratatui.rs). Formulas can be rendered as real typeset math
(via [Typst](https://typst.app)) directly inside the terminal.

## Features

- **Exact arithmetic.** All numbers are exact rationals. `0.1 + 0.2` is `0.3`, and `0.25` is
  `1/4` internally. There is no binary floating-point noise.
- **Decimal and Symbolic mode.** The same value can be shown as `0.25` (Decimal) or `1/4`
  (Symbolic). Symbolic mode keeps expressions like `sqrt(2)`, `2*pi` or `5*x` exact and
  substitutes defined variables before simplifying.
- **Variables and equations.** `x := 0.25` assigns. A variable may also hold an equation,
  for example `y := 3x = 2`.
- **Typeset output.** Inputs and results are rendered as math with Typst, using the same colours
  as the terminal text, including the fade of older history lines. It can be switched off
  independently of Decimal / Symbolic mode.
- **Smart editing.**
  - Function snippets: `frac` + `Tab` becomes `frac(<>, <>)`, `Tab` jumps between parameters,
    and the last `Tab` leaves the call.
  - Brackets close themselves at a sensible place, and typing `)` in front of an existing `)`
    steps over it.
  - Completion for functions, constants and your variables, with ghost text and a signature hint.
- **Vim motions everywhere.** The input line has Normal, Insert and Visual mode. The history and
  the variables drawer are navigable with the same keys.
- **System clipboard.** Yanks and deletes are copied to the system clipboard, and `p` pastes from it.
- **Configurable.** A Settings tab manages the options and lets you rebind every hotkey.
- **Persistent.** History, variables and mode survive restarts.

## Requirements

- A recent stable Rust toolchain (edition 2024, let-chains; developed with Rust 1.97)
- The `chafa` development library, needed by `ratatui-image`
  - Debian / Ubuntu: `sudo apt install libchafa-dev`
  - Arch: `sudo pacman -S chafa`
  - macOS: `brew install chafa`
- A terminal with an image protocol (Kitty, WezTerm, iTerm2, foot, or any Sixel terminal) to see
  typeset math. Other terminals fall back to a lower-fidelity block rendering. Typst rendering
  can be turned off.

## Build and run

```sh
cargo run --release
```

Run the tests with `cargo test`.

## Usage

Type an expression and press `Enter`. An empty `Enter` repeats the previous input.

```text
> 1/4                     0.25            (Decimal)   1/4        (Symbolic)
> x := 0.25               x := 0.25       (Decimal)   x := 1/4   (Symbolic)
> frac(1, 4)              0.25
> sqrt(2)                 1.414213562373  (Decimal)   sqrt(2)    (Symbolic)
> 2^10                    1024
> y := 3x = 2             stores an equation in y
```

Precision rules in Decimal mode: a result is shown exactly if its expansion has at most 15
decimal places, otherwise with 13 significant digits.

### Hotkeys (defaults)

| Key       | Action                                          |
|-----------|-------------------------------------------------|
| `F2`      | Toggle Decimal / Symbolic                       |
| `F3`      | Variables drawer (open, focus, close)           |
| `F4`      | Switch between Calculator and Settings          |
| `F5`      | Toggle Typst rendering                          |
| `Ctrl+q`  | Quit                                            |
| `Ctrl+c`  | Quit (always, cannot be rebound)                |

All except `Ctrl+c` can be changed in the Settings tab. Bindings must be an F-key or a
Ctrl/Alt combination.

### Input line

| Mode    | Keys                                                                         |
|---------|------------------------------------------------------------------------------|
| Insert  | type, `Tab` / `Shift+Tab` complete and jump, `Enter` evaluate, `Esc` Normal   |
| Normal  | `h l w b e 0 ^ $ %` move, `i a I A` insert, `x X D C s S r p P u` edit        |
| Normal  | `d` `c` `y` + motion (`dw`, `c$`, `yy` ...), counts (`3x`, `2dw`), `v` Visual |
| Visual  | motions extend the selection, `y` `d` `c` act on it, `Esc` cancels            |

`k` or `Up` in Normal mode moves focus to the history.

### History and variables drawers

| Key                 | Action                                                   |
|---------------------|----------------------------------------------------------|
| `j` `k`             | Move                                                     |
| `gg` `G`            | First / last                                             |
| `Ctrl+d` `Ctrl+u`   | Half page down / up                                      |
| `v`                 | Select a range                                           |
| `y` `yy`            | Yank to register and system clipboard                    |
| `d` `dd`            | Delete                                                   |
| `Enter`             | Insert the selection into the input line                 |
| `/`                 | Filter (variables)                                       |
| `h` `l`             | Switch between history and variables                     |
| `Esc`               | Back to the input line                                   |

Counts work as in Vim (`5j`). Several selected history lines are inserted comma-separated, which
is handy as function arguments.

### Settings

| Setting                | Default |
|------------------------|---------|
| Typst rendering        | on      |
| Auto-close brackets    | on      |
| System clipboard       | on      |
| Hotkeys                | see above |

Navigate with `j` / `k`, change with `Enter` or `Space`, reset a hotkey with `r`.

### Clipboard notes

Calculatr uses the native clipboard (X11, Wayland, macOS, Windows). If none is available, for
example over SSH, it writes through the OSC 52 terminal escape sequence, which most modern
terminals support (some need it enabled in their settings). Reading from the clipboard works with
the native clipboard only. In a terminal, use its own paste shortcut (for example `Ctrl+Shift+V`)
to insert text.

### Files

Settings and state are stored in your platform's config directory under `Calculatr`:

- `settings.json` for options and hotkeys
- `state.json` for history, variables and mode (the last 500 entries are kept)

A corrupt file is moved to `*.bak` and replaced by defaults.

## Project layout

```text
src/
  main.rs                 entry point
  app/                    event loop, key routing, clipboard, settings tab logic
  features/
    core/                 numbers, parser, algebra, formatting, function registry, evaluator
    editor/               input buffer, completion, Vim editor, list navigation
    settings/             settings model and hotkey map
    history.rs            history model
    persistence/          settings.json / state.json
    tui/                  theme, highlighter, Typst rendering, views
```

Functions are defined once in `features/core/functions.rs`. That single registry drives
evaluation, simplification, Typst output and autocompletion, so adding a function is one entry.

---

## Math checklist

### Implemented

**Constants**
- [x] `pi`
- [x] `e`
- [X] `tau`
- [X] `phi`
- [X] `i` (imaginary unit)
- [X] `inf`
- [X] `ans` (last result)

**Operators and syntax**
- [x] `+`
- [x] `-` (binary and unary)
- [x] `*`
- [x] `/`
- [x] `^` (right-associative)
- [x] `( )`
- [x] implicit multiplication (`2x`, `3(x+1)`, `2pi`)
- [x] decimal literals (`0.25`, exact)
- [x] `=` (equation)
- [x] `:=` (assignment)
- [x] variables (multi-letter names)

**Functions**
- [x] `sin`
- [x] `cos`
- [x] `tan`
- [x] `asin`
- [x] `acos`
- [x] `atan`
- [X] `sinh`
- [X] `cosh`
- [X] `tanh`
- [X] `asinh`
- [X] `acosh`
- [X] `atanh`
- [X] `sec`
- [X] `csc`
- [X] `cot`
- [X] `atan2`
- [x] `exp`
- [x] `ln`
- [x] `log`
- [x] `sqrt`
- [x] `abs`
- [x] `add`
- [x] `sub`
- [x] `mult`
- [x] `frac`
- [X] `floor`
- [X] `ceil`
- [X] `round`
- [X] `trunc`
- [X] `fract`
- [X] `sign`

**Commands**
- [x] `clear`
- [x] `delvar`

### Missing

**Constants**
- tbd

**Operators and syntax**
- [ ] `!` (factorial)
- [ ] `%` (modulo)
- [ ] percent (`5%`)
- [ ] scientific notation (`1e-3`)
- [ ] comparison (`<`, `>`, `<=`, `>=`, `==`)
- [ ] hex / binary / octal literals
- [ ] user-defined functions (`f(x) := x^2`)

**Trigonometry**
- [ ] `deg` (rad to deg)
- [ ] `rad` (deg to rad)

**Powers and logarithms**
- [ ] `log2`
- [ ] `log10`
- [ ] `logb` (arbitrary base)
- [ ] `cbrt`
- [ ] `root`
- [ ] `hypot`

**Rounding and sign**
- [ ] round_ties_even (toggle)

**Number theory and combinatorics**
- [ ] `gcd`
- [ ] `lcm`
- [ ] `mod`
- [ ] `fact`
- [ ] `binom`
- [ ] `perm`
- [ ] `isprime`
- [ ] `factor` (prime factorization)

**Statistics**
- [ ] `min`
- [ ] `max`
- [ ] `sum`
- [ ] `prod`
- [ ] `mean`
- [ ] `median`
- [ ] `stdev`
- [ ] `rand`

**Algebra**
- [ ] `expand`
- [ ] `factor` (polynomials)
- [ ] `solve`
- [ ] `subs`
- [ ] `simplify` (explicit)
- [ ] `numer` / `denom`
- [ ] `diff`
- [ ] `integrate`
- [ ] `limit`

**Other**
- [ ] complex numbers
- [ ] vectors and matrices
- [ ] units and conversions
- [ ] `clearvars` (delete all variables)
- [ ] `vars` / `help` (list functions)

## Ideas

- [x] Drop the `=` on result lines
- [ ] Rename variables
- [ ] Number of decimal places and rounding mode as settings
- [ ] Radian / degree mode
- [x] Copy to clipboard in Visual mode
- [ ] Projects (Saveable states) for stored variables / Subfolders -> physics::g so theres a tree with organized variables

## Bugs

- [ ] When in autocomplete e.g. frac(<>, <>) the '<>' wont get removed on quick entering (select a value in history, press enter to quickenter)
