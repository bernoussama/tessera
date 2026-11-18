//! The Browser panel: a double-click on an instrument makes a track with it, a double-click on an
//! effect inserts it on the selected track, a drag of an instrument into the empty area does the
//! same as a double-click, and the search field filters every category.

use egui::accesskit::Role;
use egui::{pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_model::TrackKind;
use soundcraft_ui_egui::{Services, SoundApp};

/// The Edit window, which opens with the Browser docked on the left, over `engine`.
fn harness(engine: soundcraft_engine::Engine) -> Harness<'static, SoundApp> {
    let app = SoundApp::new(engine, None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

/// The name a built-in plug-in is listed under.
fn name(id: &str) -> &'static str {
    soundcraft_dsp::plugin_info(id).expect("a built-in plug-in").name
}

/// Double-clicks the row labelled `label`. egui counts a click as a triple when the two clicks
/// before it land within 0.3 s of each other, whatever their place, so a click just before (the
/// search field) would turn this double-click into a triple. The wait keeps that click out.
fn double_click(h: &mut Harness<SoundApp>, label: &str) {
    h.run_steps(60);
    let at = h.get_by_label(label).rect().center();
    for pressed in [true, false, true, false] {
        h.event(egui::Event::PointerMoved(at));
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
}

/// Types `text` into the Browser's search field. The panel is docked on the left, so the field is
/// the leftmost text field (the Clips panel has one too).
fn search(h: &mut Harness<SoundApp>, text: &str) {
    let field = h.query_all_by_role(Role::TextInput).map(|node| node.rect()).min_by(|a, b| a.min.x.total_cmp(&b.min.x)).unwrap().center();
    h.hover_at(field);
    h.drag_at(field);
    h.drop_at(field);
    h.step();
    h.event(egui::Event::Text(text.into()));
    h.run_steps(3);
}

#[test]
fn double_clicking_an_instrument_makes_a_track_with_it() {
    let mut h = harness(soundcraft_engine::Engine::default());
    double_click(&mut h, name("drum_synth"));
    let session = h.state().engine.session();
    assert_eq!(session.tracks.len(), 1);
    let track = &session.tracks[0];
    assert_eq!(track.kind, TrackKind::Instrument);
    assert_eq!(track.instrument.as_ref().map(|i| i.plugin.as_str()), Some("drum_synth"));
    assert_eq!(session.edit.selected_tracks, vec![track.id]);
}

#[test]
fn double_clicking_an_effect_inserts_it_on_the_selected_track() {
    let mut engine = soundcraft_engine::Engine::default();
    engine.execute("track.new", &json!({"name": "Keys"})).unwrap();
    engine.execute("track.new", &json!({"name": "Bass"})).unwrap();
    let mut h = harness(engine);
    h.state_mut().run("edit.select", json!({"tracks": ["Bass"]})).unwrap();
    h.run_steps(2);
    // The effects follow the instruments in a long list; the search brings the EQ into view.
    search(&mut h, "eq");
    double_click(&mut h, name("eq_7band"));
    let session = h.state().engine.session();
    let plugins = |track: &soundcraft_model::Track| track.mixer.inserts.iter().flatten().map(|i| i.plugin.clone()).collect::<Vec<_>>();
    assert_eq!(plugins(&session.tracks[1]), ["eq_7band"], "the EQ is inserted on Bass");
    assert!(plugins(&session.tracks[0]).is_empty(), "Keys is untouched");
}

#[test]
fn dragging_an_instrument_into_the_empty_area_makes_a_track() {
    let mut h = harness(soundcraft_engine::Engine::default());
    let source = h.get_by_label(name("drum_synth")).rect().center();
    let [x0, y0, x1, y1] = h.state().edit_layout.timeline;
    let target = pos2((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    h.hover_at(source);
    h.drag_at(source);
    h.hover_at(target);
    h.drop_at(target);
    h.run_steps(3);
    let session = h.state().engine.session();
    assert_eq!(session.tracks.len(), 1);
    assert_eq!(session.tracks[0].instrument.as_ref().map(|i| i.plugin.as_str()), Some("drum_synth"));
}

#[test]
fn the_search_filters_every_category_by_name() {
    let mut h = harness(soundcraft_engine::Engine::default());
    let (instrument, effect) = (name("drum_synth"), name("eq_7band"));
    assert!(h.query_by_label(instrument).is_some() && h.query_by_label(effect).is_some());
    search(&mut h, "eq");
    assert!(h.query_by_label(effect).is_some(), "the matching effect stays listed");
    assert!(h.query_by_label(instrument).is_none(), "the instrument that does not match is hidden");
}
