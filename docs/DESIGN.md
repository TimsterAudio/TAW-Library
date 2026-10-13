# Panel design

The Library uses Drone Box's panel language, as the Equaliser does, so the family looks like one range of hardware. Drone Box's `docs/DESIGN.md` is the reference; the Equaliser's `docs/DESIGN.md` shows how a native egui panel applies it. This page covers what's different in Music Box.

## The one big difference: lists

The Equaliser is a fixed panel of controls. The Library is mostly lists, so:

- **The chrome is fixed** (top strip, stripe, section frames, keys, red displays), and **the lists stretch** with the window. Below 1560 × 880 the whole panel scales down instead of squeezing, as the Equaliser's does.
- **Lists sit in a recessed well** and use the legend face (Barlow Semi Condensed) in the legend colour. Long lists in display red are tiring to read, so red is kept for counts, readouts and things that are missing.
- **Selection** is the orange focus colour: a tinted row with an orange edge.

## Top strip, left to right

Maker badge, logo (Michroma, slanted) and model line "LB-001 · MUSIC LIBRARY MANAGER"; the LED display; **Library** keys (Add in cream, Exclude in grey); the **Tags** and **Art** completeness ladders (green and amber); **View** (Library, Missing, Dupes, Organise: grey keys with LEDs); **Panel** (Black, Silver); **Online** (Lookups, yellow, with its LED off until stage 2); **Scan / Stop** (red, with its LED lit while scanning, in the Equaliser's Run / Stop place).

## Lights and chips

| | Meaning |
|---|---|
| Green light | Present and correct; a drive that's connected |
| Amber light | Could be better: small artwork, mixed formats, an artist with some incomplete albums |
| Red light | Missing |
| Grey light | A drive that isn't connected |
| Green outlined chip | Lossless format; "Complete" |
| Grey outlined chip | Lossy format |
| Red outlined chip | Something missing |
| Amber filled chip | A suggestion ("Merge?") |

## Keys

| Colour | Used for |
|---|---|
| Red | Scan / Stop; Play (sends the album to a player) |
| Yellow | Lookups (going online) |
| Cream | Add, Folder, Look up, Edit |
| Grey | Exclude, View, Panel |
| Green lit key | A drive scanned whole |
