# Relics Reforged

An **Elden Ring Nightreign** mod that takes the one good effect off a bad relic and moves it onto
a relic that will use it, inside Relic Rites, always producing a relic the game itself could have
rolled.

- Pick an effect on a **donor** relic and it replaces an effect on a **target** relic.
- The donor and **3 sacrifice relics** are destroyed, and you pay **murk**.
- The result is always a combination **the game could have rolled**: the mod never creates
  impossible relics, it only cuts down the RNG needed to get a good one.

Works offline and online through mods with their own connection (Seamless Co-op, the More Map
Variations server). **Never use it on FromSoftware's official servers.**

Download, installation and the full description are on Nexus Mods
([`docs/nexus/descripcion.bbcode`](docs/nexus/descripcion.bbcode) has the same text); the
end-user readme is [`dist/README.txt`](dist/README.txt). Released under the
[MIT license](LICENSE).

The code comments and the research notes ([`docs/relevamiento.md`](docs/relevamiento.md)) are in
Spanish. The code and the texts were written with Claude under the author's
direction; the design, balance and in-game testing are the author's.

## Status

All of this is tested in game unless noted. Details of each test are in
[`docs/relevamiento.md`](docs/relevamiento.md).

| Feature | Status |
|---|---|
| Forge: move an effect between relics, with sacrifices, murk and forge limits | ✔ |
| Only legal relics (same slot tables, exclusive effects, curses and sort order as the game) | ✔ |
| Favorites and relics in use can't be sacrificed; as donors they are removed from where they were | ✔ vessels; equipped and preset donors not tested yet |
| In-game menu in Relic Rites (F7 / R2), in the game's style, with its icons and sounds | ✔ |
| Search, filter (including by effect) and sorting | ✔ |
| Mouse, keyboard and controller; the game gets no input while the forge is open | ✔ PlayStation controller through Steam Input; Xbox not tested |
| English and Spanish, following the game's language | ✔ (Spanish button position in the controls bar not seen in game yet) |
| Settings file (`RelicsReforged\relics_reforged.ini`) | ✔ |
| Save backup before the first forge of each session; forge record in `%APPDATA%\RelicsReforged` | ✔ |
| Notice when the game version isn't supported (the mod turns itself off) | ✔ |
| Seamless Co-op, also with players without the mod | ✔ |
| Mods that change relics through their own `regulation.bin` (More Map Variations) | ✔ offline and on its online server |
| Install with ME3 Manager, the player's own me3 profile or the standalone profile | ✔ |

## Layout

| Path | Contents |
|---|---|
| `crates/relic-rules/` | Rules engine in Rust: relic and transfer legality. No access to the game |
| `crates/relics-reforged/` | The DLL loaded by me3. Writes `relics_reforged.log` and `relics_reforged.ini` in the `RelicsReforged\` folder next to the DLL; in development it also accepts test commands in `relics_reforged_cmd.txt`, next to the DLL |
| `crates/nr-assets/` | Reads single files from the game's `data*.bhd/.bdt` (RSA, AES, Oodle, TPF, BC7) to get the menu icons. `cargo run -p nr-assets --example icons` builds the atlas outside the game |
| `crates/re-tools/` | `disasm`: disassembler for the `nightreign.exe` dump (the DLL's `dumpimage` command) |
| `me3/relics-reforged-dev.me3` | me3 profile for testing, with a separate save (`NR_RelicsReforged.sl2`). `relics-reforged-seamless.me3`: the same with Seamless |
| `docs/relevamiento.md` | Everything found out: save format, params, pointers, rules |
| `docs/nexus/` | Nexus Mods page: BBCode description (`descripcion.bbcode`) and a sheet with the summary, files, changelog, screenshots and the pre-release checklist (`ficha.md`) |
| `dist/` | What goes in the release besides the DLL: the end-user `README.txt` and, in `standalone/`, the `RelicsReforged.me3` profile of the optional file (offline, with a save copy) |
| `tools/package.ps1` | Builds the release: compiles the public DLL in `target/public` and leaves the folders and zips in `target/dist` |
| `tools/` | Analysis scripts (Python/PowerShell) used in the research |
| `data/` | Local data, **not in git**: game CSVs, the player's save, Cheat Engine files |

## Development

Requirements: Rust (toolchain `stable-x86_64-pc-windows-msvc`) and MSVC Build Tools.

```bash
cargo test -p relic-rules
```

The tests in `tests/real_data.rs` use real data when it exists:
- `data/export/`: `AttachEffectParam.csv`, `AttachEffectTableParam.csv` and `EquipParamAntique.csv`, exported with Smithbox (all rows and fields).
- `data/save/relics.json`: generated with `tools/decrypt_sl2.ps1` + `tools/relic_survey.py` from a save.

If they're missing, those tests are skipped and pass.

There are two builds of the DLL:

- **Public** (the one that is released): only the forge, the menu and its log.

  ```bash
  cargo build --release -p relics-reforged
  ```

- **Development** (`dev` feature): adds the command file `relics_reforged_cmd.txt` (reads and writes game memory), the tracer, the profiler, the memory search, the input diagnostics, HID controller reading (unconfirmed), names from `data/export` and the full inventory in the log. **Never released.**

  ```bash
  cargo build --release -p relics-reforged --features dev
  ```

Both end up in `target/release/relics_reforged.dll` (whichever was built last). To release: `powershell -File tools/package.ps1` (builds the public DLL separately, in `target/public`, and leaves in `target/dist` the main zip `RelicsReforged-v<version>.zip` and the optional `RelicsReforged-Standalone-v<version>.zip`; in the release the DLL is named `RelicsReforged.dll`). To test, double-click `me3/relics-reforged-dev.me3`. The log ends up in `target/release/RelicsReforged/relics_reforged.log`.

The menu and its messages are in English or Spanish depending on the game's language (English by default; see `relic_rules::lang`). In game, with **Relic Rites** open at the Roundtable, **F7** (or a click on "F7 :Forge relics") opens the full-screen menu: on the left, the target and the effect to replace, the donor and the effect to move, 3 sacrifices, the preview and the forge button (with double confirmation); on the right, the relic list for the current step, with each relic's image and each effect's icon. Q (or F7, or Esc) goes back to Relic Rites; R refreshes and C starts over. Above the list: search, 4 opens the filter (in the left panel, with up to 3 effect conditions) and 5 changes the sorting. While the forge is open, the game gets no keyboard, mouse or controller input. In the development build, the command file (`crates/relics-reforged/src/commands.rs`) allows testing without the menu.

With keyboard or controller, a cursor moves over buttons and rows: up/down within a panel, left/right switches panels, Enter/Space or A selects. In the effect picker, A or right opens a group. The controls show keys or buttons depending on what was used last.

| Action | Keyboard | PlayStation | Xbox |
|---|---|---|---|
| Open / close the forge (in Relic Rites) | F7 | R2 | RT |
| Select | Enter / Space | Cross | A |
| Back | Q or Esc | Circle | B |
| Refresh / Start over | R / C | Options / Triangle | Menu / Y |
| Filter / Sort | 4 / 5 | L3 / R3 | LS / RS |
| In the filter: Clear / Remove conditions | 1 / 2 | Square / Triangle | X / Y |
| Clear the search | 2 | Square | X |

## Balance rules (defaults)

They live in `Rules` (`crates/relic-rules/src/transfer.rs`). Players change them in `RelicsReforged\relics_reforged.ini` (section `[Rules]`; see `crates/relics-reforged/src/config.rs`, which also has the default file).

| Rule | Value |
|---|---|
| Sacrifices per transfer | 3 (small = 1 point, medium = 2, large = 3) |
| Minimum points (target with 1 / 2 / 3 effects) | 3 / 5 / 7 |
| Murk (target with 1 / 2 / 3 effects) | 2,000 / 4,000 / 7,000, ×1.5 for Deep relics (3,000 / 6,000 / 10,500); the multiplier accepts decimals |
| Forges per relic (1 / 2 / 3 effects); re-forging an already forged effect also counts. 0 = no limit | 1 / 1 / 2 |
