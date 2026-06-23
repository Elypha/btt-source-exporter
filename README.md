# btt-source-exporter

This tool exports client-derived dialogue source bundles for the [BilingualTooltips](https://github.com/Elypha/BilingualTooltips) Dalamud plugin.

It reads a local Final Fantasy XIV client through [ironworks](https://github.com/ackwell/ironworks) and [EXDSchema](https://github.com/xivdev/EXDSchema). It does not build the final plugin database. It only produces per-language source bundles that the BilingualTooltips package builder can merge later.

## Requirements

- Rust 1.87 or newer
- A local Final Fantasy XIV client for the language you want to export. The path passed to the tool must be the client install root that contains the `game` folder.
- This repository cloned with submodules

## Setup

```powershell
git clone --recurse-submodules https://github.com/Elypha/btt-source-exporter
cd btt-source-exporter
cargo check
```

If schemas need to be refreshed:

```powershell
git submodule update --remote
```

## Export Dialogue Source Bundles

Supported codes are:

- `ja`
- `en`
- `de`
- `fr`
- `zh-Hans`
- `zh-Hant`
- `ko`

International Japanese, English, German, and French come from the international client. Simplified Chinese, Traditional Chinese, and Korean are three separate regional clients and must be exported from their own client installs. Every client uses the same CLI path rule: pass the install root that contains the `game` folder.

Run the exporter with one client install root, an output directory, and the client languages to export from that client:

```powershell
cargo run -- "<install-root-containing-game>" --btt-dialogue --output output-btt-source --languages ja,en
```

Then, each selected language will write one archive:

```text
<language>.bttsrc.tar.zst
```

The output root also contains `diagnostics.json`. Please keep the source archives and diagnostics together when sharing an export for review.

## Export Individual Sheets

To export only specific sheets for investigation, pass `--sheets`:

```powershell
cargo run -- "<install-root-containing-game>" --btt-dialogue --output output-btt-source --languages ja --sheets DefaultTalk
```

Explicit-sheet output is only for local investigation. Release-quality source sets should use the default scope, which is selected by omitting `--sheets`.
