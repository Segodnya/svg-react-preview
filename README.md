# svg-react-preview

CLI for previewing inline JSX/TSX SVG fragments in the **Zed** editor.

Zed has a native preview for `.svg` files but no way to open SVG defined inline inside JSX. Place the cursor anywhere inside an `<svg>…</svg>` element and run the task — the tool parses the file, finds the enclosing `<svg>`, normalises it into a valid `.svg`, writes it to a temporary directory, and opens it with `zed <path>` so Zed shows the result in a new tab using its built-in SVG preview.

## Install

```bash
cargo install --git https://github.com/Segodnya/svg-react-preview
```

The binary lands at `~/.cargo/bin/svg-react-preview` (must be on `PATH`).

You also need a `zed` shim on `PATH` (Zed → CLI → Install Shell Command). Without it, the preview is still saved to `$TMPDIR/svg-react-preview/` and the path is printed to stderr.

## Zed configuration

Add the task to `~/.config/zed/tasks.json`:

```jsonc
[
  {
    "label": "Preview SVG (cursor)",
    "command": "svg-react-preview",
    "env": {
      "SVG_REACT_PREVIEW_FILE":   "${ZED_FILE}",
      "SVG_REACT_PREVIEW_ROW":    "${ZED_ROW}",
      "SVG_REACT_PREVIEW_COLUMN": "${ZED_COLUMN}"
    },
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "no_focus",
    "hide": "on_success",
    "save": "none"
  }
]
```

Optional keybinding in `~/.config/zed/keymap.json`:

```jsonc
[
  {
    "context": "Editor",
    "bindings": {
      "cmd-shift-v": ["task::Spawn", { "task_name": "Preview SVG (cursor)" }]
    }
  }
]
```

## Usage

1. Place the cursor anywhere inside an `<svg>…</svg>` element in a `.tsx` / `.jsx` file — no selection needed. The tool parses the whole file with swc and finds the innermost enclosing `<svg>`.
2. Run the task (via keybinding or `task: spawn`).
3. A new tab opens with the preview.

If the cursor is not inside an `<svg>` element, the tool prints a clear error to stderr (`cursor at <file>:<row>:<col> is not inside an <svg> element`).

### Alternative input modes

- `SVG_REACT_PREVIEW_INPUT="<svg>…</svg>"` — pass the source directly via env (useful for CI or custom Zed tasks built around `${ZED_SELECTED_TEXT}`).
- Pipe via stdin: `echo '<path d="M0 0"/>' | svg-react-preview`.

Precedence: `SVG_REACT_PREVIEW_FILE`+`ROW`+`COLUMN` → `SVG_REACT_PREVIEW_INPUT` → stdin.

## Behaviour

- `<Icon/>` (PascalCase, unresolved component) → placeholder `<rect/>` with a dashed border + warning on stderr.
- `{cond && <path/>}` → renders the right-hand side.
- `{a ? <path/> : <circle/>}` → renders the first branch.
- `{...props}` is dropped.
- Dynamic values (`{size}`, `{props.color}`) are replaced by defaults (24, currentColor, …) or the attribute is skipped.
- `className` → `class`, `strokeWidth` → `stroke-width`, `xlinkHref` → `xlink:href` (with `xmlns:xlink` auto-added).
- `onClick`, `onMouseEnter`, `htmlFor`, `dangerouslySetInnerHTML` → dropped.
- If the root is not `<svg>` or there are multiple roots, the output is wrapped in `<svg viewBox="0 0 24 24">`.

## Development

```bash
make test            # run unit + integration + CLI tests
make verify          # full pre-commit gate (fmt + clippy pedantic + test + deny + machete)
make install         # install svg-react-preview into ~/.cargo/bin
make help            # list all available targets
```

Previews are written to `$TMPDIR/svg-react-preview/<xxhash>.svg` (the file name is a stable hash of the source — re-opening the same fragment doesn't create new files).

### Toolchain

`rust-toolchain.toml` pins **Rust 1.95.0** + components `clippy`, `rustfmt`, `llvm-tools-preview`. With `rustup` installed the right toolchain is selected automatically; on Homebrew Rust (no rustup) keep your local Rust at 1.95.x and the Makefile falls back to `/opt/homebrew/opt/llvm/bin/llvm-{cov,profdata}` for coverage (override via `LLVM_COV` / `LLVM_PROFDATA`).

### Quality gates (CI)

The `.github/workflows/` set runs on every push to `main` and every PR targeting `main`:

- **`ci.yml`** — Linux, macOS, Windows
  - `cargo fmt --check`
  - `cargo clippy` (pedantic + nursery, `-D warnings`)
  - `cargo test`
  - `cargo build --release`
  - `cargo doc` (`-D warnings`)
- **`coverage.yml`**
  - `cargo llvm-cov` → Codecov
  - fails if line coverage < 95%
- **`security.yml`**
  - `cargo deny check all` (advisories + bans + licenses + sources)
  - `cargo audit` (RustSec)
- **`quality.yml`**
  - `cargo machete` (unused deps, stable)
  - `cargo udeps` (unused deps, nightly)
  - `similarity-rs` (duplicate code)
  - `cargo geiger --forbid-only` (unsafe stats)
  - `cargo outdated` (newer dependency versions)
  - `cargo msrv verify` (pinned MSRV still compiles)
  - `cargo mutants` (mutation testing)

All jobs are blocking. To reproduce locally install the tools once:

```bash
cargo install --locked cargo-deny cargo-machete cargo-llvm-cov cargo-mutants cargo-audit \
              cargo-geiger cargo-outdated cargo-msrv similarity-rs
rustup toolchain install nightly && cargo install --locked cargo-udeps
```

### Pre-commit / pre-push hooks (cargo-husky)

Hooks are committed under `.cargo-husky/hooks/` and installed automatically by `cargo test` (the `cargo-husky` build script copies them into `.git/hooks/`). After a fresh clone run `cargo test` once.

- **pre-commit** — `cargo fmt --check`, clippy pedantic+nursery, `cargo test`, `cargo deny check`, `cargo machete`.
- **pre-push** — `make coverage-gate` (fails below 95% line coverage).

Both hooks fail with an explicit "install X" hint when a tool is missing.

### Test coverage

```bash
make coverage        # per-file summary + uncovered line numbers
make coverage-html   # HTML report at target/llvm-cov/html/index.html
make coverage-gate   # fail if line coverage drops below 95% (override: COVERAGE_MIN=…)
```

## License

MIT
