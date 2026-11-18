//! The Devices panel: an instrument track's chain in the Edit window's dock. Adding an effect,
//! bypassing and removing it, and opening the instrument's window.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_mix::INSTRUMENT_SLOT;
use soundcraft_model::{Insert, TrackId};
use soundcraft_ui_egui::{MainWindow, Services, SoundApp};

/// An engine with one instrument track, `Keys`, and that track's id.
fn keys() -> (soundcraft_engine::Engine, TrackId) {
    let mut engine = soundcraft_engine::Engine::default();
    engine.execute("track.new", &json!({"kind": "instrument", "name": "Keys"})).unwrap();
    let track = engine.session().tracks[0].id;
    (engine, track)
}

/// The first built-in effect's id.
fn effect_id() -> &'static str {
    soundcraft_dsp::plugins().iter().find(|p| !p.is_instrument).map(|p| p.id).unwrap()
}

/// `Keys` with a built-in effect in its first insert slot.
fn keys_with_effect() -> (soundcraft_engine::Engine, TrackId) {
    let (mut engine, track) = keys();
    engine.execute("mix.insert", &json!({"track": track.0, "plugin": effect_id()})).unwrap();
    (engine, track)
}

/// The Edit window with the Devices tab in front and `Keys` selected.
fn harness(engine: soundcraft_engine::Engine) -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(engine, None, Services::default());
    app.ui.window = MainWindow::Edit;
    app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    app.run("ui.dock", json!({"operation": "activate", "panel": "devices"})).unwrap();
    let mut h = Harness::builder().with_size(vec2(1400.0, 1000.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn first_insert(h: &Harness<SoundApp>, track: TrackId) -> Option<Insert> {
    h.state().engine.session().track(track)?.mixer.inserts.first()?.clone()
}

/// Picking an effect from the add slot's menu puts a card in the chain, and its window opens.
#[test]
fn add_effect_puts_a_card_in_the_chain() {
    let (engine, track) = keys();
    let mut h = harness(engine);
    let plugin = soundcraft_dsp::plugins().iter().find(|p| !p.is_instrument).copied().unwrap();
    h.get_by_label("+ Add effect").click();
    h.run_steps(3);
    h.get_by_label(&format!("{:?}", plugin.category)).hover();
    h.run_steps(3);
    h.get_by_label(plugin.name).click();
    h.run_steps(3);
    assert_eq!(first_insert(&h, track).map(|i| i.plugin), Some(plugin.id.to_string()));
    assert_eq!(h.query_all_by_label("×").count(), 1, "the new effect has a card with a remove button");
    assert_eq!(h.state().ui.plugin_windows, vec![(track, 0)], "its window opens");
}

#[test]
fn the_power_dot_toggles_bypass() {
    let (engine, track) = keys_with_effect();
    let mut h = harness(engine);
    h.get_by_label("Bypass").click();
    h.run_steps(2);
    assert_eq!(first_insert(&h, track).map(|i| i.bypass), Some(true));
    h.get_by_label("Bypass").click();
    h.run_steps(2);
    assert_eq!(first_insert(&h, track).map(|i| i.bypass), Some(false));
}

#[test]
fn remove_takes_the_effect_out_of_the_chain() {
    let (engine, track) = keys_with_effect();
    let mut h = harness(engine);
    h.get_by_label("×").click();
    h.run_steps(2);
    assert!(first_insert(&h, track).is_none());
    assert_eq!(h.query_all_by_label("×").count(), 0);
}

/// The instrument card's button opens the instrument's window. With no audio engine there is no
/// editor to open, so the built-in plugin window is the one that opens.
#[test]
fn the_instrument_card_opens_its_window() {
    let (engine, track) = keys();
    let mut h = harness(engine);
    h.get_by_label("Open plugin window").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.plugin_windows, vec![(track, INSTRUMENT_SLOT)]);
}
