# Calculatr

[![Language: Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![UI: Ratatui](https://img.shields.io/badge/UI-Ratatui-blue.svg)](https://ratatui.rs/)
[![Typesetting: Typst](https://img.shields.io/badge/typesetting-Typst-239dad.svg)](https://typst.app/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

A keyboard-driven terminal calculator featuring exact rational arithmetic, a symbolic computer algebra core, live [Typst](https://typst.app) math typesetting, physical unit tracking, and modal Vim-style editing.

---

## Key Features

- **Exact Rational Arithmetic:** Calculations use arbitrary-precision rationals (`BigRational`) internally. Expressions like `0.1 + 0.2` evaluate cleanly to `0.3` (`3/10`) without IEEE 754 binary floating-point rounding noise.
- **Decimal & Symbolic Modes:** Toggle seamlessly between decimal approximations and exact symbolic fractions/expressions (`sqrt(2)`, `2*pi`, `5*x`).
- **Live Typst Math Typesetting:** Expressions and results are compiled and rendered as real mathematical notation directly inside the terminal (via Kitty, Sixel, iTerm2, or WezTerm image protocols), matching your terminal's color palette and history fade.
- **Dimensional Analysis & Units:** Attach SI and common units with `_` (e.g., `9.81_m/s^2 * 2_kg` → `19.62_N`). Supports automatic unit simplification, SI prefixes, dimensional error checking, and explicit conversions via `->` (`72_km/h -> m/s`).
- **Complex Numbers:** Optional support for imaginary unit `i` and complex evaluation (`sqrt(-4)`, `ln(-1)`).
- **Variables, Equations & Sections:** Store values or unevaluated equations (`y := 3x = 2`) and organize variables hierarchically using dot-separated sections (`physics.mech.v0 := 3`), browsable in a foldable tree drawer.
- **Vim-Style Modal Editing:** Full `Normal`, `Insert`, and `Visual` modes with motions, counts, operators (`d`, `c`, `y`), undo history, bracket matching, and system clipboard integration (including OSC 52 fallback over SSH).
- **Smart Snippets & Autocompletion:** Inline ghost text, function signature hints, and `Tab`-navigable parameter placeholders (`frac(<>, <>)`).
- **Persistent State:** History, variables, custom keybindings, and formatting preferences persist automatically across sessions.

---

## Requirements

- **Rust Toolchain:** Stable Rust (Edition 2024; requires Rust 1.85+ with let-chains support, built with 1.97).
- **`chafa` Development Library:** Required by `ratatui-image` for terminal graphics rendering:
  - **Arch Linux:** `sudo pacman -S chafa`
  - **Debian / Ubuntu:** `sudo apt install libchafa-dev`
  - **macOS:** `brew install chafa`
- **Terminal Emulator:** For native Typst math graphics, use a terminal supporting the Kitty graphics protocol, Sixel, or iTerm2 inline images (e.g., Kitty, WezTerm, Ghostty, Foot, iTerm2). Other terminals automatically fall back to Unicode half-block rendering, or Typst rendering can be disabled via `F5`.

---

## Installation & Building

Clone the repository and build the release binary:

```sh
git clone [https://github.com/](https://github.com/)<your-username>/calculatr.git
cd calculatr
cargo build --release

```

Run the application directly:

```sh
cargo run --release

```

To install the binary into your local Cargo bin directory (`~/.cargo/bin`):

```sh
cargo install --path .

```

Run the test suite:

```sh
cargo test

```

---

## Usage & Examples

Type an expression in the input bar and press `Enter` to evaluate. Pressing `Enter` on an empty input repeats the previous calculation.

```text
> 1/4                     0.25            (Decimal)   1/4        (Symbolic)
> 0.1 + 0.2               0.3             (Decimal)   3/10       (Symbolic)
> sqrt(2)                 1.414213562373  (Decimal)   sqrt(2)    (Symbolic)
> 5!                      120
> 25% * 80                20
> x := 0.25               x := 0.25
> eq := 3x = 2            eq := 3*x = 2

```

> **Precision Rules:** In Decimal mode, numbers with up to 15 exact decimal places are displayed without rounding; non-terminating values are displayed with 13 significant digits. Symbolic mode always preserves exact algebraic representations.

### Units & Conversions

Append units to numbers or parenthesized expressions using an underscore (`_`), and convert between compatible dimensions using `->`:

```text
> 5_N                     5_N
> 9.81_m/s^2 * 2_kg       19.62_N
> 3_km + 200_m            3200_m          (or 3.2_km with notation(si))
> (2 + 3)_kOhm * 2_mA     10_V
> 72_km/h -> m/s          20_m/s
> 1_h + 30_min -> min     90_min
> 1_m + 1_s               Error: Incompatible units: m and s

```

* **Syntax:** Unit chains contain no spaces (`5_m/s` is meters per second; `5_m / s` divides by variable `s`). Exponents bind tightly to the unit (`5_m^2` is $5\text{ m}^2$; use `(5_m)^2` for $25\text{ m}^2$).
* **Supported Units:** `m`, `g`, `s`, `A`, `K`, `mol`, `cd`, `N`, `J`, `W`, `Pa`, `Hz`, `C`, `V`, `Ohm` (`Ω`), `S`, `F`, `H`, `T`, `Wb`, `min`, `h`, `d`, `L` (`l`), `t`, `bar`, `atm`, `eV`, `Wh`, `Ah`, `cal`, `in`, `ft`, `yd`, `mi`, `lb`.
* **SI Prefixes:** Full range from `q` ($10^{-30}$) to `Q` ($10^{30}$), with `u` accepted as an alias for `µ`. Run `units()` inside the app to list all available units.

### Variable Sections

Variables can be grouped into nested namespaces using dots (`.`), behaving like files and directories:

```text
> physics.g := 9.81_m/s^2
> physics.mech.v0 := 15_m/s
> delvar(physics.mech, -r)

```

Sections appear as collapsible folders in the Variables drawer (`F3`) and support full-path tab completion.

---

## Number Formatting

Number formatting applies to Decimal mode and re-renders the entire calculation history immediately upon change:

| Setting | Accepted Values | Example (`15000.5`) |
| --- | --- | --- |
| **Decimal places** | `auto`, `0` … `15` | `decimals(0)` → `15001` |
| **Notation** | `standard`, `power`, `e`, `si` | `15000.5` · `1.50005*10^4` · `1.50005e4` · `15.0005k` |
| **Exponent step** | `sci` (1 digit), `eng` (multiples of 3) | `e` + `eng` → `15.0005e3` |
| **Extended SI prefixes** | `on`, `off` (adds `c`, `d`, `da`, `h`) | `0.05` with `extprefixes(on)` → `5c` |

---

## Commands

Built-in commands can be executed directly from the input prompt:

| Command | Description |
| --- | --- |
| `clear()` | Clear the calculation history |
| `delvar(name)` | Delete a single variable |
| `delvar(section, -r)` | Recursively delete a section and all variables inside it |
| `units()` | Display the list of supported units and prefixes |
| `settings()` | Print an overview of all active settings |
| `set(key, value)` | Set an option (e.g., `set(typst, off)`, `set(complex, on)`) |
| `set(key)` | Toggle a boolean option or cycle through available choices |
| `notation(mode)` | Set notation (`standard`, `power`, `e`, `si`, `sci`, `eng`) |
| `decimals(n)` | Set decimal places (`auto` or `0`–`15`) |
| `exponent(step)` | Set exponent step (`sci` or `eng`) |
| `extprefixes(state)` | Toggle extended SI prefixes (`on` or `off`) |

Available keys for `set()`: `typst`, `autoclose`, `clipboard`, `complex`, `decimals`, `notation`, `exponent`, `extprefixes`.

---

## Keybindings & Navigation

### Global Hotkeys

All hotkeys (except `Ctrl+c`) can be customized in the **Settings** tab (`F4`):

| Key | Action |
| --- | --- |
| `F2` | Toggle `Decimal` / `Symbolic` evaluation mode |
| `F3` | Open, focus, or close the Variables drawer |
| `F4` | Switch between `Calculator` and `Settings` tabs |
| `F5` | Toggle Typst math rendering |
| `Ctrl+q` | Quit application |
| `Ctrl+c` | Force quit |

### Input Editor (Vim Modes)

| Mode | Keys & Actions |
| --- | --- |
| **Insert** | Type expression · `Tab` / `Shift+Tab` autocomplete & jump snippets · `Enter` evaluate · `Esc` Normal mode |
| **Normal** | `h` `l` `w` `b` `e` `0` `^` `$` `%` move · `i` `a` `I` `A` insert · `x` `X` `D` `C` `s` `S` `r` `p` `P` `u` edit |
| **Operators** | `d` `c` `y` + motion (`dw`, `c$`, `yy`) with numeric counts (`3x`, `2dw`) · `v` Visual mode |
| **Visual** | Extend selection with motions · `y` yank · `d` / `c` delete/change · `Esc` cancel |

Pressing `k` or `Up` in Normal mode moves focus to the History pane.

### History & Variables Drawers

| Key | Action |
| --- | --- |
| `j` / `k` | Move cursor down / up (supports counts, e.g., `5j`) |
| `gg` / `G` | Jump to top / bottom |
| `Ctrl+d` / `Ctrl+u` | Scroll half-page down / up |
| `v` | Enter Visual selection mode |
| `y` / `yy` | Yank selection to register and system clipboard |
| `d` / `dd` / `x` | Delete entry (or recursively delete section in Variables drawer) |
| `Enter` | Insert selected item(s) into input bar (or fold/unfold section) |
| `h` / `l` | Switch panes, or collapse/expand sections in the Variables tree |
| `/` | Filter variables by name (Variables drawer) |
| `Esc` | Return focus to the input bar |

---

## Configuration & Persistence

Calculatr stores its configuration and session state in your platform's standard config directory under `Calculatr/`:

* **Linux:** `~/.config/Calculatr/`
* **macOS:** `~/Library/Application Support/com.calculatr.Calculatr/`
* **Windows:** `%APPDATA%\calculatr\Calculatr\config\`

| File | Contents |
| --- | --- |
| `settings.json` | Formatting options, behavior toggles, and custom keybindings |
| `state.json` | Defined variables, evaluation mode, and the last 500 history entries |

If a configuration file becomes corrupted, it is automatically backed up to `*.json.bak` and reset to defaults.

---

## License

This project is licensed under the [MIT License](https://www.google.com/search?q=LICENSE).
