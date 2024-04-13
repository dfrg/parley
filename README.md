This is a nascent project for implementing a rich text layout engine backed
by the [swash](https://github.com/dfrg/swash) crate and implementing the 
[piet](https://github.com/linebender/piet) text API.

## Usage

```rust
use parley::FontContext;
use parley::LayoutContext;
use parley::style::{StyleProperty, FontStack};
use parley::layout::Alignment;
use peniko::{Brush, Color};

// The text we are going to style and lay out
let text = String::from("Some text here");

// The display scale for HiDPI rendering
let display_scale = 1.0;

// Create a FontContext, LayoutContext and then a RangeBuilder
let mut font_cx = FontContext::default();
let mut layout_cx = LayoutContext::new();
let mut builder = layout_cx.ranged_builder(font_cx, &text, display_scale);

// Set default styles (set text colour to red and font to Helvetica)
let brush = Brush::Solid(Color::rgb8(255, 0, 0));
builder.push_default(&StyleProperty::Brush(brush));
let font_stack = FontStack::Source("Helvetica"));
builder.push_default(&StyleProperty::FontStack(font_stack));

// Override default styles for subrange of the text (make the first 5 characters bold)
let bold = style::FontWeight::new(600.0)
builder.push(&style::StyleProperty::FontWeight(), 0..=5);

// Build the builder into a Layout
let mut layout : Layout<Brush> = builder.build();

// Perform layout (including bidi resolution and shaping) with start alignment
layout.break_all_lines(max_advance, Alignment::Start);

// Render using Vello (see vello repo for setup)
// =============================================

pub fn render_text(scene: &mut Scene, transform: Affine, layout: &Layout<Brush>) {
  // Iterate over laid out lines
  for line in layout.lines() {

    // Iterate over GlyphRun's within each line
    for glyph_run in line.glyph_runs() {

        // Resolve properties of the GlyphRun
        let mut x = glyph_run.offset();
        let y = glyph_run.baseline();
        let style = glyph_run.style();

        // Get the "Run" from the "GlyphRun"
        let run = glyph_run.run();

        // Resolve properties of the Run
        let font = run.font();
        let font_size = run.font_size();
        let coords = run
            .normalized_coords()
            .iter()
            .map(|coord| vello::skrifa::instance::NormalizedCoord::from_bits(*coord))
            .collect::<Vec<_>>();

        // Create an iterator that iterates over the glyphs in the GlyphRun while converting the glyphs
        // from Parley's glyph format into Vello's glyph format
        let mut glyph_iterator = glyph_run
          .glyphs()
          .map(|glyph| {
              let gx = x + glyph.x;
              let gy = y - glyph.y;
              x += glyph.advance;
              vello::glyph::Glyph {
                  id: glyph.id as _,
                  x: gx,
                  y: gy,
              }
          });

        // Draw the glyphs using vello
        scene
            .draw_glyphs(font)
            .brush(&style.brush)
            .transform(transform)
            .font_size(font_size)
            .normalized_coords(&coords)
            .draw(Fill::NonZero, glyph_iterator);
    }
  }
}

```

## Code structure

A guide to parley's source code.


```
src
|
| MISC
|
|-- lib.rs  - Entry point. Contains only re-exports.
|-- util.rs - Float comparision helpers
| 
|-- context.rs - Contains LayoutContext and RangedBuilder which collectively make up the top-level API in Parley
|
|
| STYLE & STYLE RESOLUTION
|
|-- style - Types representing styles that can be set on a range of text
|   |
|   |-- mod.rs   - `StyleProperty` enum representing all of the styles that can be set on a range of text
|   |-- font.rs  - Types to represent font styles (e.g. font family) in the public API
|   |-- brush.rs - `Brush` trait that represents the color of glyphs or decorations.
|
|-- resolve - Resolving styles from the end-user API into styles that parley can use for shaping, layout, etc.
|   |
|   |-- mod.rs   - Types to represent font styles (e.g. font family) in the public API
|   |-- range.rs - Types that allow overlapping ranges of single style properties to be resolved into non-overlapping ranges of all style properties
|   |-- tree.rs  - (FILE EMPTY) Placeholder for an alternative to range.rs that works on trees of styles instead of ranges.
|
| BIDI, SHAPING and LAYOUT
| 
|-- bidi.rs  - Splits a string of text into a sequence of runs according to the Unicode Bidirectional Algorithm
|-- shape.rs - Takes output of bidi and style systems and splits text into runs for shaping and then shapes those runs.
|
|-- layout - Line breaking, layout and alignment of shaped text runs
|   |
|   |-- line
|   |  |
|   |  |-- greedy.rs - The actual line breaking implementation
|   |  |-- mod.rs    - Supporting code for line breaking
|   |
|   |-- mod.rs       - Data structures for line breaking
|   |-- data.rs      - Data structures for line breaking. The push_run method used in shaping.
|   |-- run.rs       - Method implementation for Run type
|   |-- cluster.rs   - Method implementations for the Cluster type
|   |-- cursor.rs    - Cursor type representing a position within a layout. Hit testing.
```
