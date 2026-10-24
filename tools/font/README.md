# Nwasabi Pixels

The font used for the on-screen text of applications (`draw_string`). It is drawn for nwasabi:
it is reminiscent of Epsilon's font without being a copy of it.

## License

MIT, see [NwasabiPixels-MIT.txt](./NwasabiPixels-MIT.txt). It covers the script in this folder and
the files it produces (`src/kernel/font/data.rs`): together they make up the font.

## Usage

```sh
python3 tools/font/generate.py
```

Python 3 is enough, nothing needs to be installed. The script rewrites `src/kernel/font/data.rs`,
the file that holds the glyphs; nothing else in the project depends on the script.

## Changing a glyph

Every character is a small function of the script, marked with `@glyph("a")`. It returns strokes
(`line`, `dot`, `stem`, `bar`...) placed on the landmarks of the font size (`m.L`, `m.R`, `m.X`,
`m.K`..., described in `Metrics`), or a pixel-by-pixel drawing with `art(...)`. There are two sizes:
7x14 pixels and 10x18 pixels. After a change, run the script again, then run the tests; the
reference screenshots of the corpus are regenerated with `just update-golden <folder>`.
