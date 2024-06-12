// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, rasterises the glyph using Swash
//! and and then renders it into a PNG using the `image` crate.

use image::codecs::png::PngEncoder;
use image::{self, Pixel, Rgba, RgbaImage};
use parley::layout::{Alignment, Glyph, GlyphRun, Layout, LayoutItem2};
use parley::style::{FontStack, FontWeight, StyleProperty, TextStyle};
use parley::{FontContext, InlineBox, LayoutContext};
use peniko::Color;
use std::fs::File;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Scaler, Source, StrikeWith};
use swash::zeno;
use swash::FontRef;
use zeno::{Format, Vector};

fn main() {
    // The text we are going to style and lay out
    let text = String::from(
        "Some text here. Let's make it a bit longer so that line wrapping kicks in 😊. And also some اللغة العربية arabic text.",
    );

    // The display scale for HiDPI rendering
    let display_scale = 1.0;

    // The width for line wrapping
    let max_advance = Some(200.0 * display_scale);

    // Colours for rendering
    let text_color = Color::rgb8(0, 0, 0);
    let bg_color = Rgba([255, 255, 255, 255]);

    // Padding around the output image
    let padding = 20;

    // Create a FontContext, LayoutContext and ScaleContext
    //
    // These are all intended to be constructed rarely (perhaps even once per app (or once per thread))
    // and provide caches and scratch space to avoid allocations
    let mut font_cx = FontContext::default();
    let mut layout_cx = LayoutContext::new();
    let mut scale_cx = ScaleContext::new();

    // Setup some Parley text styles
    let brush_style = StyleProperty::Brush(text_color);
    let font_stack = FontStack::Source("system-ui");
    let font_stack_style: StyleProperty<Color> = StyleProperty::FontStack(font_stack);
    let bold = FontWeight::new(600.0);
    let bold_style = StyleProperty::FontWeight(bold);

    let mut layout = if std::env::args().any(|arg| arg == "--tree") {
        // TREE BUILDER
        // ============

        // TODO: cleanup API

        let root_style = TextStyle {
            brush: text_color,
            font_stack,
            line_height: 1.3,
            font_size: 16.0,
            ..Default::default()
        };

        let mut builder = layout_cx.tree_builder(&mut font_cx, display_scale, &root_style);

        builder.push_style_modification_span(&[bold_style]);
        builder.push_text(&text[0..5]);
        builder.pop_style_span();

        builder.push_text(&text[5..40]);

        builder.push_inline_box(InlineBox {
            id: 0,
            index: 0,
            width: 50.0,
            height: 50.0,
        });

        builder.push_text(&text[40..50]);

        builder.push_inline_box(InlineBox {
            id: 0,
            index: 50,
            width: 50.0,
            height: 30.0,
        });

        builder.push_text(&text[50..]);

        // Build the builder into a Layout
        // let mut layout: Layout<Color> = builder.build(&text);
        let (layout, _text): (Layout<Color>, String) = builder.build();
        layout
    } else {
        // RANGE BUILDER
        // ============

        // Creatse a RangedBuilder
        let mut builder = layout_cx.ranged_builder(&mut font_cx, &text, display_scale);

        // Set default text colour styles (set foreground text color)
        builder.push_default(&brush_style);

        // Set default font family
        builder.push_default(&font_stack_style);
        builder.push_default(&StyleProperty::LineHeight(1.3));
        builder.push_default(&StyleProperty::FontSize(16.0));

        // Set the first 4 characters to bold
        builder.push(&bold_style, 0..4);

        builder.push_inline_box(InlineBox {
            id: 0,
            index: 40,
            width: 50.0,
            height: 50.0,
        });
        builder.push_inline_box(InlineBox {
            id: 0,
            index: 50,
            width: 50.0,
            height: 30.0,
        });

        // Build the builder into a Layout
        // let mut layout: Layout<Color> = builder.build(&text);
        let layout: Layout<Color> = builder.build(&text);
        layout
    };

    // Perform layout (including bidi resolution and shaping) with start alignment
    layout.break_all_lines(max_advance, Alignment::Start);

    // Create image to render into
    let width = layout.width().ceil() as u32 + (padding * 2);
    let height = layout.height().ceil() as u32 + (padding * 2);
    let mut img = RgbaImage::from_pixel(width, height, bg_color);

    // Iterate over laid out lines
    for line in layout.lines() {
        // Iterate over GlyphRun's within each line
        for item in line.items() {
            match item {
                LayoutItem2::GlyphRun(glyph_run) => {
                    render_glyph_run(&mut scale_cx, &glyph_run, &mut img, padding);
                }
                LayoutItem2::InlineBox(inline_box) => {
                    for x_off in 0..(inline_box.width.floor() as u32) {
                        for y_off in 0..(inline_box.height.floor() as u32) {
                            let x = inline_box.x as u32 + x_off + padding;
                            let y = inline_box.y as u32 + y_off + padding;
                            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                        }
                    }
                }
            };
        }
    }

    // Write image to PNG file in examples/_output dir
    let output_path = {
        let path = std::path::PathBuf::from(file!());
        let mut path = std::fs::canonicalize(path).unwrap();
        path.pop();
        path.pop();
        path.pop();
        path.push("_output");
        let _ = std::fs::create_dir(path.clone());
        path.push("swash_render.png");
        path
    };
    let output_file = File::create(output_path).unwrap();
    let png_encoder = PngEncoder::new(output_file);
    img.write_with_encoder(png_encoder).unwrap();
}

fn render_glyph_run(
    context: &mut ScaleContext,
    glyph_run: &GlyphRun<Color>,
    img: &mut RgbaImage,
    padding: u32,
) {
    // Resolve properties of the GlyphRun
    let mut run_x = glyph_run.offset();
    let run_y = glyph_run.baseline();
    let style = glyph_run.style();
    let color = style.brush;

    // Get the "Run" from the "GlyphRun"
    let run = glyph_run.run();

    // Resolve properties of the Run
    let font = run.font();
    let font_size = run.font_size();
    let normalized_coords = run.normalized_coords();

    // Convert from parley::Font to swash::FontRef
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index as usize).unwrap();

    // Build a scaler. As the font properties are constant across an entire run of glyphs
    // we can build one scaler for the run and reuse it for each glyph.
    let mut scaler = context
        .builder(font_ref)
        .size(font_size)
        .hint(true)
        .normalized_coords(normalized_coords)
        .build();

    // Iterates over the glyphs in the GlyphRun
    for glyph in glyph_run.glyphs() {
        let glyph_x = run_x + glyph.x + (padding as f32);
        let glyph_y = run_y - glyph.y + (padding as f32);
        run_x += glyph.advance;

        render_glyph(img, &mut scaler, color, glyph, glyph_x, glyph_y);
    }
}

fn render_glyph(
    img: &mut RgbaImage,
    scaler: &mut Scaler,
    color: Color,
    glyph: Glyph,
    glyph_x: f32,
    glyph_y: f32,
) {
    // Compute the fractional offset
    // You'll likely want to quantize this in a real renderer
    let offset = Vector::new(glyph_x.fract(), glyph_y.fract());

    // Render the glyph using swash
    let rendered_glyph = Render::new(
        // Select our source order
        &[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ],
    )
    // Select the simple alpha (non-subpixel) format
    .format(Format::Alpha)
    // Apply the fractional offset
    .offset(offset)
    // Render the image
    .render(scaler, glyph.id)
    .unwrap();

    let glyph_width = rendered_glyph.placement.width;
    let glyph_height = rendered_glyph.placement.height;
    let glyph_x = (glyph_x.floor() as i32 + rendered_glyph.placement.left) as u32;
    let glyph_y = (glyph_y.floor() as i32 - rendered_glyph.placement.top) as u32;

    match rendered_glyph.content {
        Content::Mask => {
            let mut i = 0;
            for pixel_y in 0..glyph_height {
                for pixel_x in 0..glyph_width {
                    let x = glyph_x + pixel_x;
                    let y = glyph_y + pixel_y;
                    let alpha = rendered_glyph.data[i];
                    let color = Rgba([color.r, color.g, color.b, alpha]);
                    img.get_pixel_mut(x, y).blend(&color);
                    i += 1;
                }
            }
        }
        Content::SubpixelMask => unimplemented!(),
        Content::Color => {
            let row_size = glyph_width as usize * 4;
            for (pixel_y, row) in rendered_glyph.data.chunks_exact(row_size).enumerate() {
                for (pixel_x, pixel) in row.chunks_exact(4).enumerate() {
                    let x = glyph_x + pixel_x as u32;
                    let y = glyph_y + pixel_y as u32;
                    let color = Rgba(pixel.try_into().expect("Not RGBA"));
                    img.get_pixel_mut(x, y).blend(&color);
                }
            }
        }
    };
}
