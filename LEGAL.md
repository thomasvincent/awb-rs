# Clean-Room Implementation Notice

## Provenance Statement

AWB-RS is a **clean-room implementation** of an AutoWikiBrowser-like tool. No code from the
original AutoWikiBrowser (AWB) project has been copied, translated, or adapted into this codebase.

## Original AWB License

AutoWikiBrowser is licensed under the GNU General Public License v2.0 (GPL-2.0).
AWB-RS is **not** a derivative work of AWB.

## How AWB-RS Was Built

All functionality in AWB-RS was implemented based on:

1. **Public MediaWiki API documentation** at https://www.mediawiki.org/wiki/API
2. **Wikipedia policy pages** describing AWB behavior (Wikipedia:AutoWikiBrowser)
3. **MediaWiki API response schemas** observed through standard API usage
4. **Original engineering** for architecture, data structures, and algorithms

No contributor to AWB-RS has referenced AWB source code during development.

## License

AWB-RS is dual-licensed under MIT and Apache-2.0. See LICENSE-MIT and LICENSE-APACHE.

## Third-Party Dependencies

All dependencies are permissively licensed (MIT, Apache-2.0, or MPL-2.0).
No GPL dependencies are used. See `Cargo.toml` for the complete dependency list.
