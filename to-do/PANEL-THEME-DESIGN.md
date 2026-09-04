# Panel theme and Design

## Scope

Panel-global (not per-site branding):

- **Color mode** (Light / Dark): per signed-in user. Sidebar toggle. Stored in `localStorage` (`cpn-color-mode`) and `/var/lib/cpn/user-prefs/<user>.json`.
- **Design**: panel chrome + Manage dashboard tokens for everyone. Stored in `/var/lib/cpn/panel-design.json`. Admin-only to edit.

## Default and Restore

- **Default** is an immutable built-in token set in code (`default_tokens()`). Choosing Default activates that baseline without deleting a saved Custom profile.
- **Restore** clears Custom from disk and sets preset back to Default.
- **Custom** edits (accent, accent focus, radius, density, font scale) save as a separate profile; they never overwrite the Default constants.

## Presets

`default` | `light` | `dark` | `custom`

## APIs (session required)

- `GET/POST /api/panel/color-mode`
- `GET /api/panel/design`
- `POST /api/panel/design` (admin, custom tokens)
- `POST /api/panel/design/preset` (admin)
- `POST /api/panel/design/restore` (admin)
