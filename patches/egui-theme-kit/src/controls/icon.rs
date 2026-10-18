//! Icon slot every control accepts: a font glyph, a tinted static image or a painter function.

use eframe::egui::{
    Align2, Color32, FontFamily, FontId, Image, ImageSource, Painter, Pos2, Rect, Response, Sense,
    Ui, Vec2, load::Bytes,
};

/// A static image asset, tinted when drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaticImage {
    /// Unique loader uri of the asset.
    pub uri: &'static str,
    /// Encoded bytes; a white or monochrome SVG tints cleanly.
    pub bytes: &'static [u8],
}

impl StaticImage {
    pub const fn new(uri: &'static str, bytes: &'static [u8]) -> Self {
        Self { uri, bytes }
    }

    /// The egui image source the loaders read.
    pub fn source(self) -> ImageSource<'static> {
        ImageSource::Bytes {
            uri: self.uri.into(),
            bytes: Bytes::Static(self.bytes),
        }
    }
}

/// Draws an icon of `rect`'s size in `color`.
pub type PaintFn = fn(&Painter, Rect, Color32);

/// One icon drawn into a square slot; the kit never depends on an icon crate.
#[derive(Clone, Copy, Debug)]
pub enum Icon {
    /// A glyph of an installed icon font, drawn as text.
    Glyph(&'static str),
    /// A static image tinted with the slot colour; needs egui image loaders for SVG.
    Image(StaticImage),
    /// Two image layers: a back layer tinted `bg` under a front layer tinted `fg`.
    Duotone {
        front: StaticImage,
        back: StaticImage,
    },
    /// A function that paints into the slot rect.
    Painted(PaintFn),
}

impl Icon {
    /// A glyph of an installed icon font.
    pub const fn glyph(glyph: &'static str) -> Self {
        Self::Glyph(glyph)
    }

    /// A tinted static image.
    pub const fn image(uri: &'static str, bytes: &'static [u8]) -> Self {
        Self::Image(StaticImage::new(uri, bytes))
    }

    /// A tinted image from an `ImageSource` holding static bytes, as `include_image!` produces.
    pub fn from_source(source: &ImageSource<'static>) -> Option<Self> {
        match source {
            ImageSource::Bytes {
                uri,
                bytes: Bytes::Static(bytes),
            } => match uri {
                std::borrow::Cow::Borrowed(uri) => Some(Self::image(uri, bytes)),
                std::borrow::Cow::Owned(_) => None,
            },
            _ => None,
        }
    }

    /// A duotone image pair.
    pub const fn duotone(front: StaticImage, back: StaticImage) -> Self {
        Self::Duotone { front, back }
    }

    /// An icon drawn by `paint`.
    pub const fn painted(paint: PaintFn) -> Self {
        Self::Painted(paint)
    }

    /// The font glyph, when this icon is one.
    pub const fn as_glyph(self) -> Option<&'static str> {
        match self {
            Self::Glyph(glyph) => Some(glyph),
            _ => None,
        }
    }

    /// The front image, when this icon has one.
    pub const fn front_image(self) -> Option<StaticImage> {
        match self {
            Self::Image(image) | Self::Duotone { front: image, .. } => Some(image),
            _ => None,
        }
    }

    /// Untinted image fixed at `size` points square; none for glyph and painted icons.
    pub fn image_widget(self, size: f32) -> Option<Image<'static>> {
        self.front_image()
            .map(|image| Image::new(image.source()).fit_to_exact_size(Vec2::splat(size)))
    }

    /// Allocates a `size` square and paints the icon in the current text colour.
    pub fn show(self, ui: &mut Ui, size: f32) -> Response {
        let tint = ui.visuals().text_color();
        self.show_tinted(ui, size, tint)
    }

    /// Allocates a `size` square and paints the icon tinted `tint`.
    pub fn show_tinted(self, ui: &mut Ui, size: f32, tint: Color32) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, size, tint);
        }
        response
    }

    /// Paints the icon `size` points square, centred in `rect` and snapped to whole pixels.
    pub fn paint(self, ui: &Ui, rect: Rect, size: f32, tint: Color32) {
        let target = snapped_square(rect.center(), size, ui.pixels_per_point());
        self.paint_at(ui, target, tint);
    }

    /// Paints the back layer tinted `bg` under the front layer tinted `fg`; other kinds paint `fg` only.
    pub fn paint_duo(self, ui: &Ui, rect: Rect, size: f32, fg: Color32, bg: Color32) {
        let target = snapped_square(rect.center(), size, ui.pixels_per_point());
        if let Self::Duotone { back, .. } = self {
            Image::new(back.source()).tint(bg).paint_at(ui, target);
        }
        self.paint_at(ui, target, fg);
    }

    fn paint_at(self, ui: &Ui, target: Rect, tint: Color32) {
        match self {
            Self::Glyph(glyph) => {
                let font = FontId::new(target.height(), FontFamily::Proportional);
                ui.painter()
                    .text(target.center(), Align2::CENTER_CENTER, glyph, font, tint);
            }
            Self::Image(image) | Self::Duotone { front: image, .. } => {
                Image::new(image.source()).tint(tint).paint_at(ui, target);
            }
            Self::Painted(paint) => paint(ui.painter(), target, tint),
        }
    }
}

impl From<&'static str> for Icon {
    fn from(glyph: &'static str) -> Self {
        Self::Glyph(glyph)
    }
}

impl From<PaintFn> for Icon {
    fn from(paint: PaintFn) -> Self {
        Self::Painted(paint)
    }
}

/// Square of a whole number of physical pixels whose origin sits on the pixel grid.
fn snapped_square(center: Pos2, size: f32, pixels_per_point: f32) -> Rect {
    let px = (size * pixels_per_point).round().max(1.0);
    let min = Pos2::new(
        (center.x * pixels_per_point - px / 2.0).round() / pixels_per_point,
        (center.y * pixels_per_point - px / 2.0).round() / pixels_per_point,
    );
    Rect::from_min_size(min, Vec2::splat(px / pixels_per_point))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, Shape};

    #[test]
    fn a_snapped_square_lands_on_whole_pixels() {
        let square = snapped_square(Pos2::new(10.3, 7.7), 14.0, 1.5);
        for edge in [square.min.x, square.min.y, square.max.x, square.max.y] {
            let px = edge * 1.5;
            assert!((px - px.round()).abs() < 1e-3, "{edge}");
        }
        assert!((square.width() - 14.0).abs() < 0.5);
    }

    #[test]
    fn static_sources_round_trip_and_others_do_not() {
        let icon = Icon::image("bytes://a.svg", b"<svg/>");
        let source = icon.front_image().map(StaticImage::source);
        let back = source.as_ref().and_then(Icon::from_source);
        assert_eq!(back.and_then(|i| i.front_image()), icon.front_image());
        let owned = ImageSource::Bytes {
            uri: String::from("bytes://b.svg").into(),
            bytes: Bytes::Static(b"<svg/>"),
        };
        assert!(Icon::from_source(&owned).is_none());
    }

    const RING: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="none" stroke="#fff" stroke-width="2"/></svg>"##;

    #[test]
    fn an_svg_icon_paints_one_textured_rect_and_a_duotone_paints_two() {
        let ctx = Context::default();
        egui_extras::install_image_loaders(&ctx);
        let ring = StaticImage::new("bytes://ring.svg", RING);
        let back = StaticImage::new("bytes://ring-back.svg", RING);
        let textured = |icon: Icon| {
            let mut count = 0;
            for _ in 0..3 {
                let mut full = ctx.run_ui(Default::default(), |ui| {
                    let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::splat(16.0));
                    icon.paint_duo(ui, rect, 16.0, Color32::WHITE, Color32::GRAY);
                });
                full.textures_delta.clear();
                count = full
                    .shapes
                    .iter()
                    .filter(|s| matches!(&s.shape, Shape::Rect(r) if r.brush.is_some()))
                    .count();
            }
            count
        };
        assert_eq!(textured(Icon::Image(ring)), 1);
        assert_eq!(textured(Icon::duotone(ring, back)), 2);
    }

    #[test]
    fn only_glyphs_report_a_glyph() {
        assert_eq!(Icon::glyph("g").as_glyph(), Some("g"));
        assert_eq!(Icon::from("g").as_glyph(), Some("g"));
        assert_eq!(super::super::icons::CHECK.as_glyph(), None);
    }
}
