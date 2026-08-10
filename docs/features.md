# Features

- [Features](#features)
  - [Indexing](#indexing)
  - [Search](#search)
  - [Duplicate Detection](#duplicate-detection)
  - [Web UI](#web-ui)
  - [Cleanup](#cleanup)
  - [Consolidation](#consolidation)
  - [Resumability](#resumability)
- [Notes](#notes)

## Indexing
  - [Files]()
  - [Folders]()

## Search

## Duplicate Detection

## Web UI

Browser UI served from `static/` (no frontend build). See **[UI.md](./UI.md)** for the full design system.

| Area | Status | Notes |
|---|---|---|
| Theme switching (light / dark / system) | Done | Sidebar control; `localStorage` key `fi-theme` |
| Design tokens | Done | `static/css/tokens.css` |
| Reusable components | Done | Buttons, cards, tables, drawers, badges, … in `components.css` |
| Shared sidebar / nav | Done | Injected by `sidebar.js` on every page |
| Drawer (detail panels) | Done | `drawer.js`; used for folders, ignore rules |
| Dashboard | Done | Stats + Chart.js timeline (theme-aware) |
| Search | Done | Results table + folder drawer |
| Explorer | Done | Indexed tree browser |
| Duplicate files / folders | Done | Groups, filters, merge flows |
| Processes | Done | Cards + history table + detail sidebar |
| Logs / Status / Settings / Ignored / Skipped | Done | Tokenized layouts |

## Cleanup
- [node_modules]()

## Consolidation

## Resumability



# Notes

- I have a path ((( one entity )))
- I retrieve entries from directory ((( multiple entries )))
- Create transaction
- Store all entries in database
- Commit transaction

- for all directory entries start all over