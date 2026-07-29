# Introduction

RFC 093 A3.2 negative fixture: `SUMMARY.md` references `./does-not-exist.md`,
which is not present under `src/`. `mdbook build` on this tree must exit
non-zero.
