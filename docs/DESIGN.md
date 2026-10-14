# Design

The Library's panel is the TAW hi-fi look shared with the Player, Equaliser and Analyser: TAW-Panel's `hifi` module. Spool's warm panels are the default (Light, Amber), tinted by any of Spool's seven colour themes; Porcelain is the alternative; each in Light, Dark or Automatic. The maker's badge ("Timster's Audio Works - Library"), the slanted logo (LIBRARY) and the model line (LB-001 · MUSIC LIBRARY MANAGER · V…) stay from the original panel. The reviewed mock-up is TAW-Panel's `docs/finishes-review.html` (Library, Simple and Detailed).

## Layout

- **Nameplate (top):** badge, logo and model line; window buttons at the top right; under them the family switcher (Library, Player, Equaliser, Analyser; apps not found are dimmed), Simple / Detailed and the gear for Settings. Drag the strip to move the window.
- **Search and buttons:** search across artist and album names; **Scan drives** (Stop scan while scanning), and in Detailed **Add folder**, **Exclude…** and **Lookups: off**.
- **Sidebar:** VIEW (Library, What's missing; Detailed adds Duplicates and Organise, marked "soon"); DRIVES (a green light when connected, an empty ring when not, the track count; Detailed adds what's scanned and when, a switch for scanning the whole drive, and × to forget an offline drive or remove a folder); Detailed adds LEFT OUT (excluded folders).
- **Library view:** artists (a light for how complete their albums are; Detailed shows "Merge?" suggestions), their albums (artwork, year, title, tracks, time and format, and what each needs), and the album: big artwork, its title and facts, tags for the format and what's missing, **Play in Player** and **Show folder**, where it is, and its tracks.
- **What's missing:** a tile for each kind of gap with its count, and the albums with it; click one to open it in the Library view.
- **Footer:** track and album counts and the last scan (Detailed adds how complete the tags and artwork are, and the main formats); messages on the right, otherwise "Read only · your files are never changed".

## Colours that mean something

| Colour | Means |
|---|---|
| Green light or tag | Connected; complete; lossless |
| Amber | Worth fixing, not broken (small artwork, a suggestion) |
| Red | Missing |
| Empty ring | A drive that isn't connected |
