# 04 · General page

Default-browser status, startup behaviour, the tray icon, and an explanation of what Wye
cannot intercept.

## Layout

```
Default Browser
┌───────────────────────────────────────────────┐
│ ✓ Wye is your default browser                 │
│                          [Stop Being Default] │
│ Also open local HTML files              [off] │
└───────────────────────────────────────────────┘

Startup
┌───────────────────────────────────────────────┐
│ Launch at login                         [on ] │
└───────────────────────────────────────────────┘

Tray
┌───────────────────────────────────────────────┐
│ Tray icon            ☰ Primary Browser    ⌃⌄  │
│ Show tray icon                          [on ] │
└───────────────────────────────────────────────┘

┌───────────────────────────────────────────────┐
│ ⊗  Wye cannot handle links clicked inside a   │
│    browser. …                                 │
└───────────────────────────────────────────────┘
```

## Requirements

| ID | Requirement | Evidence |
|---|---|---|
| GEN-01 | Group **Startup**, switch row **Launch at login**. On: Wye starts with the user session, so the tray icon is there from the start. | Specified |
| GEN-02 | Group **Tray**, popup row **Tray icon** with two values: **Primary Browser** (the tray icon mirrors the primary browser, [TRAY-02](01-tray-menu.md)) and **Wye** (a fixed monochrome symbolic icon that takes the panel's colour, the norm on GNOME and Plasma panels). | Specified (row, first value), Proposed (second value) |
| GEN-03 | Same group, switch row **Show tray icon**. Off: the icon disappears; Wye keeps routing links ([TRAY-04](01-tray-menu.md)). | Specified |
| GEN-04 | Dismissible callout ([BLK-09](03-settings-window.md#shared-building-blocks)) below the groups: "Wye cannot handle links clicked inside a browser. You can either use the browser extension (see the website for more info), or copy the link and then choose “Open URL from Clipboard” in the Wye menu." | Specified (callout), wording adapted |
| GEN-05 | Group **Default Browser**, first on the page: default-browser status row with **Make Default** / **Stop Being Default**, and switch **Also open local HTML files**. Specified in [ONB-10](18-onboarding.md#default-browser-status). | Proposed |

## Defaults

| Setting | Default |
|---|---|
| Launch at login | on (Proposed; the value shown in the design) |
| Tray icon | Primary Browser |
| Show tray icon | on |
| Also open local HTML files | off |
| Callout | shown until dismissed |

## Linux notes

- **Launch at login**: an XDG autostart entry in `$XDG_CONFIG_HOME/autostart/`, or a
  systemd user unit bound to `graphical-session.target` (Token Station's approach). In a
  Flatpak build, use the Background portal (`org.freedesktop.portal.Background`,
  `RequestBackground` with `autostart`).
- Handling links does not depend on this switch, because the handler is D-Bus
  activatable ([DEF-04](11-url-pipeline.md#default-browser-registration)); the switch
  only decides whether the tray icon is there from the start.
