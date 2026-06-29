//! Graphical fan-curve editor: a DrawingArea with draggable (temp, duty) points.
//! Drag a point to move it; double-click empty space to add a point, double-click a
//! point to remove it. Save writes the curve back to the profile via the daemon.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::cairo;

const T_MAX: f64 = 100.0; // temperature axis °C
const D_MAX: f64 = 100.0; // duty axis %
const PAD: f64 = 64.0; // plot padding (room for the °C / % tick labels + axis titles)
const HIT: f64 = 14.0; // px radius to grab/remove a point

type Curve = Rc<RefCell<Vec<(i32, i32)>>>;

/// Minimum fan duty % allowed at a temperature (mirrors the daemon's safety net). The editor
/// won't let a point go below this, so you can't create a curve that's unsafe while hot.
fn safety_floor(temp: i32) -> i32 {
    match temp {
        t if t >= 90 => 80,
        t if t >= 85 => 60,
        t if t >= 80 => 45,
        t if t >= 75 => 30,
        _ => 0,
    }
}

fn sorted_clamped(pts: &mut [(i32, i32)]) {
    pts.sort_by_key(|p| p.0);
    for p in pts.iter_mut() {
        p.0 = p.0.clamp(0, T_MAX as i32);
        // clamp into range, then never below the safety floor for this temperature
        p.1 = p.1.clamp(0, D_MAX as i32).max(safety_floor(p.0));
    }
}

/// Open the editor for `name` (perf preserved). `save` is called with the new curve.
pub fn open(
    app: &adw::Application,
    name: &str,
    perf: String,
    curve: Vec<(i32, i32)>,
    save: impl Fn(&str, &str, &[(i32, i32)]) + 'static,
) {
    let pts: Curve = Rc::new(RefCell::new({
        let mut c = curve;
        if c.is_empty() {
            c = vec![(25, 0), (60, 0), (80, 60), (100, 100)];
        }
        sorted_clamped(&mut c);
        c
    }));

    let win = adw::Window::builder()
        .application(app)
        .title(format!("Fan curve: {name}"))
        .default_width(560)
        .default_height(440)
        .modal(false)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    let save_btn = gtk::Button::builder()
        .label("Save")
        .css_classes(["suggested-action"])
        .build();
    let cancel_btn = gtk::Button::with_label("Cancel");
    header.pack_start(&cancel_btn);
    header.pack_end(&save_btn);
    root.append(&header);

    let area = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    root.append(&area);
    let hint = gtk::Label::builder()
        .label("Drag a point to move it · double-click to add · double-click a point to remove")
        .css_classes(["dim-label"])
        .margin_top(6)
        .margin_bottom(8)
        .build();
    root.append(&hint);
    win.set_content(Some(&root));

    // pixel <-> data mapping helpers
    let to_px = |w: f64, h: f64, t: f64, d: f64| -> (f64, f64) {
        let x = PAD + (t / T_MAX) * (w - 2.0 * PAD);
        let y = (h - PAD) - (d / D_MAX) * (h - 2.0 * PAD);
        (x, y)
    };
    let to_data = move |w: f64, h: f64, x: f64, y: f64| -> (i32, i32) {
        let t = ((x - PAD) / (w - 2.0 * PAD) * T_MAX).round() as i32;
        let d = ((h - PAD - y) / (h - 2.0 * PAD) * D_MAX).round() as i32;
        (t.clamp(0, T_MAX as i32), d.clamp(0, D_MAX as i32))
    };

    // ---- draw ----
    {
        let pts = pts.clone();
        area.set_draw_func(move |da, cr: &cairo::Context, w, h| {
            let (w, h) = (w as f64, h as f64);
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.25);
            cr.set_line_width(1.0);
            for i in 0..=10 {
                let gx = PAD + (i as f64 / 10.0) * (w - 2.0 * PAD);
                cr.move_to(gx, PAD);
                cr.line_to(gx, h - PAD);
                let gy = (h - PAD) - (i as f64 / 10.0) * (h - 2.0 * PAD);
                cr.move_to(PAD, gy);
                cr.line_to(w - PAD, gy);
            }
            let _ = cr.stroke();
            // safety floor (red dashed): points can't go below this line
            cr.set_source_rgba(0.85, 0.20, 0.20, 0.8);
            cr.set_line_width(1.5);
            cr.set_dash(&[6.0, 4.0], 0.0);
            let mut first = true;
            for t in 0..=(T_MAX as i32) {
                let (x, y) = to_px(w, h, t as f64, safety_floor(t) as f64);
                if first {
                    cr.move_to(x, y);
                    first = false;
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
            // curve
            let p = pts.borrow();
            cr.set_source_rgb(0.20, 0.55, 0.95);
            cr.set_line_width(2.5);
            for (i, &(t, d)) in p.iter().enumerate() {
                let (x, y) = to_px(w, h, t as f64, d as f64);
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();
            // points
            for &(t, d) in p.iter() {
                let (x, y) = to_px(w, h, t as f64, d as f64);
                cr.arc(x, y, 5.0, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            }
            // axis tick labels, with units: temperature (°C) on X, fan duty (%) on Y.
            // Use the widget's foreground colour so labels stay legible in light and dark.
            let col = da.color();
            cr.set_source_rgba(col.red() as f64, col.green() as f64, col.blue() as f64, 0.7);
            cr.set_font_size(11.0);
            for i in (0..=10).step_by(2) {
                let frac = i as f64 / 10.0;
                // X axis: temperature, centred under each gridline.
                let xlabel = format!("{}°C", (frac * T_MAX) as i32);
                let gx = PAD + frac * (w - 2.0 * PAD);
                if let Ok(ext) = cr.text_extents(&xlabel) {
                    cr.move_to(gx - ext.width() / 2.0, h - PAD + 18.0);
                    let _ = cr.show_text(&xlabel);
                }
                // Y axis: fan duty, right-aligned just left of the plot.
                let ylabel = format!("{}%", (frac * D_MAX) as i32);
                let gy = (h - PAD) - frac * (h - 2.0 * PAD);
                if let Ok(ext) = cr.text_extents(&ylabel) {
                    cr.move_to(PAD - 8.0 - ext.width(), gy + ext.height() / 2.0);
                    let _ = cr.show_text(&ylabel);
                }
            }
            // Axis titles (named axis + unit), slightly stronger than the tick labels.
            cr.set_source_rgba(col.red() as f64, col.green() as f64, col.blue() as f64, 0.9);
            cr.set_font_size(12.0);
            let xtitle = "Temperature (°C)";
            if let Ok(ext) = cr.text_extents(xtitle) {
                cr.move_to(w / 2.0 - ext.width() / 2.0, h - 10.0);
                let _ = cr.show_text(xtitle);
            }
            let ytitle = "Fan duty (%)";
            if let Ok(ext) = cr.text_extents(ytitle) {
                // Rotate -90° so the title runs up the left edge, centred on the plot height.
                let _ = cr.save();
                cr.move_to(18.0, h / 2.0 + ext.width() / 2.0);
                cr.rotate(-std::f64::consts::FRAC_PI_2);
                let _ = cr.show_text(ytitle);
                let _ = cr.restore();
            }
        });
    }

    // ---- drag points ----
    let drag = gtk::GestureDrag::new();
    let dragging: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
    {
        let (pts, dragging, area) = (pts.clone(), dragging.clone(), area.clone());
        drag.connect_drag_begin(move |_g, sx, sy| {
            let (w, h) = (area.width() as f64, area.height() as f64);
            let mut best: Option<(usize, f64)> = None;
            for (i, &(t, d)) in pts.borrow().iter().enumerate() {
                let (x, y) = to_px(w, h, t as f64, d as f64);
                let dist = ((x - sx).powi(2) + (y - sy).powi(2)).sqrt();
                if dist <= HIT && best.is_none_or(|(_, b)| dist < b) {
                    best = Some((i, dist));
                }
            }
            *dragging.borrow_mut() = best.map(|(i, _)| i);
        });
    }
    {
        let (pts, dragging, area, to_data) = (pts.clone(), dragging.clone(), area.clone(), to_data);
        drag.connect_drag_update(move |g, ox, oy| {
            let Some(idx) = *dragging.borrow() else {
                return;
            };
            let Some((sx, sy)) = g.start_point() else {
                return;
            };
            let (w, h) = (area.width() as f64, area.height() as f64);
            let (mut t, d) = to_data(w, h, sx + ox, sy + oy);
            // keep ordering: clamp temp between neighbours
            let mut p = pts.borrow_mut();
            let lo = if idx > 0 { p[idx - 1].0 + 1 } else { 0 };
            let hi = if idx + 1 < p.len() {
                p[idx + 1].0 - 1
            } else {
                T_MAX as i32
            };
            if lo <= hi {
                t = t.clamp(lo, hi);
            }
            // never below the safety floor for this temperature
            p[idx] = (t, d.max(safety_floor(t)));
            drop(p);
            area.queue_draw();
        });
    }
    {
        let dragging = dragging.clone();
        drag.connect_drag_end(move |_, _, _| {
            *dragging.borrow_mut() = None;
        });
    }
    area.add_controller(drag);

    // ---- double-click: add / remove ----
    let click = gtk::GestureClick::new();
    {
        let (pts, area, to_data) = (pts.clone(), area.clone(), to_data);
        click.connect_pressed(move |_g, n, x, y| {
            if n != 2 {
                return;
            } // double-click only
            let (w, h) = (area.width() as f64, area.height() as f64);
            // remove if on a point (but keep at least 2)
            let mut p = pts.borrow_mut();
            let mut hit = None;
            for (i, &(t, d)) in p.iter().enumerate() {
                let (px, py) = to_px(w, h, t as f64, d as f64);
                if ((px - x).powi(2) + (py - y).powi(2)).sqrt() <= HIT {
                    hit = Some(i);
                    break;
                }
            }
            if let Some(i) = hit {
                if p.len() > 2 {
                    p.remove(i);
                }
            } else {
                let (t, d) = to_data(w, h, x, y);
                p.push((t, d));
                sorted_clamped(&mut p);
            }
            drop(p);
            area.queue_draw();
        });
    }
    area.add_controller(click);

    // ---- buttons ----
    {
        let win = win.clone();
        cancel_btn.connect_clicked(move |_| win.close());
    }
    {
        let (win, pts, name, perf) = (win.clone(), pts.clone(), name.to_string(), perf);
        save_btn.connect_clicked(move |_| {
            let mut c = pts.borrow().clone();
            sorted_clamped(&mut c);
            save(&name, &perf, &c);
            win.close();
        });
    }

    win.present();
}
