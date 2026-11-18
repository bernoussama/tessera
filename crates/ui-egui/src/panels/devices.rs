//! The Devices panel: the selected track's sound chain, left to right. An instrument track's
//! instrument comes first, then each insert effect, then a slot that adds one. Every change is an
//! engine command. Opening a device goes through `SoundApp::open_inserted`, so a hosted plugin's
//! own editor opens here as it does from a mixer insert.

use crate::SoundApp;
use crate::theme::Tokens;
use egui::{Color32, CornerRadius, Margin, Pos2, Response, RichText, Sense, Shape, Stroke, Ui, WidgetInfo, vec2};
use serde_json::{Value, json};
use soundcraft_dsp::{ParamInfo, PluginInfo};
use soundcraft_model::{Insert, Session, TrackId};

/// Width of a device card, and of the slot that adds one.
const CARD_W: f32 = 148.0;
/// Height of a card and of the add slot, so the chain reads as one row.
const CARD_H: f32 = 116.0;
/// Frame margin inside a card.
const PAD: i8 = 6;
/// Width of the ‹ and › move buttons.
const ARROW_W: f32 = 18.0;
/// Diameter of a knob.
const KNOB: f32 = 32.0;
/// Knobs drawn on a built-in plugin's card: its first parameters.
const KNOBS: usize = 3;

/// A device as the panel draws it, copied out of the session so the panel can then run commands.
struct Device {
    /// An insert index, or [`soundcraft_mix::INSTRUMENT_SLOT`] for the instrument.
    slot: usize,
    name: String,
    bypass: bool,
    /// The first parameters of a built-in plugin, with their values. A hosted plugin keeps its
    /// settings in its own editor, so it has none here.
    knobs: Vec<(&'static ParamInfo, f32)>,
}

impl Device {
    fn new(slot: usize, insert: &Insert) -> Self {
        let info: Option<&'static PluginInfo> = crate::mix_window::plugin_info(&insert.plugin);
        let knobs = match info {
            Some(info) if !soundcraft_mix::is_third_party(&insert.plugin) => {
                info.params.iter().take(KNOBS).map(|p| (p, insert.params.get(p.id).copied().unwrap_or(p.default))).collect()
            }
            _ => Vec::new(),
        };
        Self { slot, name: info.map_or_else(|| insert.plugin.clone(), |i| i.name.to_string()), bypass: insert.bypass, knobs }
    }
}

/// The selected track's devices, in chain order, and the first insert slot that is free.
struct Chain {
    instrument: Option<Device>,
    inserts: Vec<Device>,
    free: Option<usize>,
}

fn chain(s: &Session, track: TrackId) -> Option<Chain> {
    let t = s.track(track)?;
    Some(Chain {
        instrument: t.instrument.as_ref().map(|ins| Device::new(soundcraft_mix::INSTRUMENT_SLOT, ins)),
        inserts: t.mixer.inserts.iter().enumerate().filter_map(|(slot, ins)| Some(Device::new(slot, ins.as_ref()?))).collect(),
        free: t.mixer.inserts.iter().position(Option::is_none),
    })
}

/// Draws the Devices panel for the first selected track.
pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let picked = app.engine.session().edit.selected_tracks.first().copied();
    let Some((track, chain)) = picked.and_then(|id| Some((id, chain(app.engine.session(), id)?))) else {
        ui.centered_and_justified(|ui| ui.label(crate::localization::text("Select a track to see its devices.")));
        return;
    };
    egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
        if chain.instrument.is_none() && chain.inserts.is_empty() {
            ui.vertical_centered(|ui| {
                ui.label(crate::localization::text("No instrument or effects on this track."));
                add_slot(app, ui, track, chain.free);
            });
            return;
        }
        ui.horizontal_top(|ui| {
            if let Some(instrument) = &chain.instrument {
                card(app, ui, track, instrument, None, None);
            }
            for (k, device) in chain.inserts.iter().enumerate() {
                let left = k.checked_sub(1).and_then(|j| chain.inserts.get(j)).map(|d| d.slot);
                let right = chain.inserts.get(k + 1).map(|d| d.slot);
                card(app, ui, track, device, left, right);
            }
            add_slot(app, ui, track, chain.free);
        });
    });
}

/// One device card: a header (bypass, name, remove), its knobs and a footer (move, open).
/// `left` and `right` are the slots of the neighbouring inserts, when the card can move.
fn card(app: &mut SoundApp, ui: &mut Ui, track: TrackId, device: &Device, left: Option<usize>, right: Option<usize>) {
    let t = Tokens::current();
    let instrument = device.slot == soundcraft_mix::INSTRUMENT_SLOT;
    let inner = CARD_W - 2.0 * f32::from(PAD);
    egui::Frame::new().fill(t.strip_bg).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(PAD)).show(
        ui,
        |ui| {
            ui.set_width(inner);
            ui.set_min_height(CARD_H - 2.0 * f32::from(PAD));
            // The frame inherits the chain's left-to-right layout; a card stacks its rows.
            ui.vertical(|ui| {
                header(app, ui, track, device);
                if !device.knobs.is_empty() {
                    knobs(app, ui, track, device);
                }
                ui.add_space(4.0);
                if instrument {
                    let label = RichText::new(crate::localization::text("Open plugin window")).color(Color32::WHITE);
                    if ui.add_sized([ui.available_width(), 20.0], egui::Button::new(label).fill(t.accent)).clicked() {
                        open(app, ui, track, device.slot);
                    }
                } else {
                    footer(app, ui, track, device, left, right);
                }
            });
        },
    );
}

/// A card's top row: the bypass dot and the name (inserts only), then the remove button.
fn header(app: &mut SoundApp, ui: &mut Ui, track: TrackId, device: &Device) {
    let t = Tokens::current();
    let instrument = device.slot == soundcraft_mix::INSTRUMENT_SLOT;
    ui.horizontal(|ui| {
        if !instrument && power_dot(ui, !device.bypass).clicked() {
            run(app, "mix.insert_bypass", json!({"track": track.0, "slot": device.slot}));
        }
        let name_w = (ui.available_width() - if instrument { 0.0 } else { 22.0 }).max(0.0);
        let color = if device.bypass { t.text_dim } else { t.text };
        ui.add_sized([name_w, 18.0], egui::Label::new(RichText::new(&device.name).strong().color(color)).truncate());
        if !instrument && ui.small_button("×").on_hover_text(crate::localization::text("Remove")).clicked() {
            run(app, "mix.insert_remove", json!({"track": track.0, "slot": device.slot}));
        }
    });
}

/// The card's footer for an insert: move left, open, move right.
fn footer(app: &mut SoundApp, ui: &mut Ui, track: TrackId, device: &Device, left: Option<usize>, right: Option<usize>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let left_button = egui::Button::new("‹").min_size(vec2(ARROW_W, 20.0));
        if ui.add_enabled(left.is_some(), left_button).on_hover_text(crate::localization::text("Move left")).clicked()
            && let Some(to) = left
        {
            move_insert(app, track, device.slot, to);
        }
        let open_w = (ui.available_width() - ARROW_W - 3.0).max(0.0);
        if ui.add_sized([open_w, 20.0], egui::Button::new(crate::localization::text("Open"))).clicked() {
            open(app, ui, track, device.slot);
        }
        let right_button = egui::Button::new("›").min_size(vec2(ARROW_W, 20.0));
        if ui.add_enabled(right.is_some(), right_button).on_hover_text(crate::localization::text("Move right")).clicked()
            && let Some(to) = right
        {
            move_insert(app, track, device.slot, to);
        }
    });
}

/// The card's knobs, one per built-in parameter, each captioned with the parameter's name.
fn knobs(app: &mut SoundApp, ui: &mut Ui, track: TrackId, device: &Device) {
    let cell = ui.available_width() / device.knobs.len().max(1) as f32;
    ui.horizontal(|ui| {
        for (param, value) in &device.knobs {
            ui.vertical(|ui| {
                ui.set_width(cell);
                if let Some(next) = knob(ui, param, *value) {
                    set_param(app, track, device, param.id, next);
                }
                ui.add(egui::Label::new(RichText::new(crate::localization::text(param.name)).size(9.5)).truncate());
            });
        }
    });
}

/// A rotary control for one built-in parameter: drag up or down to change it, double-click to
/// reset it to its default. Returns the value the user set, when they changed it.
fn knob(ui: &mut Ui, param: &ParamInfo, value: f32) -> Option<f32> {
    let t = Tokens::current();
    let (rect, resp) = ui.allocate_exact_size(vec2(KNOB, KNOB), Sense::click_and_drag());
    let set = if resp.double_clicked() {
        Some(param.default)
    } else if resp.dragged() {
        // A full sweep takes 200 pixels, whatever the parameter's taper.
        Some(param.from_normalized(param.to_normalized(value) - resp.drag_delta().y / 200.0))
    } else {
        None
    };
    let shown = set.unwrap_or(value);
    let n = param.to_normalized(shown);
    let center = rect.center();
    let radius = KNOB / 2.0 - 3.0;
    // Angles run clockwise from the lower left (135 degrees) over the top to the lower right.
    let at = |n: f32, r: f32| {
        let angle = (135.0 + 270.0 * n).to_radians();
        center + vec2(angle.cos(), angle.sin()) * r
    };
    let arc = |to: f32| -> Vec<Pos2> { (0..=24).map(|i| at(to * i as f32 / 24.0, radius)).collect() };
    let painter = ui.painter();
    painter.add(Shape::line(arc(1.0), Stroke::new(2.0, t.border)));
    painter.add(Shape::line(arc(n), Stroke::new(2.0, t.accent)));
    painter.line_segment([at(n, 0.3 * radius), at(n, radius)], Stroke::new(1.5, t.text));
    let resp = resp.on_hover_text(param.format(shown));
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Slider, true, crate::localization::text(param.name).to_string()));
    set
}

/// The bypass dot of an insert: lit while the insert processes audio.
fn power_dot(ui: &mut Ui, on: bool) -> Response {
    let t = Tokens::current();
    let (rect, resp) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::click());
    let painter = ui.painter();
    if on {
        painter.circle_filled(rect.center(), 4.5, t.accent);
    } else {
        painter.circle_stroke(rect.center(), 4.0, Stroke::new(1.5, t.text_dim));
    }
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, crate::localization::text("Bypass").to_string()));
    resp.on_hover_text(crate::localization::text("Bypass"))
}

/// The slot that adds an effect. Its menu is the mixer's insert menu, so the choices match, and
/// the plugin picked opens as it does from an insert.
fn add_slot(app: &mut SoundApp, ui: &mut Ui, track: TrackId, free: Option<usize>) {
    let label = crate::localization::text("+ Add effect");
    let Some(slot) = free else {
        ui.add_enabled(false, egui::Button::new(label).min_size(vec2(CARD_W, CARD_H)))
            .on_disabled_hover_text(crate::localization::text("All insert slots are in use"));
        return;
    };
    let resp = ui.add(egui::Button::new(label).min_size(vec2(CARD_W, CARD_H)));
    egui::Popup::menu(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| crate::mix_window::plugin_menu(app, ui, track, slot, false));
}

/// Sets one built-in parameter of `device` through the engine. Drags of one knob merge into one
/// undo step, the same way the plugin windows' drags do.
fn set_param(app: &mut SoundApp, track: TrackId, device: &Device, param: &str, value: f32) {
    let (command, params, key) = if device.slot == soundcraft_mix::INSTRUMENT_SLOT {
        ("mix.instrument_param", json!({"track": track.0, "param": param, "value": value}), format!("instrument_param:{}:{param}", track.0))
    } else {
        let slot = device.slot;
        ("mix.insert_param", json!({"track": track.0, "slot": slot, "param": param, "value": value}), format!("param:{}:{slot}:{param}", track.0))
    };
    if let Err(e) = app.engine.execute_merged(command, &params, &key) {
        app.ui.status = e.to_string();
    }
}

/// Runs a device command. A failure shows in the status line.
fn run(app: &mut SoundApp, command: &str, params: Value) {
    if let Err(e) = app.run(command, params) {
        app.ui.status = e;
    }
}

fn move_insert(app: &mut SoundApp, track: TrackId, from: usize, to: usize) {
    run(app, "mix.insert_move", json!({"track": track.0, "from": from, "to": to}));
}

/// Opens a device's window next frame, through `SoundApp::open_inserted`.
fn open(app: &mut SoundApp, ui: &Ui, track: TrackId, slot: usize) {
    app.pending_open = Some((track, slot));
    ui.ctx().request_repaint();
}
