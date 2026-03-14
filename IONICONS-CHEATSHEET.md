# Ionicons v2 Cheat Sheet for This Repo

This project uses the `Ionicons` v2 font file at:

- `assets/fonts/ionicons.ttf`

The Rust helper constants live at:

- `src/icons.rs`

The goal of this file is to stop future agents from guessing Ionicons codepoints.

## Source of truth

Use the official Ionicons v2 references:

- Site: <https://ionic.io/ionicons/v2>
- Official CSS map: <https://code.ionicframework.com/ionicons/2.0.1/css/ionicons.css>

If you need a new icon, resolve it from the official CSS and then store it in `src/icons.rs`.
Do not guess from memory.

## Verified mapping already used in this repo

These mappings were checked against the official Ionicons v2 CSS.

| Rust const | Unicode | Official class | Typical use in this repo |
| --- | --- | --- | --- |
| `HELP` | `\u{F142}` | `ion-help-circled` | Help button |
| `SETTINGS` | `\u{F43C}` | `ion-ios-gear-outline` | Settings button / settings card |
| `CLOSE` | `\u{F404}` | `ion-ios-close-empty` | Close button |
| `BACK` | `\u{F3CF}` | `ion-ios-arrow-back` | Back button |
| `ADD` | `\u{F2C7}` | `ion-android-add` | Add images |
| `DOCUMENT` | `\u{F12F}` | `ion-document` | Files module icon |
| `FOLDER` | `\u{F139}` | `ion-folder` | Folder / output button |
| `IMAGE` | `\u{F147}` | `ion-image` | Single image placeholder |
| `IMAGES` | `\u{F148}` | `ion-images` | Photos / multiple images |
| `VIDEO` | `\u{F256}` | `ion-videocamera` | Video module icon |
| `ARCHIVE` | `\u{F102}` | `ion-archive` | Archive / extract module icon |
| `PLAY` | `\u{F488}` | `ion-ios-play` | Primary compress action |
| `TRASH` | `\u{F252}` | `ion-trash-a` | Delete item from queue |
| `ZOOM_IN` | `\u{F48B}` | `ion-ios-plus` | Image preview zoom in |
| `ZOOM_OUT` | `\u{F464}` | `ion-ios-minus` | Image preview zoom out |

## How to add a new icon safely

1. Open the official CSS map.
2. Find the exact class, for example `.ion-ios-play:before`.
3. Copy the `content: "\f488"` codepoint.
4. Store it in Rust as a char escape, for example `'\u{F488}'`.
5. Give the constant a semantic name in `src/icons.rs`.
6. If the icon is used as an icon-only control, prefer `icons::rich(...)` or `icons::font_id(...)`.

## Usage pattern in this repo

The font family is registered under the custom family name `ionicons`.

Examples:

```rust
pub const PLAY: char = '\u{F488}';

Button::new(
    RichText::new(format!("{} Compress", icons::PLAY))
        .size(13.0)
        .strong(),
)
```

```rust
ui.painter().text(
    rect.center(),
    egui::Align2::CENTER_CENTER,
    icons::HELP,
    icons::font_id(15.0),
    color,
);
```

## Repo-specific note

This repo also uses `assets/fonts/icon/icon.svg` for the application logo/window icon.
That SVG mapping is separate from the Ionicons font mapping.
