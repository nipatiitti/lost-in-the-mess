FONTS
=====

This system pulls fonts from Google Fonts via @import in colors_and_type.css:

  - Space Grotesk  (display + UI)        — variable, 300–700
  - JetBrains Mono (data / code / mono)  — variable, 300–700
  - Inter Tight    (UI body fallback)    — variable, 400–600

No font files are bundled. If you want offline use, download the
above families from fonts.google.com and drop the .woff2 files in
this folder, then swap the @import in colors_and_type.css for a
local @font-face block.

NOTE TO BRAND OWNER
-------------------
These are the closest open-source matches for a "tactical engineering
telemetry" feel. If "Lost in the mes(h)s" has a real wordmark / type
license already (e.g. Berkeley Mono, GT America, NB International,
Söhne Mono, ABC Diatype Mono), share the files and we'll swap them in.
