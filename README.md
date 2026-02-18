# MDD-UI - Diagnostic Database Viewer

A terminal-based user interface for browsing and exploring ODX (Open Diagnostic Data Exchange) diagnostic databases. Built with Rust and [ratatui](https://ratatui.rs/), this tool provides an efficient way to navigate ECU diagnostic services, parameters, and metadata.

**DISCLAIMER** 
This tool is 99% vibe coded, and this is pretty visible in the codebase, anyhow it works, which is good enough for now.
This project is in early development and may contain bugs or incomplete features. 
Use at your own risk. Contributions and feedback are welcome to help improve the tool.

## Features

<img src="img/demo.png">

### 🎯 Core Functionality
- **Tree Navigation**: Browse ECU structure, variants, and diagnostic services
- **Detail Panes**: View comprehensive information with table-based formatting
- **Multi-Section Views**: Separate panes for Request, Positive Response, and Negative Response
- **Search**: Quick search through the tree structure (`/` to search, `n/N` for next/previous)
- **Keyboard-First**: Vim-style navigation (h/j/k/l) with full keyboard control

### 📊 Service Information Display
- Service ID and Sub-function ID with proper formatting
- Request and response parameters with full details
- Parameter types, positions, coded values, and semantics
- Data operation (DOP) information
- Visual separation with color-coded headers

### 🎨 Visual Features
- **Row-Level Navigation**: Select individual table rows with j/k
- **Highlighted Selection**: Dark gray background for selected rows
- **Parameter Headers**: Yellow + bold styling for easy identification
- **Visual Separators**: Horizontal lines between parameter groups
- **Proper Alignment**: Two-column table layout (40% / 60%)
- **Scrollbars**: Indicators when content exceeds viewport

## Installation

### Prerequisites
- Rust 1.70 or later
- Access to ODX diagnostic database files

### Build from Source
```bash
git clone <repository-url>
cd mdd-ui
cargo build --release
```

The binary will be available at `target/release/mdd-ui`.

## Usage

```bash
mdd-ui <path-to-odx-database>
```

### Keyboard Controls

#### Tree Navigation (Default Focus)
| Key | Action |
|-----|--------|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `h` / `Left` | Collapse node or go to parent |
| `l` / `Right` | Expand node |
| `Space` | Toggle expand/collapse |
| `e` | Expand all nodes |
| `c` | Collapse all nodes |
| `Home` | Jump to first node |
| `End` | Jump to last node |
| `PageUp` / `PageDown` | Page through nodes |

#### Detail Pane Navigation (When Focused)
| Key | Action |
|-----|--------|
| `j` / `Down` | Move to next row |
| `k` / `Up` | Move to previous row |
| `h` / `Left` | Switch to previous section |
| `l` / `Right` | Switch to next section |
| `Home` | Jump to first row in section |
| `End` | Jump to last row in section |
| `PageUp` / `PageDown` | Page through rows |

#### General
| Key | Action |
|-----|--------|
| `Tab` | Toggle focus between tree and detail pane |
| `/` | Start search |
| `n` | Next search match |
| `N` | Previous search match |
| `q` / `Esc` | Quit |
| `Ctrl+C` | Force quit |

## Structure

The UI is organized into three main areas:

### Tree View
- **ECU Level**: Top-level ECU information
- **Services**: Diagnostic services with SID and sub-function
- **Variants**: ECU variants (base and specific)
- **State Charts**: State machines and transitions
- **Jobs**: Single ECU jobs and their parameters

### Detail Panes
When a service is selected, the detail pane splits into multiple sections:
- **Service Details**: Overall service information
- **Request**: Request parameters with types and values
- **Positive Responses**: Success response parameters
- **Negative Responses**: Error response parameters

Each section can be navigated independently using `h/l` keys, and rows within sections can be selected with `j/k`.

## Color Scheme

| Element | Color | Style |
|---------|-------|-------|
| Parameter Headers | Yellow | Bold |
| Regular Headers | White | Bold |
| Property Keys | Cyan | - |
| Property Values | Gray | - |
| Selected Row | White on Dark Gray | - |
| Focused Border | Cyan | - |
| Unfocused Border | Dark Gray | - |
| Separators | Dark Gray | - |

## Technical Details

### Dependencies
- **ratatui**: Terminal UI framework
- **crossterm**: Terminal manipulation
- **cda-database**: ODX database parsing and access
- **anyhow**: Error handling

### Architecture
- **Event-driven**: Responsive keyboard input handling
- **Stateful**: Maintains tree expansion, cursor positions, and scroll states
- **Efficient**: Only renders visible content
- **Flexible**: Table-based layout adapts to terminal size

## Development

### Building
```bash
cargo build
```

### Running in Development
```bash
cargo run -- <path-to-odx-database>
```

### Testing
```bash
cargo test
```

## Contributing

Contributions are welcome! Areas for improvement:
- Additional detail views for other diagnostic elements
- Adding ci cd from opensovd/cicd-workflows
- clippy/fmt

## License

MIT

## Credits

Built with:
- [ratatui](https://ratatui.rs/) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [cda-database](https://github.com/eclipse-opensovd/classic-diagnostic-adapter) - ODX database library
