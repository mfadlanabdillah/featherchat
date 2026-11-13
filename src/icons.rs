//! Hand-drawn line icons. Everything is painted with egui shapes, so the app
//! carries no icon font or bitmap assets.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

fn pt(r: Rect, x: f32, y: f32) -> Pos2 {
    r.min + Vec2::new(r.width() * x, r.height() * y)
}

fn line(painter: &Painter, r: Rect, points: &[(f32, f32)], color: Color32, width: f32) {
    let pts: Vec<Pos2> = points.iter().map(|&(x, y)| pt(r, x, y)).collect();
    painter.add(Shape::line(pts, Stroke::new(width, color)));
}

pub fn search(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.circle_stroke(pt(r, 0.43, 0.42), r.width() * 0.25, Stroke::new(w, color));
    painter.line_segment(
        [pt(r, 0.62, 0.61), pt(r, 0.83, 0.82)],
        Stroke::new(w, color),
    );
}

pub fn compose(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let stroke = Stroke::new(w, color);
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.14, 0.30), pt(r, 0.72, 0.86)),
        egui::Rounding::same(2.0),
        stroke,
    );
    line(
        painter,
        r,
        &[(0.52, 0.22), (0.84, 0.52), (0.60, 0.64), (0.44, 0.58)],
        color,
        w,
    );
    line(painter, r, &[(0.44, 0.58), (0.52, 0.22)], color, w);
}

pub fn dots(painter: &Painter, r: Rect, color: Color32, _w: f32) {
    for y in [0.24, 0.5, 0.76] {
        painter.circle_filled(pt(r, 0.5, y), r.width() * 0.06, color);
    }
}

pub fn send(painter: &Painter, r: Rect, color: Color32, _w: f32) {
    let a = pt(r, 0.88, 0.5);
    let b = pt(r, 0.12, 0.14);
    let c = pt(r, 0.36, 0.5);
    let d = pt(r, 0.12, 0.86);
    painter.add(Shape::convex_polygon(
        vec![a, b, c],
        color,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![a, c, d],
        color,
        Stroke::NONE,
    ));
}

pub fn paperclip(painter: &Painter, r: Rect, color: Color32, w: f32) {
    line(
        painter,
        r,
        &[
            (0.64, 0.16),
            (0.26, 0.56),
            (0.24, 0.70),
            (0.36, 0.82),
            (0.50, 0.78),
            (0.84, 0.44),
            (0.84, 0.28),
            (0.72, 0.16),
            (0.58, 0.28),
            (0.38, 0.50),
        ],
        color,
        w,
    );
}

pub fn smiley(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.circle_stroke(r.center(), r.width() * 0.38, Stroke::new(w, color));
    painter.circle_filled(pt(r, 0.36, 0.42), r.width() * 0.035, color);
    painter.circle_filled(pt(r, 0.64, 0.42), r.width() * 0.035, color);
    line(
        painter,
        r,
        &[(0.31, 0.62), (0.42, 0.71), (0.58, 0.71), (0.69, 0.62)],
        color,
        w,
    );
}

pub fn phone(painter: &Painter, r: Rect, color: Color32, w: f32) {
    line(
        painter,
        r,
        &[
            (0.24, 0.18),
            (0.40, 0.26),
            (0.42, 0.42),
            (0.55, 0.55),
            (0.72, 0.58),
            (0.80, 0.74),
            (0.70, 0.86),
            (0.54, 0.84),
            (0.18, 0.48),
            (0.15, 0.30),
            (0.24, 0.18),
        ],
        color,
        w * 1.15,
    );
}

pub fn video(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.10, 0.28), pt(r, 0.64, 0.72)),
        egui::Rounding::same(2.5),
        Stroke::new(w, color),
    );
    let a = pt(r, 0.67, 0.43);
    let b = pt(r, 0.88, 0.28);
    let c = pt(r, 0.88, 0.72);
    let d = pt(r, 0.67, 0.57);
    painter.add(Shape::convex_polygon(vec![a, b, c, d], color, Stroke::NONE));
}

pub fn info(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.circle_stroke(r.center(), r.width() * 0.38, Stroke::new(w, color));
    painter.circle_filled(pt(r, 0.5, 0.30), r.width() * 0.04, color);
    line(painter, r, &[(0.5, 0.44), (0.5, 0.70)], color, w);
}

pub fn back(painter: &Painter, r: Rect, color: Color32, w: f32) {
    line(
        painter,
        r,
        &[(0.70, 0.18), (0.36, 0.5), (0.70, 0.82)],
        color,
        w,
    );
    painter.line_segment([pt(r, 0.38, 0.5), pt(r, 0.86, 0.5)], Stroke::new(w, color));
}

pub fn close(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.line_segment([pt(r, 0.25, 0.25), pt(r, 0.75, 0.75)], Stroke::new(w, color));
    painter.line_segment([pt(r, 0.75, 0.25), pt(r, 0.25, 0.75)], Stroke::new(w, color));
}

pub fn ticks(painter: &Painter, r: Rect, color: Color32, w: f32, double: bool) {
    let stroke = Stroke::new(w, color);
    let first = [
        pt(r, 0.10, 0.55),
        pt(r, 0.34, 0.78),
        pt(r, 0.78, 0.22),
    ];
    painter.add(Shape::line(first.to_vec(), stroke));
    if double {
        let second = [
            pt(r, 0.42, 0.55),
            pt(r, 0.66, 0.78),
            pt(r, 0.96, 0.22),
        ];
        painter.add(Shape::line(second.to_vec(), stroke));
    }
}

pub fn clock(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.circle_stroke(r.center(), r.width() * 0.34, Stroke::new(w, color));
    line(painter, r, &[(0.5, 0.36), (0.5, 0.55), (0.66, 0.64)], color, w);
}

pub fn pin(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.circle_stroke(pt(r, 0.5, 0.40), r.width() * 0.17, Stroke::new(w, color));
    line(
        painter,
        r,
        &[(0.62, 0.52), (0.5, 0.86), (0.38, 0.52)],
        color,
        w,
    );
}

pub fn settings(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let stroke = Stroke::new(w, color);
    for (y, knob) in [(0.28, 0.62), (0.5, 0.36), (0.72, 0.7)] {
        painter.line_segment([pt(r, 0.14, y), pt(r, 0.86, y)], stroke);
        painter.circle_filled(pt(r, knob, y), r.width() * 0.075, color);
    }
}

pub fn mic(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.38, 0.12), pt(r, 0.62, 0.58)),
        egui::Rounding::same(8.0),
        Stroke::new(w, color),
    );
    line(
        painter,
        r,
        &[(0.24, 0.48), (0.28, 0.64), (0.40, 0.74), (0.60, 0.74), (0.72, 0.64), (0.76, 0.48)],
        color,
        w,
    );
    painter.line_segment([pt(r, 0.5, 0.74), pt(r, 0.5, 0.88)], Stroke::new(w, color));
    painter.line_segment([pt(r, 0.36, 0.88), pt(r, 0.64, 0.88)], Stroke::new(w, color));
}

pub fn photo(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.10, 0.20), pt(r, 0.90, 0.80)),
        egui::Rounding::same(2.5),
        Stroke::new(w, color),
    );
    painter.circle_stroke(pt(r, 0.32, 0.38), r.width() * 0.07, Stroke::new(w, color));
    line(
        painter,
        r,
        &[(0.14, 0.74), (0.36, 0.50), (0.52, 0.64), (0.66, 0.46), (0.86, 0.72)],
        color,
        w,
    );
}

pub fn document(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let stroke = Stroke::new(w, color);
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.22, 0.12), pt(r, 0.78, 0.88)),
        egui::Rounding::same(2.0),
        stroke,
    );
    line(
        painter,
        r,
        &[(0.34, 0.34), (0.66, 0.34)],
        color,
        w * 0.8,
    );
    line(
        painter,
        r,
        &[(0.34, 0.50), (0.66, 0.50)],
        color,
        w * 0.8,
    );
    line(
        painter,
        r,
        &[(0.34, 0.66), (0.56, 0.66)],
        color,
        w * 0.8,
    );
}

pub fn location(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.circle_stroke(pt(r, 0.5, 0.40), r.width() * 0.24, Stroke::new(w, color));
    line(
        painter,
        r,
        &[(0.30, 0.58), (0.5, 0.90), (0.70, 0.58)],
        color,
        w,
    );
    painter.circle_filled(pt(r, 0.5, 0.40), r.width() * 0.06, color);
}

pub fn chat_bubble(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let stroke = Stroke::new(w, color);
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.12, 0.16), pt(r, 0.88, 0.72)),
        egui::Rounding::same(7.0),
        stroke,
    );
    let tail = vec![
        pt(r, 0.30, 0.72),
        pt(r, 0.28, 0.90),
        pt(r, 0.48, 0.72),
    ];
    painter.add(Shape::convex_polygon(
        tail,
        color,
        Stroke::NONE,
    ));
    painter.circle_filled(pt(r, 0.34, 0.44), r.width() * 0.045, color);
    painter.circle_filled(pt(r, 0.50, 0.44), r.width() * 0.045, color);
    painter.circle_filled(pt(r, 0.66, 0.44), r.width() * 0.045, color);
}

pub fn moon(painter: &Painter, r: Rect, color: Color32, bg: Color32, w: f32) {
    let center = pt(r, 0.48, 0.5);
    let radius = r.width() * 0.32;
    painter.circle_filled(center, radius, color);
    painter.circle_filled(
        center + Vec2::new(r.width() * 0.15, -r.height() * 0.10),
        radius * 0.88,
        bg,
    );
    painter.circle_stroke(center, radius, Stroke::new(w, color));
}

pub fn sun(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let c = r.center();
    painter.circle_stroke(c, r.width() * 0.18, Stroke::new(w, color));
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = Vec2::new(a.cos(), a.sin());
        painter.line_segment(
            [c + dir * r.width() * 0.30, c + dir * r.width() * 0.40],
            Stroke::new(w, color),
        );
    }
}

pub fn plus(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.line_segment([pt(r, 0.5, 0.2), pt(r, 0.5, 0.8)], Stroke::new(w, color));
    painter.line_segment([pt(r, 0.2, 0.5), pt(r, 0.8, 0.5)], Stroke::new(w, color));
}

pub fn leaf(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let outline = vec![
        pt(r, 0.50, 0.10),
        pt(r, 0.78, 0.30),
        pt(r, 0.80, 0.58),
        pt(r, 0.50, 0.90),
        pt(r, 0.20, 0.58),
        pt(r, 0.22, 0.30),
    ];
    painter.add(Shape::convex_polygon(
        outline,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 34),
        Stroke::new(w, color),
    ));
    line(painter, r, &[(0.50, 0.88), (0.52, 0.22)], color, w * 0.8);
    line(
        painter,
        r,
        &[(0.52, 0.46), (0.68, 0.36)],
        color,
        w * 0.7,
    );
    line(
        painter,
        r,
        &[(0.52, 0.62), (0.34, 0.50)],
        color,
        w * 0.7,
    );
}

pub fn bell(painter: &Painter, r: Rect, color: Color32, w: f32) {
    line(
        painter,
        r,
        &[
            (0.24, 0.66),
            (0.30, 0.56),
            (0.30, 0.38),
            (0.38, 0.24),
            (0.5, 0.18),
            (0.62, 0.24),
            (0.70, 0.38),
            (0.70, 0.56),
            (0.76, 0.66),
            (0.24, 0.66),
        ],
        color,
        w,
    );
    painter.circle_filled(pt(r, 0.5, 0.78), r.width() * 0.05, color);
}

pub fn lock(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.rect_stroke(
        Rect::from_min_max(pt(r, 0.25, 0.44), pt(r, 0.75, 0.82)),
        egui::Rounding::same(2.5),
        Stroke::new(w, color),
    );
    line(
        painter,
        r,
        &[
            (0.36, 0.44),
            (0.36, 0.32),
            (0.42, 0.22),
            (0.58, 0.22),
            (0.64, 0.32),
            (0.64, 0.44),
        ],
        color,
        w,
    );
}

pub fn arrow_in(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.line_segment([pt(r, 0.22, 0.78), pt(r, 0.76, 0.24)], Stroke::new(w, color));
    line(
        painter,
        r,
        &[(0.40, 0.22), (0.78, 0.22), (0.78, 0.60)],
        color,
        w,
    );
}

pub fn arrow_out(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.line_segment([pt(r, 0.22, 0.22), pt(r, 0.76, 0.76)], Stroke::new(w, color));
    line(
        painter,
        r,
        &[(0.42, 0.78), (0.78, 0.78), (0.78, 0.42)],
        color,
        w,
    );
}

pub fn globe(painter: &Painter, r: Rect, color: Color32, w: f32) {
    let stroke = Stroke::new(w, color);
    painter.circle_stroke(r.center(), r.width() * 0.37, stroke);
    painter.line_segment([pt(r, 0.5, 0.13), pt(r, 0.5, 0.87)], stroke);
    painter.line_segment([pt(r, 0.14, 0.5), pt(r, 0.86, 0.5)], stroke);
    line(
        painter,
        r,
        &[
            (0.5, 0.13),
            (0.31, 0.27),
            (0.27, 0.5),
            (0.31, 0.73),
            (0.5, 0.87),
        ],
        color,
        w,
    );
    line(
        painter,
        r,
        &[
            (0.5, 0.13),
            (0.69, 0.27),
            (0.73, 0.5),
            (0.69, 0.73),
            (0.5, 0.87),
        ],
        color,
        w,
    );
}

pub fn download(painter: &Painter, r: Rect, color: Color32, w: f32) {
    painter.line_segment([pt(r, 0.5, 0.16), pt(r, 0.5, 0.62)], Stroke::new(w, color));
    line(
        painter,
        r,
        &[(0.32, 0.46), (0.5, 0.64), (0.68, 0.46)],
        color,
        w,
    );
    line(
        painter,
        r,
        &[(0.22, 0.78), (0.22, 0.86), (0.78, 0.86), (0.78, 0.78)],
        color,
        w,
    );
}
