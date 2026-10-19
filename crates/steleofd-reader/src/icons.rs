//! The symbolic icon set: line drawings on a 16-unit grid, in the window's
//! colours. Geometry rather than image files, so a theme switch recolours them
//! and no raster ships. The grid is the named `-symbolic` one: 1.5-unit stroke.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2};

use crate::theme::{self, Token};

/// The square every glyph is drawn on, before the reader's scale.
pub const SIZE: f32 = 16.0;

/// The stroke width the symbolic set uses, on that square.
pub const STROKE: f32 = 1.5;

/// The edge an icon is drawn at, in points, before the reader's scale.
pub const BOX: f32 = 20.0;

/// One of the window's icons. Where the desktop's icon theme names the same
/// idea, the name is reused, so a reader who knows the theme knows these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    /// Open a file.
    Open,
    /// Close the open document.
    Close,
    /// Search the document.
    Search,
    /// The signature report.
    Signatures,
    /// The annotation list.
    Annotations,
    /// The main menu: three bars, as the theme draws it.
    Menu,
    /// The previous page or entry.
    Previous,
    /// The next page or entry.
    Next,
    /// One page at a time.
    SinglePage,
    /// Two pages facing.
    FacingPages,
    /// The document scrolling.
    Continuous,
    /// Fit the width.
    FitWidth,
    /// Fit the whole page.
    FitPage,
    /// Print.
    Print,
    /// Write a PDF.
    ExportPdf,
    /// Write PNG files.
    ExportPng,
    /// A signature that holds.
    Valid,
    /// A signature that does not hold.
    Tampered,
    /// A proof nothing can confirm.
    Unverified,
    /// A sound proof whose signer is not trusted.
    Untrusted,
}

impl Icon {
    /// Every icon, so a test can draw all of them.
    pub const ALL: [Self; 20] = [
        Self::Open,
        Self::Close,
        Self::Search,
        Self::Signatures,
        Self::Annotations,
        Self::Menu,
        Self::Previous,
        Self::Next,
        Self::SinglePage,
        Self::FacingPages,
        Self::Continuous,
        Self::FitWidth,
        Self::FitPage,
        Self::Print,
        Self::ExportPdf,
        Self::ExportPng,
        Self::Valid,
        Self::Tampered,
        Self::Unverified,
        Self::Untrusted,
    ];

    /// The name an icon theme gives the same idea. It is written down here so a
    /// maintainer adding a glyph can look the standard one up first.
    pub fn theme_name(self) -> &'static str {
        match self {
            Self::Open => "document-open-symbolic",
            Self::Close => "window-close-symbolic",
            Self::Search => "edit-find-symbolic",
            Self::Signatures => "application-certificate-symbolic",
            Self::Annotations => "view-list-symbolic",
            Self::Menu => "open-menu-symbolic",
            Self::Previous => "go-previous-symbolic",
            Self::Next => "go-next-symbolic",
            Self::SinglePage => "view-paged-symbolic",
            Self::FacingPages => "view-dual-symbolic",
            Self::Continuous => "view-continuous-symbolic",
            Self::FitWidth => "zoom-fit-best-symbolic",
            Self::FitPage => "zoom-fit-page-symbolic",
            Self::Print => "document-print-symbolic",
            Self::ExportPdf => "document-save-symbolic",
            Self::ExportPng => "image-x-generic-symbolic",
            Self::Valid => "emblem-ok-symbolic",
            Self::Tampered => "dialog-error-symbolic",
            Self::Unverified => "dialog-question-symbolic",
            Self::Untrusted => "dialog-warning-symbolic",
        }
    }
}

/// One part of a glyph: a straight line by default, or a rounded shape.
enum Part {
    /// A line between two points on the grid.
    Line((f32, f32), (f32, f32)),
    /// An outlined box, `(x0, y0)` to `(x1, y1)`, with `radius` in units.
    Box((f32, f32), (f32, f32), f32),
    /// An outlined circle at a point, with a radius in units.
    Circle((f32, f32), f32),
    /// A filled dot, for a list bullet.
    Dot((f32, f32)),
    /// An open polyline through the points.
    Path(&'static [(f32, f32)]),
    /// A closed outline through the points.
    Closed(&'static [(f32, f32)]),
}

use Part::{Box as Box_, Circle as Circle_, Closed, Dot, Line, Path};

/// The glyph for each icon. Written as one table so every shape is in one place
/// and a reviewer can see a whole icon before reading any code.
fn parts(icon: Icon) -> &'static [Part] {
    match icon {
        Icon::Open => &[
            Box_((4.0, 1.5), (12.0, 10.5), 1.0),
            Box_((1.5, 6.0), (14.5, 14.5), 1.0),
        ],
        Icon::Close => &[
            Line((4.0, 4.0), (12.0, 12.0)),
            Line((12.0, 4.0), (4.0, 12.0)),
        ],
        Icon::Search => &[Circle_((6.8, 6.8), 4.4), Line((10.0, 10.0), (14.0, 14.0))],
        Icon::Signatures => &[
            Box_((2.5, 2.0), (13.5, 14.0), 1.5),
            Circle_((8.0, 7.0), 2.2),
            Line((6.5, 13.0), (6.5, 10.5)),
            Line((9.5, 13.0), (9.5, 10.5)),
        ],
        Icon::Annotations => &[
            Line((4.0, 3.0), (14.0, 3.0)),
            Dot((2.0, 3.0)),
            Line((4.0, 8.0), (14.0, 8.0)),
            Dot((2.0, 8.0)),
            Line((4.0, 13.0), (14.0, 13.0)),
            Dot((2.0, 13.0)),
        ],
        Icon::Menu => &[
            Line((2.5, 4.0), (13.5, 4.0)),
            Line((2.5, 8.0), (13.5, 8.0)),
            Line((2.5, 12.0), (13.5, 12.0)),
        ],
        Icon::Previous => &[
            Line((13.5, 8.0), (4.0, 8.0)),
            Line((4.0, 8.0), (8.5, 3.5)),
            Line((4.0, 8.0), (8.5, 12.5)),
        ],
        Icon::Next => &[
            Line((2.5, 8.0), (12.0, 8.0)),
            Line((12.0, 8.0), (7.5, 3.5)),
            Line((12.0, 8.0), (7.5, 12.5)),
        ],
        Icon::SinglePage => &[Box_((3.5, 1.5), (12.5, 14.5), 1.0)],
        Icon::FacingPages => &[
            Box_((1.5, 2.0), (7.5, 14.5), 1.0),
            Box_((8.5, 2.0), (14.5, 14.5), 1.0),
        ],
        Icon::Continuous => &[
            Box_((3.0, 1.5), (13.0, 5.5), 0.8),
            Box_((3.0, 6.0), (13.0, 10.0), 0.8),
            Box_((3.0, 10.5), (13.0, 14.5), 0.8),
        ],
        Icon::FitWidth => &[
            Box_((5.0, 2.0), (11.0, 14.0), 1.0),
            Line((1.5, 8.0), (4.0, 5.5)),
            Line((1.5, 8.0), (4.0, 10.5)),
            Line((14.5, 8.0), (12.0, 5.5)),
            Line((14.5, 8.0), (12.0, 10.5)),
        ],
        Icon::FitPage => &[
            Box_((2.0, 5.0), (14.0, 11.0), 1.0),
            Line((8.0, 1.5), (5.5, 4.0)),
            Line((8.0, 1.5), (10.5, 4.0)),
            Line((8.0, 14.5), (5.5, 12.0)),
            Line((8.0, 14.5), (10.5, 12.0)),
        ],
        Icon::Print => &[
            Box_((2.0, 5.5), (14.0, 12.5), 1.0),
            Box_((4.5, 1.5), (11.5, 7.0), 0.5),
            Line((4.5, 10.0), (11.5, 10.0)),
        ],
        Icon::ExportPdf => &[
            Box_((3.0, 1.5), (13.0, 14.5), 1.0),
            Line((8.0, 4.5), (8.0, 10.5)),
            Line((8.0, 10.5), (5.5, 8.0)),
            Line((8.0, 10.5), (10.5, 8.0)),
        ],
        Icon::ExportPng => &[
            Box_((1.5, 2.5), (14.5, 13.5), 1.0),
            Circle_((5.5, 6.0), 1.2),
            Path(&[
                (2.5, 12.5),
                (6.5, 7.5),
                (9.5, 10.5),
                (11.5, 8.5),
                (13.5, 12.5),
            ]),
        ],
        Icon::Valid => &[
            Circle_((8.0, 8.0), 6.0),
            Path(&[(5.0, 8.2), (7.2, 10.5), (11.0, 5.8)]),
        ],
        Icon::Tampered => &[
            Circle_((8.0, 8.0), 6.0),
            Line((5.5, 5.5), (10.5, 10.5)),
            Line((10.5, 5.5), (5.5, 10.5)),
        ],
        Icon::Unverified => &[
            Circle_((8.0, 8.0), 6.0),
            // The question mark's hook, drawn as a short arc of points.
            Path(&[
                (6.0, 6.3),
                (6.4, 5.4),
                (7.2, 5.0),
                (8.4, 5.2),
                (9.0, 6.0),
                (8.8, 7.0),
                (8.0, 7.6),
                (7.8, 8.4),
            ]),
            Line((7.8, 10.3), (7.8, 10.5)),
        ],
        Icon::Untrusted => &[
            Closed(&[(8.0, 2.0), (14.5, 14.0), (1.5, 14.0)]),
            Line((8.0, 6.5), (8.0, 9.5)),
            Line((8.0, 11.6), (8.0, 11.8)),
        ],
    }
}

/// Paint one icon into `rect`, in `colour`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, colour: Color32) {
    let unit = rect.width().min(rect.height()) / SIZE;
    let at = |(x, y): (f32, f32)| -> Pos2 { rect.min + Vec2::new(x * unit, y * unit) };
    let stroke = Stroke::new((STROKE * unit).max(1.0), colour);
    for part in parts(icon) {
        match *part {
            Line(from, to) => {
                painter.line_segment([at(from), at(to)], stroke);
            }
            Box_(from, to, radius) => {
                let frame = Rect::from_min_max(at(from), at(to));
                painter.rect_stroke(frame, radius * unit, stroke, egui::StrokeKind::Inside);
            }
            Circle_(centre, radius) => {
                painter.circle_stroke(at(centre), radius * unit, stroke);
            }
            Dot(point) => {
                painter.circle_filled(at(point), STROKE * unit * 0.75, colour);
            }
            Path(points) => {
                let points: Vec<Pos2> = points.iter().map(|p| at(*p)).collect();
                painter.add(Shape::line(points, stroke));
            }
            Closed(points) => {
                let points: Vec<Pos2> = points.iter().map(|p| at(*p)).collect();
                painter.add(Shape::closed_line(points, stroke));
            }
        }
    }
}

/// The edge an icon occupies at the reader's scale, for a layout to reserve.
pub fn box_size(scale: f32) -> Vec2 {
    Vec2::splat((BOX * scale).round().max(1.0))
}

/// Draw an icon as a widget of its own, in the window's text colour.
pub fn show(ui: &mut egui::Ui, icon: Icon) -> Response {
    let colour = theme::colour(ui.ctx(), false, Token::Text);
    show_toned(ui, icon, colour)
}

/// The same in a named colour, for a glyph that stands for a verdict.
pub fn show_toned(ui: &mut egui::Ui, icon: Icon, colour: Color32) -> Response {
    let side = box_size((ui.spacing().interact_size.y / CONTROL_RATIO).max(0.6)).x;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect.shrink(side * 0.1), icon, colour);
    }
    response
}

/// The edge a glyph is rasterized at, in pixels. Four times the grid, so the
/// stroke lands on whole pixels at the sizes the window draws and the picture
/// stays sharp when a screen scales the window up.
const RASTER: u32 = (SIZE as u32) * 4;

/// The glyph as a texture, for the places that need a picture: a button lays
/// its contents out before it knows where they will land. One per icon and
/// colour is cached in the context, so a frame uploads nothing new.
pub fn texture(ctx: &egui::Context, icon: Icon, colour: Color32) -> egui::TextureHandle {
    let key = egui::Id::new(("icon-texture", icon as u8, colour.to_array()));
    if let Some(handle) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(key)) {
        return handle;
    }
    let image = ctx.load_texture(
        format!("icon-{}", icon.theme_name()),
        rasterize(icon, colour),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(key, image.clone()));
    image
}

/// Paint one glyph into an image, transparent where the ink is not, from the
/// same parts the painted form uses, so the two cannot drift apart.
fn rasterize(icon: Icon, colour: Color32) -> egui::ColorImage {
    let vertices = triangles(icon, colour);
    let size = [RASTER as usize, RASTER as usize];
    let mut pixels = vec![Color32::TRANSPARENT; size[0] * size[1]];
    for triangle in vertices.as_chunks::<3>().0 {
        let points = [
            (triangle[0].x, triangle[0].y),
            (triangle[1].x, triangle[1].y),
            (triangle[2].x, triangle[2].y),
        ];
        cover(&mut pixels, size, points, colour);
    }
    egui::ColorImage::new(size, pixels)
}

/// The glyph tessellated in its own pixel space, for the rasterizer to fill.
fn triangles(icon: Icon, colour: Color32) -> Vec<Pos2> {
    use eframe::egui::epaint::{
        CircleShape, ClippedShape, PathShape, Primitive, TessellationOptions, Tessellator,
    };

    let unit = RASTER as f32 / SIZE;
    let at = |(x, y): (f32, f32)| -> Pos2 { Pos2::new(x * unit, y * unit) };
    let stroke = Stroke::new((STROKE * unit).max(1.0), colour);
    let mut shapes: Vec<Shape> = Vec::new();
    for part in parts(icon) {
        shapes.push(match *part {
            Line(from, to) => Shape::line_segment([at(from), at(to)], stroke),
            Box_(from, to, radius) => Shape::rect_stroke(
                Rect::from_min_max(at(from), at(to)),
                radius * unit,
                stroke,
                egui::StrokeKind::Inside,
            ),
            Circle_(centre, radius) => {
                Shape::Circle(CircleShape::stroke(at(centre), radius * unit, stroke))
            }
            Dot(point) => {
                Shape::Circle(CircleShape::filled(at(point), STROKE * unit * 0.75, colour))
            }
            Path(points) => Shape::Path(PathShape::line(
                points.iter().map(|p| at(*p)).collect(),
                stroke,
            )),
            Closed(points) => Shape::Path(PathShape::closed_line(
                points.iter().map(|p| at(*p)).collect(),
                stroke,
            )),
        });
    }
    let clipped: Vec<ClippedShape> = shapes
        .into_iter()
        .map(|shape| ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape,
        })
        .collect();
    let mut primitives = Tessellator::new(
        1.0,
        TessellationOptions::default(),
        [RASTER as usize, RASTER as usize],
        Vec::new(),
    )
    .tessellate_shapes(clipped);
    primitives
        .drain(..)
        .filter_map(|primitive| match primitive.primitive {
            Primitive::Mesh(mesh) => Some(mesh),
            _ => None,
        })
        .flat_map(|mesh| {
            let at = |index: u32| mesh.vertices[index as usize].pos;
            mesh.indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|turn| [at(turn[0]), at(turn[1]), at(turn[2])])
                .collect::<Vec<[Pos2; 3]>>()
        })
        .flatten()
        .collect()
}

/// Fill the pixels one triangle covers, in `colour`.
fn cover(pixels: &mut [Color32], size: [usize; 2], points: [(f32, f32); 3], colour: Color32) {
    let area = edge(points[0], points[1], points[2]);
    if area == 0.0 {
        return;
    }
    let lo_x = points.iter().map(|p| p.0).fold(f32::MAX, f32::min);
    let hi_x = points.iter().map(|p| p.0).fold(f32::MIN, f32::max);
    let lo_y = points.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let hi_y = points.iter().map(|p| p.1).fold(f32::MIN, f32::max);
    let x0 = lo_x.max(0.0) as usize;
    let y0 = lo_y.max(0.0) as usize;
    let x1 = (hi_x.ceil() as usize).min(size[0]);
    let y1 = (hi_y.ceil() as usize).min(size[1]);
    for y in y0..y1 {
        for x in x0..x1 {
            let at = (x as f32 + 0.5, y as f32 + 0.5);
            // Inside means on the same side of all three edges, which is the
            // sign test every rasterizer starts from.
            let sides = [
                edge(points[1], points[2], at),
                edge(points[2], points[0], at),
                edge(points[0], points[1], at),
            ];
            if sides.iter().all(|side| side.signum() == area.signum()) {
                pixels[y * size[0] + x] = colour;
            }
        }
    }
}

/// Twice the signed area of the triangle three points make.
fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// The ratio between a control's height and the box an icon sits in, so a glyph
/// reads as part of a button rather than filling it edge to edge.
const CONTROL_RATIO: f32 = 2.2;
