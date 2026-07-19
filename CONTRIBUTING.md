# Contributing

## Installation

If you already have a Rust environment set up, you can use the `cargo install` command.

Might have to install Xcode app or Xcode command line tools (run `xcode-select --install`) to get bevy working. Dynamic linking is already configured for bevy in `Cargo.toml`.

## Git Hooks

This project uses [cargo-husky](https://github.com/rhysd/cargo-husky) to manage Git hooks. Hooks are automatically installed when you run `cargo test`.

### Pre-commit

- `cargo fmt --check` — ensures code is properly formatted
- `cargo clippy -- -D warnings` — catches common mistakes and lint issues

### Pre-push

- `bunx cspell` — checks spelling in source files and docs
- `cargo test` — runs the full test suite before pushing

### Running cspell locally

To check spelling manually:

```sh
bunx cspell "**/*.rs" "**/*.toml" "**/*.md" --no-progress
```

Project-specific terms are defined in `cspell.json` at the project root. Add domain-specific words to the `words` array when needed.

## Getting started

Set your XReal display as an extended display (Settings -> Displays -> Use as -> Extended display).

Then launch with `cargo run`, drag the window to the extended display and fullscreen.

## Known Issues

- Jittering - When moving around there is a good amount of jittering of the rendered image.
- Focus - When clicking off of the game window and on something on a different monitor the rendering becomes blurry and input lag is substantially greater.
- Drifting - There is still drift left/right.
- Tracking - The tracking goes wonky when head is tilted close to 90 degrees. Should probably use a different fusion library than `dcmimu`.

## Notes

The goal was to create an [AR desktop](https://www.xreal.com/experience/?virtual-desktop) application as an alternative to XReal's [Nebula](https://www.xreal.com/app/).

I created this small demo with a lot of help from GitHub Copilot. It was my first time writing Rust, so there may be design flaws or errors in the code. Unfortunately, I don't have enough time to continue working on this project and I also lack experience in this field. Therefore I have decided to open source this code in hope that someone more experienced can make something out of it. I have added some notes below of things I learned whist researching and developing this project:

[This](https://kguttag.com/2023/08/05/apple-vision-pro-part-5a-why-monitor-replacement-is-ridiculous/#rendering-a-dot) article goes into great depth explaining the problems with having virtual displays in VR/AR, the main problem being that rendered text uses "hints" to achieve better antialiasing, and when you project the display in virtual space the pixels no longer align properly which results in text that looks grainy and also shimmers. Locking the roll axis would alleviate a lot of the problems with distortion of text. In an ideal scenario you would have a virtual 1080p display be the exact size of the display output so that the pixels line up perfectly with the 1080p display of the glasses.

VITURE's [SpaceWalker](https://www.reddit.com/r/VITURE/comments/1bl72zb/unlock_the_best_of_your_macbook_spacewalker_for/) app has a great implementation of a dropdown UI which should be way easier to implement than slider style settings that Nebula has.

Running as higher priority (`sudo nice -n -10 cargo run`) might reduce input lag. Hard to tell the difference but it feels slightly better.

Might want to consider using `async-hid` instead of `hidapi` for the driver as a way to fix the jitter, although the loop is already running at 1000 Hz so I'm not too sure if that would help.

Experiment with different Hz for fixed schedule of updating the camera. Rendering PreUpdate (before each frame) seems to create more jittering than using a fixed schedule, but results may vary based on your framerate.
