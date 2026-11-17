# Fonts for sketch text

The fonts `@skText` draws with, as Onshape's sketch text does (batches 091, 185): its outlines are these versions'.

| file | font | version | from | licence |
|---|---|---|---|---|
| OpenSans-Regular.ttf, -Bold, -Italic, -BoldItalic | Open Sans | 1.10 | Debian's `fonts-open-sans` 1.11 | Apache-2.0 (OpenSans-COPYRIGHT, LICENSE-Apache-2.0) |
| Arimo-Regular.ttf, -Italic, -BoldItalic | Arimo | 1.33 | Debian's `fonts-croscore` 20201225 | OFL-1.1 (Arimo-COPYRIGHT) |
| Tinos-Regular.ttf, -Bold, -Italic, -BoldItalic | Tinos | 1.33 | Debian's `fonts-croscore` 20201225 | OFL-1.1 (Arimo-COPYRIGHT, the package's) |

The current releases differ (Open Sans 3's "I" is 200 to 370 units, Onshape's 201 to 371 as in 1.10). Onshape's Arimo
Bold, Noto Sans and Noto Serif are other versions than those packages' (batch 185: their "Ag" 0.003 to 0.03 mm² off),
and Allerta Stencil, Droid Sans Mono and Roboto Slab aren't here: the engine refuses them. Any other name is Open
Sans Regular, as std documents.
