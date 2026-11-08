//! Formato de fechas y duraciones.

use super::*;
// ---------- formato ----------

pub(super) fn fmt_time(ts: u32) -> String {
    let Ok(dt) = glib::DateTime::from_unix_local(ts as i64) else { return String::new() };
    let today = glib::DateTime::now_local().ok().map(|n| n.ymd());
    let fmt = if Some(dt.ymd()) == today { "%H:%M" } else { "%d/%m %H:%M" };
    dt.format(fmt).map(|s| s.to_string()).unwrap_or_default()
}

pub(super) fn fmt_datetime(ts: u32) -> String {
    glib::DateTime::from_unix_local(ts as i64)
        .and_then(|d| d.format("%d/%m/%Y %H:%M"))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub(super) fn ago(ts: u32) -> String {
    if ts == 0 {
        return "nunca oído".into();
    }
    let d = now().saturating_sub(ts);
    match d {
        0..=59 => "ahora".into(),
        60..=3599 => format!("hace {} min", d / 60),
        3600..=86399 => format!("hace {} h", d / 3600),
        _ => format!("hace {} días", d / 86400),
    }
}

pub(super) fn fmt_duration(s: u32) -> String {
    let (d, h, m) = (s / 86400, (s % 86400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{m} min")
    }
}

pub(super) fn hops_text(h: u32) -> String {
    match h {
        0 => "directo".into(),
        1 => "1 salto".into(),
        _ => format!("{h} saltos"),
    }
}

pub(super) fn battery_text(b: u32) -> String {
    if b > 100 {
        "enchufado".into()
    } else {
        format!("batería {b}%")
    }
}
