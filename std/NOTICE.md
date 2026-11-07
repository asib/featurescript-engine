# Onshape FeatureScript Standard Library (std)

The `.fs` files in this directory are Onshape's FeatureScript Standard Library, copied unchanged. They are
distributed under the MIT License; see [LICENSE](LICENSE). Copyright (c) 2013-Present PTC Inc.

- **Source:** Onshape's public "std" document,
  <https://cad.onshape.com/documents/12312312345abcabcabcdeff>
- **Version:** see `versionName` and `versionId` in [manifest.json](manifest.json). The manifest also records each
  file's element id, SHA-256 hash and line count.
- **How it was copied:** `tools/browser/fetch-std.mjs` opened each Feature Studio tab of that version in a
  logged-in browser and saved the text of its editor. Nothing was fetched through Onshape's API.
- **Check a copy:** `node tools/check-std.mjs std`

The only file left out is the README tab, which is a PDF.

Files are not all marked with the copied version. Since 2985, Onshape bumps a file's `FeatureScript` header, and the
versions in its imports, only when that file or something it imports changes. So headers run from 2985 up to the
copied version. Every std import names a file whose header is no newer than the import's version, so this one
copy holds exactly the module each import refers to. `tools/check-std.mjs` checks this.

Copied 2026-11-07: version 3112.0. It has 277 Feature Studios and 150,805 lines. Compared with the community
mirror's 2960, 165 files are unchanged apart from version numbers, and they match it byte for byte. 100 files
changed, and 12 are new.

To move to a newer std version, run the fetch script into an empty directory, check it, and replace these files in
one commit. Our engine and the oracle's test Feature Studio must use the same version.
