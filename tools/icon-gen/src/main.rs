//! Generates the Postbin Ultra app icon at every size needed for both the
//! macOS `.icns` bundle and the runtime window icon.
//!
//! Run with `cargo run -p icon-gen`. Outputs go to:
//!   crates/postbin-ultra-desktop/assets/icons/
//!     ├── window-icon.png    (256×256, embedded into the binary)
//!     ├── icon-1024.png      (1024×1024, source for the .icns)
//!     └── AppIcon.iconset/   (iconutil input — produced by hand-rolled here)
//!
//! The drawing is intentionally compact: a Mac-style rounded "squircle"
//! background, a soft radial highlight, and a chunky white play arrow with a
//! drop shadow. Rendered with tiny-skia for clean anti-aliasing, no system
//! dependencies.

use std::fs;
use std::path::{Path, PathBuf};

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, Path as SkiaPath, PathBuilder, Pixmap,
    Point, RadialGradient, Rect, SpreadMode, Stroke, Transform,
};

const ICON_DIR: &str = "crates/postbin-ultra-desktop/assets/icons";
const ICONSET_DIR: &str = "crates/postbin-ultra-desktop/assets/icons/AppIcon.iconset";
const WINDOW_ICON: &str = "crates/postbin-ultra-desktop/assets/icons/window-icon.png";
const SOURCE_PNG: &str = "crates/postbin-ultra-desktop/assets/icons/icon-1024.png";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(ICON_DIR)?;
    fs::create_dir_all(ICONSET_DIR)?;

    // Render the master 1024×1024 first, then resample for every .icns size
    // and the runtime window icon. tiny-skia downscales with bilinear filtering
    // which is plenty for icon sizes; Apple does not require Lanczos here.
    let master = render_icon(1024);
    write_png(&master, Path::new(SOURCE_PNG))?;

    // .icns sizes Apple expects in an iconset bundle.
    let icns_sizes: &[(&str, u32)] = &[
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];
    for (name, size) in icns_sizes {
        let p = render_icon(*size);
        write_png(&p, &PathBuf::from(ICONSET_DIR).join(name))?;
    }

    // Window icon — embedded into the binary at compile time.
    let window = render_icon(256);
    write_png(&window, Path::new(WINDOW_ICON))?;

    println!("rendered icons to {ICON_DIR}");
    println!("next: run `iconutil -c icns {ICONSET_DIR}` to produce AppIcon.icns");
    Ok(())
}

fn render_icon(size: u32) -> Pixmap {
    let mut pix = Pixmap::new(size, size).expect("pixmap");
    let s = size as f32;
    let inset = s * 0.0625; // 6.25% margin around the squircle
    let inner = s - inset * 2.0;
    let radius = inner * 0.2237; // Apple-ish squircle approximation

    // 1) Background squircle with a lavender→indigo gradient.
    let bg_rect = Rect::from_xywh(inset, inset, inner, inner).expect("bg rect");
    let bg_path = rounded_rect(bg_rect, radius);
    let bg_paint = Paint {
        shader: LinearGradient::new(
            Point::from_xy(inset, inset),
            Point::from_xy(inset + inner, inset + inner),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(0x8a, 0x99, 0xff, 0xff)),
                GradientStop::new(0.55, Color::from_rgba8(0x7c, 0x8c, 0xff, 0xff)),
                GradientStop::new(1.0, Color::from_rgba8(0x4a, 0x55, 0xb3, 0xff)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .expect("bg gradient"),
        anti_alias: true,
        ..Paint::default()
    };
    pix.fill_path(
        &bg_path,
        &bg_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // 2) Soft top-left highlight for that Big-Sur-y sheen.
    let highlight_paint = Paint {
        shader: RadialGradient::new(
            Point::from_xy(s * 0.5, s * 0.32),
            Point::from_xy(s * 0.5, s * 0.32),
            s * 0.55,
            vec![
                GradientStop::new(0.0, Color::from_rgba8(0xff, 0xff, 0xff, 0x40)),
                GradientStop::new(1.0, Color::from_rgba8(0xff, 0xff, 0xff, 0x00)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .expect("highlight gradient"),
        anti_alias: true,
        ..Paint::default()
    };
    pix.fill_path(
        &bg_path,
        &highlight_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // 3) Play-arrow drop shadow (drawn before the white arrow itself).
    let arrow = play_arrow(s);
    let shadow_offset = s * 0.012;
    let shadow_paint = Paint {
        shader: tiny_skia::Shader::SolidColor(Color::from_rgba8(0, 0, 0, 0x55)),
        anti_alias: true,
        ..Paint::default()
    };
    pix.fill_path(
        &arrow,
        &shadow_paint,
        FillRule::Winding,
        Transform::from_translate(0.0, shadow_offset),
        None,
    );

    // 4) White play arrow.
    let white_paint = Paint {
        shader: tiny_skia::Shader::SolidColor(Color::WHITE),
        anti_alias: true,
        ..Paint::default()
    };
    pix.fill_path(
        &arrow,
        &white_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // 5) Subtle 1px inner ring for definition at small sizes.
    let ring_paint = Paint {
        shader: tiny_skia::Shader::SolidColor(Color::from_rgba8(0xff, 0xff, 0xff, 0x30)),
        anti_alias: true,
        ..Paint::default()
    };
    let stroke = Stroke {
        width: (s / 1024.0).max(1.0),
        ..Stroke::default()
    };
    pix.stroke_path(&bg_path, &ring_paint, &stroke, Transform::identity(), None);

    pix
}

fn rounded_rect(r: Rect, radius: f32) -> SkiaPath {
    let mut pb = PathBuilder::new();
    let radius = radius.min(r.width() / 2.0).min(r.height() / 2.0);
    let (x, y, w, h) = (r.x(), r.y(), r.width(), r.height());
    pb.move_to(x + radius, y);
    pb.line_to(x + w - radius, y);
    pb.cubic_to(x + w, y, x + w, y, x + w, y + radius);
    pb.line_to(x + w, y + h - radius);
    pb.cubic_to(x + w, y + h, x + w, y + h, x + w - radius, y + h);
    pb.line_to(x + radius, y + h);
    pb.cubic_to(x, y + h, x, y + h, x, y + h - radius);
    pb.line_to(x, y + radius);
    pb.cubic_to(x, y, x, y, x + radius, y);
    pb.close();
    pb.finish().expect("rounded rect path")
}

/// Centred play-arrow path. Width and height are derived from `size` so the
/// arrow scales cleanly with the canvas.
fn play_arrow(size: f32) -> SkiaPath {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let half_w = size * 0.18;
    let half_h = size * 0.22;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - half_w, cy - half_h);
    pb.line_to(cx + half_w * 1.6, cy);
    pb.line_to(cx - half_w, cy + half_h);
    pb.close();
    pb.finish().expect("play arrow path")
}

fn write_png(pix: &Pixmap, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    pix.save_png(path)?;
    Ok(())
}
