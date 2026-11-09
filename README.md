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
- **Sections.** Dotted names organise variables in a tree of any depth: `physics.g := 9.81`,
  `physics.mech.v0 := 3`. The variables drawer shows them as foldable folders.
- **Units.** `9.81_m/s^2 * 2_kg` gives `19.62_N`; `72_km/h -> m/s` converts. Units are checked
  (`1_m + 1_s` is an error) and combine with SI prefixes.
- **Number formatting.** Round to a fixed number of decimal places and choose between
  `1500`, `1.5*10^3`, `1.5e3` and `1.5k`, with scientific or engineering exponents.
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
- **Configurable.** A Settings tab manages the options and lets you rebind every hotkey. Every
  option can also be changed with a command such as `notation(si)` or `set(typst, off)`.
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
decimal places, otherwise with 13 significant digits. The number format settings below build on
that value.

### Number format

Applies to Decimal mode; Symbolic mode always stays exact. The whole history is re-rendered
when a setting changes.

| Setting              | Values                         | Example for 15 000.5            |
|----------------------|--------------------------------|---------------------------------|
| Decimal places       | `auto`, `0` … `15`             | `decimals(0)` → `15001`         |
| Notation             | `standard` `power` `e` `si`    | `15000.5` `1.50005*10^4` `1.50005e4` `15.0005k` |
| Exponent step        | `sci` (one digit before the point) or `eng` (multiples of 3) | `e` + `eng` → `15.0005e3` |
| Extended SI prefixes | `on` / `off`                   | adds `c` `d` `da` `h`: `0.05` → `5c` |

Rounding is exact (half away from zero) and trailing zeros are omitted. With decimal places
set, they apply to the mantissa: `decimals(2)` + `notation(si)` shows `15k`. SI prefixes always
use steps of three (`q` … `Q`, micro is `µ`); beyond them the output falls back to `e`.
When a result is inserted into the input line it is written as `*10^n`, so it can be read back.

### Commands

| Command                          | Effect                                                    |
|----------------------------------|-----------------------------------------------------------|
| `clear()`                        | Clear the history                                         |
| `delvar(x)`                      | Delete a variable                                         |
| `delvar(physics, -r)`            | Delete a section and everything in it (`-rf` works too)   |
| `settings()`                     | List all settings                                         |
| `set(name, value)`               | Change a setting, e.g. `set(typst, off)`, `set(decimals, 4)` |
| `set(name)`                      | Flip a switch / advance to the next choice                |
| `notation(standard\|power\|e\|si)` | Notation; `notation(sci)` / `notation(eng)` pick power of ten with that exponent step |
| `decimals(auto\|n)`               | Decimal places                                            |
| `exponent(sci\|eng)`              | Exponent step                                             |
| `extprefixes(on\|off)`            | Extended SI prefixes                                      |

Setting names for `set`: `typst`, `autoclose`, `clipboard`, `complex`, `decimals`, `notation`,
`exponent`, `extprefixes` (aliases like `precision` or `format` are accepted). Without an
argument, `notation()` and friends cycle to the next value.

### Sections

`physics.g := 9.81` creates the variable `g` in the section `physics`; sections can be nested to
any depth (`physics.a1.a.v`). A name is either a variable or a section, like a file and a
directory. Deleting a section needs the recursive flag: `delvar(physics, -r)`. Completion works
on the full path, so `phy` + `Tab` completes to `physics.`.

### Units

A unit follows a number or a closing bracket directly, after `_`:

```text
> 5_N                     5_N
> 9.81_m/s^2 * 2_kg       19.62_N
> 3_km + 200_m            3200_m          (3.2_km with notation(si))
> (2+3)_kOhm * 2_mA       10_V
> 72_km/h -> m/s          20_m/s
> 1_h + 30_min -> min     90_min
> 1_m + 1_s               Error: Incompatible units: m and s
```

- The unit chain is written without spaces: `5_m/s` is metres per second, `5_m / s` divides
  by a variable `s`. A chain stops at the first non-unit, so `2_m*x` is `2_m` times `x`.
  `_m^2` raises only the unit (`5_m^2` is five square metres); use `(5_m)^2` for 25 m².
- `value -> unit` converts at the end of an input or assignment (`v := 72_km/h -> m/s`).
- Without a conversion, a result keeps the unit if only one unit was used (`2_h + 3_h` = `5_h`),
  otherwise it is shown in a named SI unit (`N`, `J`, `W`, `Pa`, `Hz`, `C`, `V`, `Ohm`, `S`, `F`,
  `H`, `T`, `Wb`) or in SI base units (`m/s`, `kg*m^2`).
- Units work in variables and `ans`, in Decimal and Symbolic mode (`x * 2_m` → `(2*x)_m`).
  `sqrt`, `cbrt` and `abs` accept quantities; other functions need dimensionless arguments.
- With `notation(si)`, the prefix moves into simple units: `1500_m` is shown as `1.5_km`.
- Available (`units()` lists them): `m g s A K mol cd N J W Pa Hz C V Ohm S F H T Wb min h d L t
  bar atm eV Wh Ah cal in ft yd mi lb`. SI prefixes `q` … `Q` work on all but
  `min h d t atm in ft yd mi lb`; `u` can be typed for `µ`, `l` for `L`, `Ω` for `Ohm`.
- Temperatures are absolute (kelvin only); °C/°F with their offsets aren't supported.

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

In the variables drawer, sections show the number of variables they contain, for example
`▾ physics (3)`.

| Key (variables)       | Action                                                     |
|-----------------------|------------------------------------------------------------|
| `Enter` `Space`       | Fold or unfold the section under the cursor                |
| `l` `→`               | Unfold a section, or step into an unfolded one             |
| `h` `←`               | Fold a section, or jump to the parent section; at the top level, go to the history |
| `dd` `x` `Del`        | Delete; on a section, deletes the section with everything in it |
| `v` … `Enter`         | Insert the selected variables (a single selected section inserts `physics.`) |
| `Esc`               | Back to the input line                                   |

Counts work as in Vim (`5j`). Several selected history lines are inserted comma-separated, which
is handy as function arguments.

### Settings

| Setting                | Default |
|------------------------|---------|
| Typst rendering        | on      |
| Auto-close brackets    | on      |
| System clipboard       | on      |
| Complex numbers        | off     |
| Decimal places         | auto    |
| Notation               | standard |
| Exponent step          | sci     |
| Extended SI prefixes   | off     |
| Hotkeys                | see above |

Navigate with `j` / `k`, change with `Enter`, `Space` or `←` / `→`, reset a hotkey with `r`.
Settings without effect in the current notation are dimmed. The bottom line shows the matching
command for the selected setting.

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
    core/                 numbers, parser, algebra, formatting, function registry, evaluator,
                          number notation (notation.rs), section paths (names.rs),
                          unit registry and parsing (units.rs),
                          unit arithmetic (quantity.rs)
    editor/               input buffer, completion, Vim editor, list navigation
    settings/             settings model and hotkey map
    history.rs            history model
    variables.rs          tree view of sectioned variables
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
- [X] `log10`
- [X] `log2`
- [X] `cbrt`
- [X] `hypot`
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
- [X] `gcd`
- [X] `lcm`
- [X] `mod`

**Commands**
- [x] `clear`
- [x] `delvar` (recursive for sections with `-r`)
- [x] `settings`, `set`, `notation`, `decimals`, `exponent`, `extprefixes`

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
- [ ] `logb` (arbitrary base)
- [ ] `root`

**Rounding and sign**
- [ ] round_ties_even (toggle)

**Number theory and combinatorics**
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
- [x] units and conversions (`5_N`, `-> km/h`)
- [ ] temperature scales with offsets (°C, °F)
- [ ] unit completion after `_`
- [ ] `clearvars` (delete all variables)
- [ ] `vars` / `help` (list functions)

## Ideas

- [x] Drop the `=` on result lines
- [ ] Rename variables
- [x] Number of decimal places as a setting
- [ ] Rounding mode as a setting
- [ ] Radian / degree mode
- [x] Copy to clipboard in Visual mode
- [ ] Projects (Saveable states) for stored variables
- [x] Sections -> physics.g so there's a tree with organized variables

## Bugs

- [ ] When in autocomplete e.g. frac(<>, <>) the '<>' wont get removed on quick entering (select a value in history, press enter to quickenter)
