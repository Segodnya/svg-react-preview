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
make test       # run golden fixtures in tests/fixtures/
make install    # install svg-react-preview into ~/.cargo/bin
make help       # list all available targets
```

Previews are written to `$TMPDIR/svg-react-preview/<xxhash>.svg` (the file name is a stable hash of the source — re-opening the same fragment doesn't create new files).

## License

MIT
