# Personal Knowledge OS — Complete Guide

> Applies to **V1.3.0** (2026-10-14) · 中文版：[完全指南](./GUIDE.md)
>
> This guide is for **users**: install it, use it well, and know what to do when something looks wrong.
> Developer/build details live in [BUILD_COMMANDS.md](./BUILD_COMMANDS.md) and [RELEASE_PROCESS.md](./RELEASE_PROCESS.md).
> Note: the app UI is currently Chinese-only — UI labels are quoted in Chinese with an English gloss.

---

## Contents

1. [What it is, and who it is for](#1-what-it-is-and-who-it-is-for)
2. [Five-minute start](#2-five-minute-start)
3. [UI map and keyboard shortcuts](#3-ui-map-and-keyboard-shortcuts)
4. [Notes and editing](#4-notes-and-editing)
5. [Organizing: categories, tags, favorites, trash](#5-organizing-categories-tags-favorites-trash)
6. [Retrieval: search, graph, attachments](#6-retrieval-search-graph-attachments)
7. [Data and safety: backup, restore, external edits](#7-data-and-safety-backup-restore-external-edits)
8. [AI features: off by default, enable one tier at a time](#8-ai-features-off-by-default-enable-one-tier-at-a-time)
9. [Settings, section by section](#9-settings-section-by-section)
10. [Troubleshooting](#10-troubleshooting)
11. [For power users: commands, acceptance, performance](#11-for-power-users-commands-acceptance-performance)
12. [Appendix](#12-appendix)

---

## 1. What it is, and who it is for

**In one line**: a desktop app that keeps your notes as plain Markdown files in *your own* directory,
with a Chinese-friendly interface for organizing, searching, graphing, backing up and researching.

Three design principles that explain most of its behaviour:

1. **Data ownership.** Notes are ordinary `.md` files you can open, edit and move with any other tool.
   The database (`.knowledge/database.sqlite`) is **derived data** — delete it and the app rebuilds it from Markdown.
2. **Local first.** No network and no models by default; search runs on local FTS5; attachments, backups and
   exports all happen on your machine.
3. **AI is off by default and explainable.** Networking is a separate gate (default "local only", first use needs
   confirmation). If a decision/generation tier fails, the app degrades automatically **and records which tier
   actually produced the result**.

**Good fit**: people who want their notes to stay on their own disk while still getting solid Chinese full-text
search and structure — and people who need research output that **says "not enough evidence" instead of guessing**.
**Not a fit**: real-time collaboration, cloud sync, and online sharing (out of scope for this version).

---

## 2. Five-minute start

### 2.1 Install

Download from the [v1.3.0 Release](https://github.com/ResonLee-Code/personalknowlge/releases/tag/v1.3.0) and
verify before installing (recommended, especially on flaky networks):

```bash
shasum -a 256 -c SHA256SUMS-all.txt      # Linux: sha256sum -c
```

| Platform | What happens on first launch |
| --- | --- |
| macOS (arm64) | The app is **ad-hoc signed** (no Apple Developer ID, not notarized). Double-click shows "**Apple cannot verify…**": open **System Settings → Privacy & Security**, click "**Open Anyway**" ("仍要打开") and confirm once. After that it never asks again. If you instead see "**is damaged and can't be opened**", that build shipped without ad-hoc signing — use the V1.3.0 package and please file an issue. |
| Windows (x64) | Unsigned: SmartScreen warns → "More info" → "Run anyway". The installer stub being 32-bit is normal (NSIS). |
| Linux (x64) | `.deb` / `.rpm` install directly; `.AppImage` needs `chmod +x`. Requires **WebKitGTK 4.1** (usually present on Ubuntu 22.04+). |

Platform specifics (silent install, uninstall, leftovers) are in
[Windows install guide](./WINDOWS_INSTALL_GUIDE.md) and [Linux install guide](./LINUX_INSTALL_GUIDE.md).

### 2.2 Create your first knowledge base

The first launch shows a welcome page:

1. **Knowledge base name** ("知识库名称") — a display name, e.g. "My Knowledge";
2. **Save directory (absolute path)** ("保存目录") — e.g. `/Users/you/Documents/MyKnowledge`.
   It is created if missing; an **existing empty directory** works too (initialised in place);
3. Click "**创建知识库**" (Create). To take over an existing folder instead, use "**打开已有目录**" (Open existing folder).

The directory then contains `.knowledge/` (database, config, backups, templates, history, logs), `00_Inbox/`,
`attachments/` and so on. The "最近打开" (Recent) list remembers it for one-click access next time.

### 2.3 Write your first note

- Sidebar "**新建笔记**" (New note) or `⌘N`: the dialog offers three entry points —
  **手动创建** (manual) / **导入外部文件** (import files) / **从网页剪藏** (clip a web page);
- Type a title (required); category, tags, template and initial content are optional → click "创建笔记";
- Write plain Markdown: `#` headings, `-` lists, `**bold**`, tables, code blocks;
- Typing `[[` opens title completion; picking one inserts a wiki link;
- Pasting an image stores it under `attachments/YYYY-MM/` and inserts the relative path;
- Saving is automatic (~1 s debounce). `⌘S` forces a save now, `⌘E` puts focus in the editor.

### 2.4 Find things again

- `⌘K` — global search (title/body/tags, with highlighted snippets);
- `⌘P` — quick switcher by title (fuzzy, ↑↓ then Enter);
- `/search` — the full search page with filters (category / tag / favorite / time) and sorting.

That covers ~80% of daily use. Read on as needed.

---

## 3. UI map and keyboard shortcuts

```text
Top bar : global search (⌘K) · save status · theme · settings
Sidebar : Dashboard / All notes / Inbox / Favorites / Graph / Research / Attachments /
          Manage / Categories / Tags / Trash / Settings
Middle  : note list (title filter · sort · multi-select · item menu) ⇄ outline (toolbar toggle)
Right   : editor / preview (toolbar · relations panel · backlinks)
Status  : index status · stats · runtime mode
```

Sidebar width, list width and the editor split ratio are all **draggable** (double-click to reset, arrow keys to
nudge) and are remembered per workspace. Window size and position are remembered too — if the saved position is
no longer on any display (e.g. an external monitor was unplugged) only the size is restored, so the window can
never open off-screen.

| Shortcut | Action |
| --- | --- |
| `⌘/Ctrl + N` | New note |
| `⌘/Ctrl + K` | Global search |
| `⌘/Ctrl + P` | Quick switcher |
| `⌘/Ctrl + S` | Save now |
| `⌘/Ctrl + E` | Focus the editor |
| `⌘/Ctrl + W` | Close the current note |
| `Delete` | Move the selected note to trash |

---

## 4. Notes and editing

### 4.1 Three editor views

The toolbar toggles **编辑 / 分屏 / 预览** (edit / split / preview) and remembers your choice; the split ratio is
draggable (0.3–0.7).

**The outline is not on the right by default** (layout change in V1.3.0): it **shares the middle column with the
note list** — that column normally shows the list; the toolbar's outline button swaps it to the outline (no extra
column, so the text gets more width). "**〈〈 返回列表**" (back to list) in the outline header — or clicking the
button again — brings the list back. "**钉在正文右侧**" in the header pins it to the right of the text instead
(nicer on wide screens); from there "**改到笔记列表那一栏**" moves it back.
Once pinned to the right it keeps the original three states: pinned → "收起大纲" collapses it to a side rail
(hover to expand) → "钉住大纲" pins it again; the toolbar button hides it entirely. With no headings yet, the
outline tells you to start with `#`.

Two related switches live in **Settings → Editor**: "**大纲位置**" (share the list column / pin to the right of
the text) and "**选中笔记后自动显示大纲**" (off by default: when on, selecting any note jumps straight to its
outline — off by default because flipping the pane on every click interrupts browsing). The **placement** is
remembered per workspace. Which pane the column happens to show is not persisted: it goes back to the list when
you restart the app, and otherwise keeps your choice — including while you move from note to note.

### 4.2 Wiki links and backlinks

- `[[Title]]` creates a link (`[[` triggers completion); links are clickable chips in preview;
- "**反向链接**" (backlinks) at the bottom of the editor shows who links here, and jumps on click;
- The "**未解析链接**" (unresolved links) card lists links whose target note does not exist yet and can
  **create that note in one click**;
- For richer semantics use the "**关系**" (relations) panel: typed relations (derived-from / supersedes / related…)
  plus a note.

### 4.3 Sub-documents and dragging

Drag one note onto another in the list to make it a **child**; drag between rows to reorder within a level
(switches sorting to "custom order"). Both are stored in frontmatter (`parent` / `order`), so they survive external
edits. Notes can be pinned (always first within their level, under any sorting).

### 4.4 Templates, history, export

- **Templates**: "更多操作 → 另存为模板…" saves the current note as a template under `.knowledge/templates`;
  placeholders `{{title}}` `{{date}}` `{{time}}` `{{datetime}}` `{{weekday}}` are supported.
- **Version history**: every save is preceded by a snapshot (60-second throttle, max 50 per note).
  "更多操作 → 历史版本…" lets you name versions, **diff any two versions** line by line, and
  **restore the body** (metadata such as title/tags/category stays as-is).
- **Export**: Markdown (frontmatter preserved) / self-contained HTML / Word `.docx` / PDF via the print view.

### 4.5 Three "surprises" while editing

| What you see | What it means | What to do |
| --- | --- | --- |
| "**保存失败：文件已被其他程序修改…**" (save failed: file changed by another program) | You edited the file externally (or restored an old snapshot with `cp`), so the editor's base hash is stale — **this is the conflict guard working**, not a bug | Open "详情" (details) to read the full reason, then choose "重新加载" (reload from disk) or "覆盖保存" (overwrite with the editor) |
| "**已保存（检索索引待重建）**" (saved, index to be rebuilt) | The file was written, but index update failed, so search may not find it yet | "设置 → 索引与数据 → 重建", or restart the app |
| The file was deleted/moved externally | The note is marked "**缺失**" (missing) rather than becoming an empty note | Put the file back and rescan; it recovers automatically. Or handle it in `/manage` |

---

## 5. Organizing: categories, tags, favorites, trash

- **Categories** (`/categories`): a tree; a category **maps to a directory**, and renaming a category renames the
  directory and the notes' paths. Deleting a category asks for a strategy: **move to Inbox / move to parent /
  remove category** — none of them deletes notes.
- **Tags** (`/tags`): create/rename/delete; a note can carry several; names are case-insensitively unique.
- **Favorites** (`/favorites`), also toggleable from the item menu.
- **Trash**: deleting moves the file to `.trash/` (original relative path kept) → restore or delete permanently.
  If the original path was taken by another file, the note is restored to a **safe new path** and that file is
  left untouched.
- **Manage** (`/manage`): a cross-state table (active / trashed / missing) with search, a "not in any folder"
  filter, and batch move / tag / delete / copy.

---

## 6. Retrieval: search, graph, attachments

### 6.1 Search behaviour

- Index: SQLite **FTS5**; Chinese uses single- and double-character expansion, English uses prefix matching,
  title hits rank first;
- Filters: category, tag, favorite, time; sorting: relevance / updated time;
- Results include **highlighted snippets**; index status lives under "设置 → 索引与数据" with manual
  "重建" (rebuild) and "扫描" (rescan).
- Measured locally: with **10,000 notes**, search averages ~20 ms, p95 ~48 ms, recall 100%.

### 6.2 Knowledge graph (`/graph`)

Nodes are notes, edges are wiki links. Colour by category, size by degree, filter by category/tag, show orphans
only, focus 1–2 hops, zoom/pan/drag to pin nodes, double-click to open a note.
Large libraries are **sampled smartly**: every category keeps a representative and neighbours of high-degree nodes
are pulled in.

### 6.3 Attachments (`/attachments`)

Grouped by `YYYY-MM`, showing size, modified time and **the notes referencing them**.
"**只看未引用（N 个 · X MB）**" filters down to unreferenced files for cleanup.

**How "unreferenced" is decided** (documented in the [API contract](./API_CONTRACT.md)):

| Writing style in the body | Counts as a reference? |
| --- | --- |
| `![img](../attachments/2026-10/a.png)` (the relative path the app writes) | ✅ |
| `![[attachments/2026-10/a.png]]` | ✅ |
| `![[20261014-…-a.png]]` (the **stored file name** only) | ✅ |
| `![[img.png]]` (your original file name) | ❌ **dangling**: attachments are renamed on save, so that link never resolved |

Deletion is always per file, with a confirmation that lists the notes referencing it.

---

## 7. Data and safety: backup, restore, external edits

### 7.1 What the directory looks like

```text
MyKnowledge/
├── .knowledge/
│   ├── database.sqlite      # metadata + FTS5 index + settings + decision cache + chunk vectors
│   ├── config.json          # workspace config
│   ├── backups/             # backup zips
│   ├── templates/           # local templates
│   ├── history/<note id>/    # version snapshots
│   ├── research/            # research sessions (disposable)
│   ├── logs/app.log         # runtime log (same reasons the UI shows)
│   └── logs/ai-audit.jsonl  # AI audit trail (never note bodies)
├── .trash/                  # trash
├── 00_Inbox/ 01_… 
└── attachments/
```

### 7.2 Backup / verify / restore

Under "设置 → 备份与导出":

1. **立即备份** (Back up now) → `KnowledgeBackup_<timestamp>.zip` containing database, config, all notes,
   attachments, history, templates and a manifest;
2. **校验** (Verify) → in-place per-item results (manifest parses / Markdown counts match / attachment counts match /
   history+templates match);
3. **恢复** (Restore) → pick an **empty directory**; you get a complete, usable knowledge base;
4. **导出 Markdown** (Export Markdown) → plain directory of all notes, handy for handover or migration.

### 7.3 External edits and conflicts

The app watches the workspace (native events with a polling fallback): externally added/changed files are adopted
and re-indexed, removed files are flagged "missing", and echoes of its own writes are ignored.
If you change a note externally while it is open in the editor, the next save is stopped by the **conflict guard**
(see §4.5) — it would rather ask you than silently overwrite either side.

### 7.4 What can be recovered

| Lost | Recovery |
| --- | --- |
| `.knowledge/database.sqlite` | Reopening the workspace rebuilds it; search index, caches and vectors are rebuilt (all derived) |
| A note's version history | Under `.knowledge/history/<note id>/`, or from a backup archive |
| The whole machine | Restore from a backup zip, or just copy the workspace directory to the new machine |
| An accidentally deleted note | Check `.trash/` first; if purged, only a backup helps |

---

## 8. AI features: off by default, enable one tier at a time

> **Defaults**: AI master switch **off**, network tier **local only**, web-page fetching **off**.
> Out of the box it is a purely local application — it makes no requests at all.

### 8.1 Four gates

| Gate | Default | Notes |
| --- | --- | --- |
| AI master switch | off | While off, every AI capability is refused (`AI_DISABLED`) |
| Network tier | local only | Switching to "allow network" still requires **per-run confirmation**; changing endpoint or model invalidates the authorization |
| Per-send confirmation (optional) | off | When on, every outbound request first shows you "the exact snippet + target host" |
| Domain allow/deny lists | empty | The **denylist always wins**; dropped results are visible in the report |

Every outbound action is appended to `.knowledge/logs/ai-audit.jsonl` (time / capability / outbound yes-no / result /
latency — **never note bodies**).

### 8.2 Decision model tier (optional)

Under "设置 → AI → 决策模型" you can switch between:

- **Rule tier** (default): offline, zero cost, always available — keyword/polarity scoring;
- **System One tier**: endpoint + model name + API key (Perplexity Decisions and Jev share one protocol, so a single
  adapter covers both). The key is **write-only**, separately authorized, and can be cleared anytime.

Once configured and authorized, the decision tier is tried **first**; on failure it degrades to the rule tier and
the report records which tier actually ran. To see data before trusting it, run the "**决策探针**" (decision probe):
it measures accuracy, coherence (P(yes)+P(no)), run-to-run determinism, contradiction handling, latency and
estimated cost on **our own Chinese items** — with **100 scored items** producing ECE / Brier / binning and an
**out-of-sample** ECE, plus an **A/B comparison of the rule tier vs the decision tier** on scoring tasks.

> Measured for V1.3.0: in-material items **40/40, ECE 0.0159**; out-of-material items skew toward "no"
> (mean stated probability 0.376) — so do **not** read a low score there as "certainly irrelevant";
> it means "I don't know this thing".

### 8.3 Research assistant (`/research`) in five steps

1. **Plan**: enter a topic (3–5 sub-questions; over-broad topics are rejected). **Query expansion** runs here
   (topic → quoted phrases / connective splits / Latin tokens, ≤6 queries, rule-based and offline); retrieval runs
   each query and de-duplicates by note.
2. **Collect**: local FTS5 (plus external search if the network tier is open, with domain policy and grading).
3. **Verify**: a sub-question needs **≥2 independent sources** to count as cross-verified; conflicting sources are
   presented **side by side without picking a winner**.
4. **Compose**: a fixed six-section report (conclusions / key facts / disputes & uncertainty / relation to your
   knowledge base / to-verify list / references), each claim carrying citations and confidence; the header always
   shows "data as of YYYY-MM-DD".
5. **Write proposal**: defaults to Inbox, **never overwrites** a same-title note, shows a diff first, and the result
   can be rolled back from version history.

**When evidence is insufficient it says so.** The report header, the warning block and the conclusions section all
state what is missing (which sub-questions have no material, whether ≥2 independent sources exist, whether the
material is too thin / too weak / likely stale) and how to fill the gap. This matters because the app's default
failure mode used to be "answer confidently anyway"; now it is "**give no conclusion**".

### 8.4 Vector retrieval (optional enhancement)

Under "设置 → AI → 向量检索": enter an **embedding model name** (reusing the online model endpoint and key, with a
**one-time confirmation**), then build incrementally. Vectors are derived data, rebuildable in one click, and are
excluded from backups. When enabled, a `vector` channel joins the **RRF fusion** — and if it is unavailable the app
falls back to pure BM25, so **search never breaks**.

### 8.5 Local / online models and generation

- **Local model (Ollama)**: loopback addresses only, off by default; low-spec presets fill in `qwen2.5:3b` or `qwen2.5:7b`;
- **Online model (OpenAI-compatible)**: endpoint / model / key, usable after the first authorization;
- **Model self-check**: one click to see which tier would write the report and why (read-only, never contacts remote endpoints);
- Fallback order: **online (authorized) → local → template**; whichever tier runs is recorded as the actual
  provider in the report, and a template-generated report is explicitly labelled "未接入模型" (no model).

---

## 9. Settings, section by section

| Section | Contents | Do you need it? |
| --- | --- | --- |
| **编辑器** (Editor) | Autosave delay, font size, theme, default view (edit/split/preview), default location for new notes, outline placement, auto-show outline on select | Tune to taste |
| **索引与数据** (Index & data) | Index status, rebuild index, rescan workspace, open workspace folder | When indexing misbehaves |
| **备份与导出** (Backup & export) | Back up now, backup list (verify / restore), export Markdown | Back up regularly |
| **AI** | Master switch, self-check, network tier, fetch web pages, allow/deny lists, research budget, local presets, online model, vector retrieval, decision model, outbound proxy | Enable item by item as needed |
| **关于** (About) | Version, data/log locations, runtime mode | For bug reports |

> **Outbound proxy** resolves as "explicit setting → environment variables → macOS system proxy", and
> **loopback addresses are never proxied**.

---

## 10. Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| macOS: "cannot verify the developer" on first launch | Expected: the build is ad-hoc signed. **System Settings → Privacy & Security → Open Anyway** ("仍要打开"), confirm once |
| macOS: "is damaged and can't be opened" | That package shipped **without** ad-hoc signing. Use the V1.3.0 Release; if V1.3.0 still shows it, please file an issue |
| Windows: SmartScreen blocks it | Unsigned: More info → Run anyway. The 32-bit installer stub is normal |
| Linux: missing libraries | Install WebKitGTK 4.1 and GTK3 (package names in the [Linux guide](./LINUX_INSTALL_GUIDE.md)) |
| Save fails with "file changed by another program" | The conflict guard (§4.5). Open "详情", then reload from disk or overwrite |
| A just-written note is not searchable | The badge "已保存（检索索引待重建）" means indexing lagged: rebuild the index in Settings, or restart |
| A note shows "缺失" (missing) | Its file was moved/renamed: put it back and rescan, or handle it in `/manage` |
| Large library feels slow | The first scan/build is one-off; a 30k-note full graph query measured ~0.9 s. Caches live under `.knowledge/` and can be rebuilt |
| Reset all settings | Quit the app and delete `.knowledge/database.sqlite` (**you lose settings and the history index**; note bodies are untouched — back up first) |
| Full uninstall | Delete the app; **your workspace directory is your data and is never touched**. App data (`~/Library/Application Support/PersonalKnowledgeOS`) and preferences must be removed manually |
| What to include in a bug report | Timestamp + the in-app **failure details** (click "详情" to copy) + the matching lines in `.knowledge/logs/app.log` |

---

## 11. For power users: commands, acceptance, performance

```bash
# Local development
pnpm install && pnpm dev            # browser preview (in-memory backend, no Tauri needed)
pnpm tauri dev                      # desktop app
pnpm test && pnpm build && pnpm lint
cd src-tauri && cargo test --offline

# Pre-release guard (don't skip after touching search/index/graph)
pnpm bench                          # 10k search + 30k graph; fails on threshold breach

# Packaging and acceptance
pnpm tauri build --bundles app      # macOS .app
scripts/acceptance/run.sh           # real-machine acceptance: drives the UI over AX, writes a report
scripts/size-report.sh              # artifact size table (caliber fixed in ARTIFACT_SIZES.md)
```

**Measured on this machine (2026-10-14)**

| Metric | Measured | Threshold |
| --- | --- | --- |
| 10k-note search p95 / mean | **48.0 ms / 20.1 ms** | p95 < 200 ms |
| 10k-note recall@5 | **100%** | ≥ 100% |
| 30k-note full graph query | **905.4 ms** | < 1.5 s |
| release binary / `.dmg` size | 10.57 MB / 5.43 MB | — |

Size calibers and per-artifact numbers: [ARTIFACT_SIZES.md](./ARTIFACT_SIZES.md).
Layering and data flows: [ARCHITECTURE.md](./ARCHITECTURE.md). AI boundaries: [AI_ARCHITECTURE_PLAN.md](./AI_ARCHITECTURE_PLAN.md).

---

## 12. Appendix

### 12.1 Shortcuts

`⌘N` new note · `⌘K` global search · `⌘P` quick switcher · `⌘S` save · `⌘E` focus editor · `⌘W` close note ·
`Delete` move selection to trash (use `Ctrl` on Windows/Linux).

### 12.2 File naming and frontmatter

- File name: `<creation timestamp>-<sanitised title>.md`; renaming a note can also rename the file on disk;
- Frontmatter keeps `id` / `title` / `tags` / timestamps / `parent` / `order` / source & freshness fields
  (`source_type` / `source_ref` / `source_reliability` / `valid_until`). External edits preserve these —
a rescan never wipes them.

### 12.3 Related documents

- [README](../README.md) · [完全指南（中文）](./GUIDE.md)
- [BUILD_COMMANDS.md](./BUILD_COMMANDS.md) — building and packaging (incl. macOS signing requirements)
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md) — release steps, CI pipeline, post-release spot checks
- [ARTIFACT_SIZES.md](./ARTIFACT_SIZES.md) — artifact sizes and calibers
- [ACCEPTANCE_V1.3.0.md](./ACCEPTANCE_V1.3.0.md) — this version's acceptance record
- [CHANGELOG.md](./CHANGELOG.md) — every user-visible change
