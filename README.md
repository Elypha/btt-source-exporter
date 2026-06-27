# btt-source-exporter

Exports dialogue source bundles for [BilingualTooltips](https://github.com/Elypha/BilingualTooltips).

## Setup

```powershell
git clone --recurse-submodules https://github.com/Elypha/btt-source-exporter
cd btt-source-exporter
cargo check
Push-Location builder
npm install
Pop-Location
```

To refresh schemas:

```powershell
git submodule update --remote
```

## Export

### (1/2) Export Dialogue Source Bundles

Supported languages:

```text
ja,en,de,fr,zh-Hans,zh-Hant,ko
```

Pass the client install root that contains the `game` folder. By default, files are written to `output`. Use `--output <path>` to change it.

```powershell
cargo run -- "<install-root-containing-game>" --languages ja,en --output "another_output"
```

Each language produces a `<language>.bttsrc.tar.zst`, and `diagnostics.json` is also written (append) to the output folder. Keep it with the archives when sharing exports. Example output:

```
output/
    ja.bttsrc.tar.zst
    en.bttsrc.tar.zst
    ...
    diagnostics.json
```

### (2/2) Build Dialogue Package

```powershell
npm --prefix builder run build -- --source-root output --output dist --build-number 1 --game-version "2026.06.18.0000.0000"
```

See `builder/REVIEW.md` for the external-review reading map.

## Development

**Export Individual Sheets:** For investigation only. Omit `--sheets` for release-quality exports.

```powershell
cargo run -- "<install-root-containing-game>" --languages ja --sheets DefaultTalk
```
