//! Gráfica de líneas sencilla con Cairo, para la telemetría de los nodos.

use super::*;

pub(super) struct Series {
    pub label: &'static str,
    pub color: (f64, f64, f64),
    pub points: Vec<(u32, f64)>,
}

/// Una tarjeta con título, leyenda y gráfica. `None` si no hay datos.
pub(super) fn chart(title: &str, unit: &'static str, series: Vec<Series>) -> Option<gtk::Box> {
    let series: Vec<Series> = series.into_iter().filter(|s| !s.points.is_empty()).collect();
    if series.is_empty() {
        return None;
    }
    let bx = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
    bx.add_css_class("card");
    let head = gtk::Box::builder().spacing(12).margin_top(8).margin_start(12).margin_end(12).build();
    let t = gtk::Label::builder().label(title).xalign(0.0).hexpand(true).build();
    t.add_css_class("heading");
    head.append(&t);
    for s in &series {
        let last = s.points.last().map(|p| p.1).unwrap_or(0.0);
        let l = gtk::Label::builder()
            .use_markup(true)
            .label(format!(
                "<span foreground='#{:02x}{:02x}{:02x}'>●</span> {} {:.1}{unit}",
                (s.color.0 * 255.0) as u8,
                (s.color.1 * 255.0) as u8,
                (s.color.2 * 255.0) as u8,
                s.label,
                last
            ))
            .build();
        l.add_css_class("meta");
        head.append(&l);
    }
    bx.append(&head);

    let area = gtk::DrawingArea::builder().content_height(150).hexpand(true).margin_bottom(8).margin_start(8).margin_end(8).build();
    let data: Vec<(Vec<(u32, f64)>, (f64, f64, f64))> = series.into_iter().map(|s| (s.points, s.color)).collect();
    area.set_draw_func(move |w, cr, width, height| {
        let fg = w.color();
        let (wf, hf) = (width as f64, height as f64);
        let (l, r, t, b) = (46.0, 8.0, 8.0, 20.0);
        let (pw, ph) = (wf - l - r, hf - t - b);
        let all = data.iter().flat_map(|(p, _)| p.iter());
        let (mut t0, mut t1, mut y0, mut y1) = (u32::MAX, 0u32, f64::MAX, f64::MIN);
        for (tt, v) in all {
            t0 = t0.min(*tt);
            t1 = t1.max(*tt);
            y0 = y0.min(*v);
            y1 = y1.max(*v);
        }
        if (y1 - y0).abs() < 1e-6 {
            y0 -= 1.0;
            y1 += 1.0;
        }
        let pad = (y1 - y0) * 0.1;
        y0 -= pad;
        y1 += pad;
        let span = (t1.saturating_sub(t0)).max(1) as f64;
        let x = |tt: u32| l + (tt.saturating_sub(t0)) as f64 / span * pw;
        let y = |v: f64| t + (1.0 - (v - y0) / (y1 - y0)) * ph;

        // Rejilla y etiquetas.
        cr.set_line_width(1.0);
        cr.set_font_size(10.0);
        for i in 0..=3 {
            let v = y0 + (y1 - y0) * i as f64 / 3.0;
            let yy = y(v);
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.12);
            cr.move_to(l, yy);
            cr.line_to(wf - r, yy);
            let _ = cr.stroke();
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.6);
            cr.move_to(2.0, yy + 3.0);
            let _ = cr.show_text(&format!("{v:.1}"));
        }
        for (tt, align) in [(t0, 0.0), (t1, 1.0)] {
            let s = fmt_time(tt);
            let ext = cr.text_extents(&s).map(|e| e.width()).unwrap_or(0.0);
            cr.move_to(x(tt) - ext * align, hf - 4.0);
            let _ = cr.show_text(&s);
        }

        // Líneas.
        cr.set_line_width(2.0);
        for (pts, c) in &data {
            cr.set_source_rgb(c.0, c.1, c.2);
            for (i, (tt, v)) in pts.iter().enumerate() {
                if i == 0 {
                    cr.move_to(x(*tt), y(*v));
                } else {
                    cr.line_to(x(*tt), y(*v));
                }
            }
            let _ = cr.stroke();
            if pts.len() == 1 {
                let (tt, v) = pts[0];
                cr.arc(x(tt), y(v), 3.0, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            }
        }
    });
    bx.append(&area);
    Some(bx)
}
