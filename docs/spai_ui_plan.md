# Plán: zjednodušení UI SPAI Notes (zápis, editor, dedup, režimy)

## Cíl
Dialog zápisu má dnes 4 plochy najednou (vstup 5 řádků, seznam typů, panel „Kontext“ s dedup kartou, patička) a dedup sám volá OpenRouter 2,2 s po posledním stisku. Cíl: minimum prvků, celostránkový editor, dedup na zkratku a Vim‑like režimy Normal/Insert.

Dotčená slice: `plugins/pi-herdr-sidebar/src/slices/spai_notes/` + `src/slices/view/keys/dialogs.rs`. Před kódem: načíst skill `herdr-plugin-dev` + Ratatui KB (`knowledge_base_kb_search`, kolekce `ratatui`). Nic se nemění v manifestu.

## Rozhodnutí (z vašich odpovědí)
| Téma | Rozhodnutí |
|---|---|
| Layout zápisu | Jen editor přes celou plochu + patička. Typ = prefix na začátku textu, projekt = `@projekt` v textu. Žádný postranní seznam ani panel. |
| Nápověda | Prázdný vstup → v ploše editoru (tlumeně) kompletní seznam typů + syntaxe. Jakmile je typ vybrán/napsán → nápověda **jen k tomu typu** v patičce (1–2 řádky). |
| Dedup | `Ctrl+D` na vyžádání (lokální + vektorová). Výsledek v překryvném panelu, `Ctrl+O/A/U` jako dnes. Automatický timer a lokální „fallback“ karta mizí. |
| Režimy | Pouze Notes: Normal (výchozí při otevření záznamu) / Insert. Režim v patičce. |

## Můj názor na režimy
Dobrý nápad a levný: kód už má kurzor po znacích, `move_line`, `line_bounds`, scroll. Podmínky: (1) vždy viditelný režim v patičce, (2) malá sada příkazů, žádný „plný Vim“, (3) `Esc` v Insert → Normal, `Esc`/`q` v Normal → zavřít (u změn zeptat se), (4) přidávání příkazů jen když jsou potřeba. Riziko: kolize s dnešním `Esc = zavřít` a `Ctrl+S`; řešeno tabulkou níže.

## Návrh

### 1. Zápis (creation) – jedna plocha
```
┌ ✍ ▸ Todo ────────────────────────── (barva typu, jen titulek) ┐
│ . Napsat review @herdr !vysoká :ui:                            │
│                                                                │
│  (prázdné → tlumený seznam typů + syntaxe)                     │
└────────────────────────────────────────────────────────────────┘
 Todo: úkol k udělání · př.: `. Opravit build @herdr`
 Enter uložit · Ctrl+D duplicity · ↑↓ typ (při prázdném) · Esc zavřít
```
- `creation.rs` zredukovat na `rows = [Min(3), Length(2)]`; smazat levý sloupec a pravý panel.
- `creation_parts.rs`: `build_hint_lines` rozdělit na `build_type_overview()` (prázdný vstup) a `build_type_footer(opt)` (vybraný typ). Dedup karta se přesune do `dedup_overlay.rs` [NEW].
- Typ se detekuje z prefixu (`detect_spai_input`, už existuje) → patička se mění automaticky, `Tab`/`↑↓` jen při prázdném vstupu vloží prefix.
- Při vstupu bez `Ctrl+D` se nespouští žádné síťové volání.

### 2. Dedup na zkratku
- [MODIFY] `state_similar_actions.rs`: `poll_debounced_dedup` → `run_dedup(api_key, model, threshold)` volané z `Ctrl+D`; poll jen přebírá výsledek z `dedup_receiver`. Odstranit `last_keystroke`/2200 ms logiku a z `dialog_state.rs` pole `last_keystroke`, `debounced_query`, `vector_evaluated`.
- [MODIFY] `keys/dialogs.rs`: `Ctrl+D` spustí kontrolu; `Ctrl+O/A/U` fungují jen při zobrazeném panelu; `Esc` panel zavře.
- Bez API klíče: jen lokální textová + uložené vektory (jako dnešní fallback), bez chyby.

### 3. Režimy Normal / Insert (jen Notes)
Jeden `NoteEditor` pro čtení i úpravu. Sjednocuji pole: **1. řádek = název, zbytek = tělo** (dnes dvě pole + Tab). Zápis se ukládá stejným `format_spai_markdown`.

| Normal | Akce |
|---|---|
| `j/k`, `↑/↓`, `PgUp/PgDn`, `g/G` | posun / scroll |
| `i`, `e` | Insert na kurzoru; `A` na konec řádku |
| `x` | cyklus stavu (dnes `cycle_selected_status`) |
| `d` pak `Delete`/`y` | smazání s potvrzením (dnešní dvoukrok) |
| `Ctrl+D` | dedup pro tento záznam |
| `Ctrl+S` | uložit (funguje v obou režimech) |
| `Ctrl+E` | externí editor |
| `Esc`/`q` | zavřít (u neuložených změn potvrdit) |

Insert: dnešní chování editoru (`on_edit_*`), `Esc` → Normal. Patička: `-- NORMAL --` / `-- INSERT --` + kontextové zkratky.

- [MODIFY] `dialog_state.rs`: `NoteEditDialog` + `mode: EditMode`, `dirty: bool`; odstranit `EditField`.
- [MODIFY] `state_edit.rs` (188 ř.) → rozdělit na `state_edit/{mod,insert,normal}.rs` (limit 300 ř.).
- [MODIFY] `dialog_views/edit.rs`: celá plocha (`area`, ne `centered_percent`), jedna `Paragraph` + patička.
- [MODIFY] `keys/dialogs.rs`: větev `edit_dialog.active` podle `mode`; soubor rozdělit na `keys/{creation,edit}.rs`.

### 4. Související oprava (z review)
Prefixy jsou ve 4 seznamech a ořezávají první písmeno („hello“ → „ello“). Při přepisu detekce prefixu zavést **jednu** tabulku v `type_options.rs` a prefix uznávat jen s mezerou. Zařazuji do této změny, protože detekce typu je teď jádro UI.

## Rozhodnuto (aktualizace 2026-10-05)
- **Název a tělo zůstávají oddělená pole** (jako dnes, `Tab` přepíná). Žádné slučování do jednoho bufferu; formát souboru a `submit_edit_dialog` se nemění.
- **`Enter` zůstává beze změny** (dnešní chování na položce i v dialogu zápisu). Normal/Insert se otevírá jako dnes přes `e`, ne přes `Enter`.
- Zápis (creation) zůstává jednořádkový (jen název), jako dnes. Tělo se doplňuje v editoru.

### Dopad na režimy (sekce 3, upřesnění)
- Editor je celostránkový se dvěma poli: Název (1 řádek nahoře) a Tělo (zbytek plochy) + patička.
- **Insert**: dnešní chování (`Tab` přepíná pole, `Enter` v názvu → tělo, `Enter` v těle = nový řádek).
- **Normal**: `Tab` přepíná pole, `j/k`/`↑↓` pohyb po řádcích těla, `i`/`a`/`A` vstup do Insert v aktivním poli, `x` cyklus stavu, `Ctrl+S` uložit, `Ctrl+E` externí editor, `Esc`/`q` zavřít (dirty → potvrdit).
- `EditField` zůstává; přidá se jen `mode: EditMode` a `dirty: bool`.

## Otevřené otázky
> [!NOTE]
> Žádné blokující. Jediný předpoklad: Normal je výchozí režim po `e` (otevření ke čtení), Insert se zapíná `i`. Pokud chcete po `e` rovnou Insert, řekněte.

## Verifikace
- `cargo test` (nové testy: detekce prefixu bez ořezu, Ctrl+D neběží automaticky, přepínání Normal↔Insert, dirty‑guard při Esc, patička se mění podle typu).
- `#[ignore]` dev‑preview testy tisknoucí snímek: prázdný zápis, zápis s typem, dedup panel, editor Normal/Insert (`cargo test <name> -- --ignored --nocapture`).
- Živě: zavřít pane, `cargo build --release`, `herdr pane read <id> --source visible`.
- Commit + push v `plugins/pi-herdr-sidebar` ve stejném tahu.
