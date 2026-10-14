# Arsenal

**ARma Server ENvironment App Launcher** - a desktop launcher for an Arma Reforger
dedicated server (Windows and Linux). It edits the server's JSON config, builds the
launch parameters, starts and stops the server, and shows its logs and CPU/RAM use.

## Build

```
cargo build --release
```

The program is `target/release/arsenal` (`arsenal.exe` on Windows).

## Disclaimer

Arsenal is an unofficial, independent tool. It is not affiliated with, endorsed by or
supported by Bohemia Interactive. "Arma" and "Arma Reforger" are trademarks of their
owners.

Use it at your own risk. It starts and stops server processes and writes config files on
your computer. Back up your server config and save data before you use it. The software is
provided "as is", without warranty of any kind; see [LICENSE](LICENSE).

## License

MIT - see [LICENSE](LICENSE). Arsenal uses open-source libraries (egui, eframe,
egui_dock, serde, sysinfo and others) that have their own licenses, mostly MIT or
Apache-2.0. If you distribute a built binary, include their copyright notices (for
example by running `cargo about`).
