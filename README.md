# Bevy XReal AR demo

A simple demo using [bevy](https://github.com/bevyengine/bevy) and [ar-drivers-rs](https://github.com/badicsalex/ar-drivers-rs) to showcase camera movement in a 3D world. Based on [this](https://bevyengine.org/examples/3D%20Rendering/generate-custom-mesh/) bevy example.

Should work with XREAL Air, Air 2, and Air 2 Pro. Also probably only works on Mac OS (Windows doesn't work, see [issue](https://github.com/badicsalex/ar-drivers-rs/issues/13)).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, known issues, and development notes.

## Useful libraries

- [knoll](https://github.com/gawashburn/knoll) - Tool (written in rust) for manipulating the configuration of macOS displays.

- [ScreenCaptureKit](https://github.com/svtlabs/screencapturekit-rs) - A high-performance screen capture (rust) framework for macOS applications.

- [ar-drivers-rs](https://github.com/badicsalex/ar-drivers-rs) - AR driver library for rust.

- [async-hid](https://github.com/sidit77/async-hid) - A rust library for asynchronously interacting with HID devices.

## Useful resources

- [Bevy cheatbook](https://bevy-cheatbook.github.io/)
- [XReal IMU/MCU protocol writeup](https://voidcomputing.hu/blog/worse-better-prettier/#the-prettier-xreal-air)
- [Spreadsheet of FOVs for most AR glasses](https://docs.google.com/spreadsheets/d/1_Af6j8Qxzl3MSHf0qfpjHM9PA-NdxAzxujZesZUyBs0/htmlview)
- [Bevy OpenXR implementation](https://github.com/awtterpip/bevy_oxr)
- [List of XReal compatible apps and drivers](https://github.com/jakedowns/xreal-webxr?tab=readme-ov-file#projects-using-open-source-xreal-drivers)

## License

Licensed under the MIT license
