# 16 · Script editor and transform scripts

Transform scripts rewrite a link before it opens. There are two kinds: the global script
([ADV-03](10-advanced.md#url-transformation)), which runs on every link before rules,
and per-rule scripts ([RUL-25](08-rules.md#rule-editor-sheet)), which run only when their
rule matches. Both use the same editor and the same script API.

## Editor window

```
Transform Script — Global                                     [Reference]
┌──────────────────────────────────────────────────────────────────────┐
│  1  // Return a new URL to change the link, or nothing to keep it.   │
│  2  export default function transform(url, context) {               │
│  3    if (url.hostname === "twitter.com") url.hostname = "x.com";    │
│  4    return url;                                                    │
│  5  }                                                                │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
Test
┌──────────────────────────────────────────────────────────────────────┐
│ Link          https://twitter.com/example/status/1                   │
│ Source app    ◉ Slack                                            ⌃⌄  │
│ Result        https://x.com/example/status/1               ✓ 0.4 ms  │
└──────────────────────────────────────────────────────────────────────┘
                                                      [Cancel] [Save]
```

| ID | Requirement | Evidence |
|---|---|---|
| SCR-01 | A separate dialog, about 680 × 520 px, resizable. Title: "Transform Script — Global" or "Transform Script — \<rule name\>". | Proposed |
| SCR-02 | Code area: monospace, line numbers, syntax highlighting, auto-indent, bracket matching, undo/redo. GNOME: GtkSourceView 5. KDE: KSyntaxHighlighting in a text area. | Proposed |
| SCR-03 | A new script starts from a commented template that shows the function signature and one example. | Proposed |
| SCR-04 | **Test** group: **Link** entry (pre-filled with the last opened link, else `https://example.com/?utm_source=test`), **Source app** popup (optional), and **Result**. The result updates as the user types (debounced): the output URL with changed parts highlighted, "Unchanged", or the error with its line number, plus the run time. | Proposed |
| SCR-05 | Errors are underlined in the code area at the reported line. | Proposed |
| SCR-06 | **Reference** (header button) opens a popover with the script API below and examples. | Proposed |
| SCR-07 | **Save** is disabled while the script has a syntax error. Runtime errors do not block saving. | Proposed |
| SCR-08 | Scripts are stored as files next to the configuration ([12](12-data-model.md#storage)). When the file changes on disk (edited elsewhere), an open editor offers to reload it. | Proposed |
| SCR-09 | Enabling a transform switch with an empty script opens the editor. | Proposed |

## Script API

Proposed language: JavaScript, run in an embedded engine (QuickJS through `rquickjs`),
because most users who write URL rewrites already know it and `URL` / `URLSearchParams`
are familiar. Alternatives considered: Rhai (Rust-native, less familiar) and Lua
(`mlua`). The choice stays open ([14](14-open-questions.md)).

```js
/**
 * @param {URL} url           The link after expansion and cleaning. Mutable.
 * @param {object} context
 * @param {string|null} context.sourceApp   Desktop ID of the source app, if known.
 * @param {string} context.entryPoint       "handler" | "clipboard" | "extension" | "cli"
 * @param {string[]} context.heldKeys       e.g. ["Ctrl"]
 * @param {string|null} context.rule        Matched rule name (per-rule scripts only).
 * @returns {URL|string|undefined}          New link, or undefined to keep it.
 */
export default function transform(url, context) {}
```

| ID | Requirement | Evidence |
|---|---|---|
| SCR-20 | Globals: `URL` and `URLSearchParams` with WHATWG semantics (backed by Rust's `url` crate), and `console.log`, which writes to Wye's log and to the editor's result area. Nothing else: no network, no file access, no timers. | Proposed |
| SCR-21 | Limits: 50 ms run time and 16 MB memory per call. A script over a limit is stopped and treated as a runtime error. | Proposed |
| SCR-22 | Runtime error while opening a link: the link continues unchanged, and Wye shows one notification per script until the script changes. | Proposed |
| SCR-23 | A returned value that is not a valid `http`/`https` URL is a runtime error. | Proposed |

Examples shipped in the Reference popover:

```js
// Open Reddit links on old.reddit.com
export default function transform(url) {
  if (url.hostname.endsWith("reddit.com")) url.hostname = "old.reddit.com";
  return url;
}
```

```js
// Send YouTube links to a self-hosted front end
export default function transform(url) {
  if (url.hostname === "youtu.be") {
    return `https://invidious.example.org/watch?v=${url.pathname.slice(1)}`;
  }
}
```
