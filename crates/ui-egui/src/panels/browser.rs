//! The Browser panel: built-in and scanned plug-ins to double-click or drag into the arrangement.
//! The lists are the mixer's instrument and insert menus' registries, read from their caches.
//! Drops land on the Edit window's empty area, track headers and lanes (`edit_window`).

use super::panel_header;
use crate::docking::Panel;
use crate::localization::text;
use crate::theme::{Tokens, bold, regular};
use crate::{MainWindow, SoundApp};
use egui::{Align2, CursorIcon, Response, RichText, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};
use serde_json::{Value, json};
use soundcraft_model::{Track, TrackKind};

/// Height of a list row.
const ROW_H: f32 = 17.0;
/// Room under the list for the hint, which wraps to at most three lines in the panel's width.
const HINT_H: f32 = 44.0;

/// A plug-in the browser lists. It is also the payload of a drag, so a drop target can act on it.
#[derive(Clone)]
pub(crate) struct BrowserItem {
    /// The engine id: a built-in id, or `clap:…`, `vst3:…` or `au:…`.
    pub(crate) id: String,
    pub(crate) name: String,
    /// Where the plug-in comes from: "built-in", "CLAP", "VST3" or "AU".
    source: &'static str,
    pub(crate) instrument: bool,
}

/// Every plug-in the browser lists, sorted by name. `scan` returns the host's cache after its first
/// scan, so the browser shows what the mixer menus show and never rescans.
fn catalog() -> Vec<BrowserItem> {
    let built_in = soundcraft_dsp::plugins().iter().map(|p| BrowserItem {
        id: p.id.to_owned(),
        name: p.name.to_owned(),
        source: "built-in",
        instrument: p.is_instrument,
    });
    let clap = soundcraft_clap_host::scan().into_iter().map(|d| BrowserItem { id: d.id, name: d.name, source: "CLAP", instrument: d.is_instrument });
    let vst3 = soundcraft_vst3_host::scan().into_iter().map(|d| BrowserItem { id: d.id, name: d.name, source: "VST3", instrument: d.is_instrument });
    let au = soundcraft_au_host::scan().into_iter().map(|d| BrowserItem { id: d.id, name: d.name, source: "AU", instrument: d.is_instrument });
    let mut items: Vec<BrowserItem> = built_in.chain(clap).chain(vst3).chain(au).collect();
    items.sort_by_cached_key(|item| item.name.to_lowercase());
    dedup_formats(&mut items);
    items
}

/// Drops a plug-in listed in several formats, keeping the first of the name-sorted `items`. The sort
/// is stable and `catalog` chains CLAP before VST3 before AU, so the CLAP build wins.
fn dedup_formats(items: &mut Vec<BrowserItem>) {
    items.dedup_by(|later, earlier| later.source != "built-in" && earlier.source != "built-in" && later.name.eq_ignore_ascii_case(&earlier.name));
}

/// Whether a plug-in's name holds the search text (an empty search holds everything).
fn listed(item: &BrowserItem, needle: &str) -> bool {
    needle.is_empty() || item.name.to_lowercase().contains(needle)
}

/// The Browser panel: a search field, then the Instruments, Effects, Plug-ins and Samples categories,
/// and a hint on dragging an instrument into the arrangement.
pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    panel_header(ui, "BROWSER");
    let key = egui::Id::unique("browser_query");
    let mut query: String = ui.ctx().memory(|m| m.data.get_temp::<String>(key)).unwrap_or_default();
    ui.add(egui::TextEdit::singleline(&mut query).hint_text(text("Search…")).desired_width(f32::INFINITY));
    ui.ctx().memory_mut(|m| m.data.insert_temp(key, query.clone()));
    let needle = query.trim().to_lowercase();
    let items = catalog();
    let instruments: Vec<&BrowserItem> = items.iter().filter(|item| item.instrument && listed(item, &needle)).collect();
    let effects: Vec<&BrowserItem> = items.iter().filter(|item| !item.instrument && item.source == "built-in" && listed(item, &needle)).collect();
    let plug_ins: Vec<&BrowserItem> = items.iter().filter(|item| !item.instrument && item.source != "built-in" && listed(item, &needle)).collect();
    // While searching, every category opens so that its matches show.
    let open = (!needle.is_empty()).then_some(true);
    let list_h = (ui.available_height() - HINT_H).max(0.0);
    egui::ScrollArea::vertical().id_salt("browser_list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
        if !needle.is_empty() && instruments.is_empty() && effects.is_empty() && plug_ins.is_empty() {
            ui.weak(text("No matches"));
        }
        category(app, ui, text("Instruments"), &instruments, open, true);
        category(app, ui, text("Effects"), &effects, open, false);
        category(app, ui, text("Plug-ins"), &plug_ins, open, true);
        egui::CollapsingHeader::new(RichText::new(text("Samples")).font(bold(11.0)).color(t.header_text))
            .id_salt("samples")
            .default_open(false)
            .show(ui, |ui| {
                ui.weak(text("Sample browsing is not available yet."));
            });
    });
    ui.add(
        egui::Label::new(RichText::new(text("Drag an instrument into the empty area to make a track.")).font(regular(10.0)).color(t.text_dim)).wrap(),
    );
}

/// A collapsible category: its header, and its rows while open. `tags` shows each row's source.
fn category(app: &mut SoundApp, ui: &mut Ui, title: &str, items: &[&BrowserItem], open: Option<bool>, tags: bool) {
    let header = RichText::new(title).font(bold(11.0)).color(Tokens::current().header_text);
    egui::CollapsingHeader::new(header).id_salt(title).default_open(true).open(open).show(ui, |ui| {
        for item in items {
            row(app, ui, item, tags);
        }
    });
}

/// One row. Double-click acts on the plug-in; a drag carries it to a drop target in the arrangement.
fn row(app: &mut SoundApp, ui: &mut Ui, item: &BrowserItem, tags: bool) {
    let t = Tokens::current();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, &item.name));
    if resp.hovered() {
        ui.painter().rect_filled(rect, 2.0, t.button_hi.gamma_multiply(0.3));
    }
    let mut name_area = rect;
    if tags {
        let tag = ui.painter().layout_no_wrap(text(item.source).to_owned(), regular(9.5), t.text_dim);
        let at = pos2(rect.max.x - tag.size().x - 4.0, rect.center().y - tag.size().y / 2.0);
        name_area.max.x = at.x - 4.0;
        ui.painter().galley(at, tag, t.text_dim);
    }
    ui.painter().with_clip_rect(name_area).text(pos2(rect.min.x + 6.0, rect.center().y), Align2::LEFT_CENTER, &item.name, regular(11.0), t.text);
    if resp.drag_started() {
        resp.dnd_set_drag_payload(item.clone());
    }
    if resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }
    if resp.double_clicked()
        && let Err(error) = activate(app, item)
    {
        app.ui.status = error;
    }
}

/// Double-click: an instrument makes a new track, an effect is inserted on the selected track.
fn activate(app: &mut SoundApp, item: &BrowserItem) -> Result<(), String> {
    if item.instrument { add_instrument_track(app, item) } else { insert_on_selected(app, &item.id) }
}

/// A new instrument track named for the plug-in, with the plug-in as its instrument. `track.new`
/// selects the track.
fn add_instrument_track(app: &mut SoundApp, item: &BrowserItem) -> Result<(), String> {
    let created = app.run("track.new", json!({"kind": "instrument", "name": item.name}))?;
    let track = created.get("tracks").and_then(|tracks| tracks.get(0)).cloned().ok_or("track.new made no track")?;
    app.run("mix.instrument", json!({"track": track, "plugin": item.id})).map(drop)
}

/// Inserts the effect on the first selected track.
fn insert_on_selected(app: &mut SoundApp, plugin: &str) -> Result<(), String> {
    let track =
        app.engine.session().edit.selected_tracks.first().copied().ok_or_else(|| text("Select a track to insert the effect on.").to_owned())?;
    app.run("mix.insert", json!({"track": track.0, "plugin": plugin})).map(drop)
}

/// A drop on the Edit window's empty area below the tracks: an instrument makes a track with it.
pub(crate) fn drop_on_empty_area(app: &mut SoundApp, ui: &Ui, resp: &Response) {
    if resp.dnd_hover_payload::<BrowserItem>().is_some_and(|item| item.instrument) {
        ui.painter().rect_stroke(resp.rect, 0.0, Stroke::new(2.0, Tokens::current().accent), StrokeKind::Inside);
    }
    let Some(item) = resp.dnd_release_payload::<BrowserItem>().filter(|item| item.instrument) else { return };
    if let Err(error) = add_instrument_track(app, &item) {
        app.ui.status = error;
    }
}

/// A drop on a track's header or lane: an effect is inserted on the track, and an instrument
/// replaces the instrument of an instrument track. Other tracks refuse an instrument.
pub(crate) fn drop_on_track(app: &mut SoundApp, ui: &Ui, resp: &Response, track: &Track) {
    if resp.dnd_hover_payload::<BrowserItem>().is_some_and(|item| accepts(&item, track)) {
        ui.painter().rect_stroke(resp.rect, 0.0, Stroke::new(2.0, Tokens::current().accent), StrokeKind::Inside);
    }
    let Some(item) = resp.dnd_release_payload::<BrowserItem>().filter(|item| accepts(item, track)) else { return };
    let command = if item.instrument { "mix.instrument" } else { "mix.insert" };
    if let Err(error) = app.run(command, json!({"track": track.id.0, "plugin": item.id})) {
        app.ui.status = error;
    }
}

/// Whether a track takes the dropped plug-in.
fn accepts(item: &BrowserItem, track: &Track) -> bool {
    !item.instrument || track.kind == TrackKind::Instrument
}

/// Whether the front window's dock shows the panel.
pub(crate) fn is_open(app: &SoundApp) -> bool {
    let workspace = if app.ui.window == MainWindow::Edit { &app.ui.docking } else { &app.ui.mix_docking };
    workspace.layout.contains(&Panel::Browser)
}

/// Window > Browser: shows the panel in the front window's dock, or hides it there.
pub(crate) fn toggle(app: &mut SoundApp) -> Result<Value, String> {
    let operation = if is_open(app) { "close" } else { "open" };
    crate::docking::command(app, "ui.dock", &json!({"operation": operation, "panel": "browser"}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, source: &'static str) -> BrowserItem {
        BrowserItem { id: format!("{source}:{name}"), name: name.to_owned(), source, instrument: true }
    }

    #[test]
    fn plugin_in_two_formats_is_listed_once_as_clap() {
        let mut items = vec![item("Surge XT", "CLAP"), item("Surge XT", "VST3"), item("Vital", "VST3")];
        dedup_formats(&mut items);
        let kept: Vec<_> = items.iter().map(|i| (i.name.as_str(), i.source)).collect();
        assert_eq!(kept, [("Surge XT", "CLAP"), ("Vital", "VST3")]);
    }
}
