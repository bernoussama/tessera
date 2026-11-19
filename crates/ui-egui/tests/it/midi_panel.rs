use egui::{Event, Id, PointerButton, Pos2, RawInput, Rect, pos2, vec2};
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp, midi_editor};

fn frame(ctx: &egui::Context, app: &mut SoundApp, events: Vec<Event>) {
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))), events, ..Default::default() };
    ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .textures_delta
    .clear();
}

/// The MIDI editor's last laid-out body rectangle, stored by the dock.
fn panel(ctx: &egui::Context) -> Rect {
    ctx.data(|data| data.get_temp::<Rect>(Id::unique("soundcraft-midi-body"))).unwrap()
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
}

fn check_resize(with_clip: bool) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    app.ui.show_devices = false;
    if with_clip {
        app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    }
    assert_eq!(midi_editor::target_clip(&app).is_some(), with_clip);
    let ctx = egui::Context::default();
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    let initial = panel(&ctx);
    assert!((initial.height() - 260.0).abs() < 1.0, "initial panel: {initial:?}");
    let notes = app.engine.session().clone();

    // Drag the shared boundary up and down, then allow several idle frames
    // to catch a panel that looks resized but snaps back after release.
    for height in [440.0, 280.0, 200.0] {
        let before = panel(&ctx);
        let start = pos2(before.center().x, before.top() - 26.0 - 2.0);
        let end = pos2(start.x, before.bottom() - height - 26.0 - 2.0);
        frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        frame(&ctx, &mut app, vec![button(start, true)]);
        for step in 1..=6 {
            frame(&ctx, &mut app, vec![Event::PointerMoved(start.lerp(end, step as f32 / 6.0))]);
        }
        frame(&ctx, &mut app, vec![button(end, false)]);
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![]);
        }
        let after = panel(&ctx);
        assert!((after.height() - height.max(260.0)).abs() < 1.0, "requested {height}, panel: {after:?}");
        assert!((after.bottom() - initial.bottom()).abs() < 1.0);
        assert!(app.edit_layout.timeline[3] <= after.top());
    }

    // Closing and reopening the dock keeps the chosen height.
    let resized = panel(&ctx);
    app.run("window.midi_editor", json!({})).unwrap();
    frame(&ctx, &mut app, vec![]);
    app.run("window.midi_editor", json!({})).unwrap();
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(panel(&ctx), resized);
    let saved = serde_json::to_string(&app.ui).unwrap();
    app.ui = serde_json::from_str(&saved).unwrap();
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(panel(&ctx), resized);
    assert_eq!(*app.engine.session(), notes);
}

#[test]
fn midi_panel_resizes_with_a_selected_clip() {
    check_resize(true);
}

#[test]
fn midi_panel_resizes_without_a_selected_clip() {
    check_resize(false);
}

#[test]
fn layout_reset_cancels_a_held_midi_divider_without_editing_the_session() {
    check_held_replacement("reset");
}

#[test]
fn arrange_cancels_a_held_midi_divider_without_editing_the_session() {
    check_held_replacement("arrange");
}

#[test]
fn legacy_visibility_sync_cancels_a_held_midi_divider_without_editing_the_session() {
    check_held_replacement("visibility");
}

fn check_held_replacement(operation: &str) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    app.ui.show_devices = false;
    app.run("edit.select", json!({"tracks":["Keys"]})).unwrap();
    app.ui.show_memory_locations = operation == "arrange";
    let session = app.engine.session().clone();
    let ctx = egui::Context::default();
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    let initial = panel(&ctx);
    let start = pos2(initial.center().x, initial.top() - 28.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    frame(&ctx, &mut app, vec![button(start, true)]);
    let first = start - vec2(0.0, 80.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(first)]);
    frame(&ctx, &mut app, vec![]);
    assert!((panel(&ctx).height() - 340.0).abs() < 1.0, "first drag must resize");
    let second = start - vec2(0.0, 120.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(second)]);
    frame(&ctx, &mut app, vec![]);
    assert!((panel(&ctx).height() - 380.0).abs() < 1.0, "a normal second held move must continue resizing");
    match operation {
        "reset" => {
            app.run("ui.dock.reset", json!({})).unwrap();
        }
        "arrange" => {
            app.run("window.arrange_cascade", json!({})).unwrap();
        }
        "visibility" => {
            app.run("window.midi_editor", json!({})).unwrap();
            frame(&ctx, &mut app, vec![]);
            app.run("window.midi_editor", json!({})).unwrap();
            frame(&ctx, &mut app, vec![]);
        }
        _ => panic!("unknown synthetic operation"),
    }
    let reset = serde_json::to_value(&app.ui.docking).unwrap();
    let after_reset = start - vec2(0.0, 160.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(after_reset)]);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(serde_json::to_value(&app.ui.docking).unwrap(), reset, "old held divider must not resize the replacement layout");
    let expected = if operation == "reset" { 260.0 } else { 380.0 };
    assert!((panel(&ctx).height() - expected).abs() < 1.0);
    frame(&ctx, &mut app, vec![button(after_reset, false)]);
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(serde_json::to_value(&app.ui.docking).unwrap(), reset);
    assert_eq!(*app.engine.session(), session);
}

#[test]
fn a_selected_video_clip_is_not_a_midi_target() {
    use soundcraft_model::{ChannelFormat, Clip, ClipContent, SourceId, Track, TrackKind};
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    let midi = midi_editor::target_clip(&app).unwrap();
    let s = app.engine.session_mut();
    let track = s.add_track(TrackKind::Video, ChannelFormat::Mono, None);
    let video = s.new_clip_id();
    let source = SourceId(s.alloc());
    s.track_mut(track).and_then(Track::playlist_mut).unwrap().clips.push(Clip::video(video, "picture", source, 0, 0, 48_000));

    app.run("edit.select", json!({"clips": [video.0]})).unwrap();
    assert_eq!(midi_editor::target_clip(&app), None, "a video clip alone leaves the MIDI editors without a target");
    app.run("edit.select", json!({"clips": [video.0, midi.0]})).unwrap();
    assert_eq!(midi_editor::target_clip(&app), Some(midi), "the MIDI clip after a video clip is the target");
    let (_, clip) = app.engine.session().find_clip(midi).unwrap();
    assert!(matches!(clip.content, ClipContent::Midi { .. }));
}

/// A frame at UI time `time` (seconds), so clicks far enough apart are not a double-click.
fn frame_at(ctx: &egui::Context, app: &mut SoundApp, time: f64, events: Vec<Event>) {
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))), time: Some(time), events, ..Default::default() };
    ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .textures_delta
    .clear();
}

/// The demo's Keys track with an empty MIDI clip open in the editor, and a point in the roll.
fn open_empty_clip() -> (SoundApp, egui::Context, u64, Pos2) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    app.ui.show_devices = false;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    let clip = created["clip"].as_u64().unwrap();
    app.run("edit.select", json!({"clips": [clip]})).unwrap();
    let ctx = egui::Context::default();
    for i in 0..6 {
        frame_at(&ctx, &mut app, f64::from(i), vec![]);
    }
    let body = panel(&ctx);
    (app, ctx, clip, pos2(body.left() + 200.0, body.top() + 60.0))
}

fn note_count(app: &SoundApp, clip: u64) -> usize {
    let (_, edited) = app.engine.session().find_clip(soundcraft_model::ClipId(clip)).unwrap();
    let soundcraft_model::ClipContent::Midi { sequence } = &edited.content else { panic!("MIDI clip required") };
    sequence.notes.len()
}

#[test]
fn only_pencil_inserts_notes_on_a_single_click() {
    use soundcraft_model::Tool;

    let (mut app, ctx, clip, point) = open_empty_clip();
    let mut t = 10.0;
    for tool in Tool::ALL.into_iter().filter(|tool| *tool != Tool::Pencil).chain([Tool::Pencil]) {
        app.run("edit.tool", json!({"tool": tool.id()})).unwrap();
        let before = app.engine.session().clone();
        t += 5.0;
        frame_at(&ctx, &mut app, t, vec![Event::PointerMoved(point)]);
        frame_at(&ctx, &mut app, t + 0.01, vec![button(point, true)]);
        frame_at(&ctx, &mut app, t + 0.02, vec![button(point, false)]);
        if tool == Tool::Pencil {
            assert_eq!(note_count(&app, clip), 1);
            app.run("edit.undo", json!({})).unwrap();
        }
        assert!(*app.engine.session() == before, "{tool:?} click must not change the session");
    }
}

#[test]
fn double_click_adds_then_removes_a_note_without_the_pencil() {
    let (mut app, ctx, clip, point) = open_empty_clip();
    app.run("edit.tool", json!({"tool": soundcraft_model::Tool::Smart.id()})).unwrap();
    let double_click = |app: &mut SoundApp, t: f64| {
        frame_at(&ctx, app, t, vec![Event::PointerMoved(point)]);
        for (i, pressed) in [true, false, true, false].into_iter().enumerate() {
            frame_at(&ctx, app, t + 0.01 * (i as f64 + 1.0), vec![button(point, pressed)]);
        }
    };
    double_click(&mut app, 20.0);
    assert_eq!(note_count(&app, clip), 1, "double-click on empty space adds a note");
    double_click(&mut app, 30.0);
    assert_eq!(note_count(&app, clip), 0, "double-click on a note removes it");
}
