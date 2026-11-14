use super::*;
use crate::{AnnotationStyleState, AnnotationTool, MosaicMode, RasterBudget};
use image::Rgba;
use minnow_imaging::ViewRect;

fn selection() -> RectF {
    RectF::new(0.0, 0.0, 120.0, 100.0)
}

fn draw_rectangle(engine: &mut AnnotationEngine, x: f64) {
    if engine.tool != Some(AnnotationTool::Rectangle) {
        engine.set_tool(AnnotationTool::Rectangle);
    }
    assert!(engine.start_draw((x, 20.0), Some(selection()), true));
    assert!(engine.update_interaction((x + 20.0, 45.0), Some(selection())));
    assert!(engine.finish_interaction(8.0));
}

#[test]
fn output_identity_is_reused_by_observation_and_separates_edited_engine_forks() {
    let mut engine = AnnotationEngine::default();
    let empty = engine.snapshot().output_identity();
    assert_eq!(engine.snapshot().output_identity(), empty);
    assert_eq!(engine.prepare_output_snapshot(8.0).unwrap().output_identity(), empty);
    draw_rectangle(&mut engine, 10.0);
    let shared = engine.snapshot().output_identity();
    assert_ne!(shared, empty);
    let mut fork = engine.clone();
    assert_eq!(fork.snapshot().output_identity(), shared);
    assert!(engine.set_color(0x123456ff));
    assert!(fork.set_color(0x654321ff));
    assert_eq!(
        engine.output_revision(),
        fork.output_revision(),
        "local counters can match on different edit branches"
    );
    assert_ne!(engine.snapshot().output_identity(), fork.snapshot().output_identity());

    let identity = engine.snapshot().output_identity();
    assert_eq!(engine.prepare_output_snapshot(8.0).unwrap().output_identity(), identity);
    assert_eq!(engine.prepare_output_snapshot(8.0).unwrap().output_identity(), identity);
    assert!(engine.select(None));
    assert!(engine.set_color(0xabcdef99));
    assert!(engine.set_tool(AnnotationTool::Text));
    assert_eq!(
        engine.snapshot().output_identity(),
        identity,
        "selection and drawing defaults do not change frozen output"
    );
    engine.clear();
    let cleared = engine.snapshot().output_identity();
    assert_ne!(cleared, identity);
    engine.clear();
    assert_eq!(
        engine.snapshot().output_identity(),
        cleared,
        "clearing an already empty document is an output no-op"
    );
}

#[test]
fn modifying_a_restored_item_cuts_redo_and_has_its_own_undo_step() {
    let mut engine = AnnotationEngine::default();
    draw_rectangle(&mut engine, 10.0);
    let original = engine.snapshot().items[0].clone();
    draw_rectangle(&mut engine, 60.0);
    assert!(engine.undo());
    engine.select(Some(original.id));
    assert!(engine.set_color(0x00ff00ff));
    assert!(!engine.can_redo());
    assert!(engine.undo());
    assert_eq!(&*engine.snapshot().items, &[original]);
    assert!(engine.redo());
    assert_eq!(engine.snapshot().items[0].style.stroke_color, 0x00ff00ff);
}

#[test]
fn wheel_gesture_is_one_undo_step_and_output_closes_the_group() {
    let mut engine = AnnotationEngine::default();
    draw_rectangle(&mut engine, 10.0);
    let initial = engine.selected_item().unwrap().clone();
    for _ in 0..3 {
        assert!(engine.adjust_stroke(1.0));
    }
    let adjusted = engine.selected_item().unwrap().clone();
    assert_eq!(engine.history_usage().undo_steps, 2);
    let frozen = engine.prepare_output_snapshot(8.0).unwrap();
    assert!(engine.adjust_stroke(1.0));
    assert_eq!(engine.history_usage().undo_steps, 3);
    assert_eq!(&frozen.items[0], &adjusted);
    assert!(engine.undo());
    assert_eq!(engine.selected_item(), Some(&adjusted));
    assert!(engine.undo());
    assert_eq!(engine.selected_item(), Some(&initial));
}

#[test]
fn output_preparation_commits_pointer_and_text_before_freezing() {
    let mut engine = AnnotationEngine::default();
    engine.set_tool(AnnotationTool::Rectangle);
    assert!(engine.start_draw((10.0, 20.0), Some(selection()), true));
    assert!(engine.update_interaction((40.0, 60.0), Some(selection())));
    assert!(engine.snapshot().items.is_empty());
    let pointer = engine.prepare_output_snapshot(8.0).unwrap();
    assert_eq!(pointer.items.len(), 1);
    assert!(!engine.has_active_interaction());
    assert_eq!(pointer.committed_revision, engine.revisions().committed);
    assert_eq!(pointer.output_revision, engine.output_revision());

    engine.set_tool(AnnotationTool::Text);
    assert!(engine.start_draw((15.0, 30.0), Some(selection()), true));
    assert!(engine.append_text_edit(" committed"));
    let text = engine.prepare_output_snapshot(8.0).unwrap();
    assert_eq!(engine.text_editing_id(), None);
    assert_eq!(text.committed_revision, engine.revisions().committed);
    assert_eq!(text.output_revision, engine.output_revision());
    assert!(matches!(&text.items[1].kind, AnnotationKind::Text { text, .. } if text == "Text committed"));
    assert!(engine.undo());
    assert!(matches!(&engine.snapshot().items[1].kind, AnnotationKind::Text { text, .. } if text == "Text"));
    assert_eq!(pointer.items.len(), 1);
}

fn assert_edit_state_unchanged(before: &AnnotationEngine, after: &AnnotationEngine) {
    assert_eq!(after.snapshot().items, before.snapshot().items);
    assert_eq!(after.revisions(), before.revisions());
    assert_eq!(after.style(), before.style());
    assert_eq!(after.history_usage(), before.history_usage());
    assert_eq!(after.selected_id, before.selected_id);
    assert_eq!(after.interaction, before.interaction);
    assert_eq!(after.text_editing, before.text_editing);
    assert_eq!((after.next_id, after.next_counter), (before.next_id, before.next_counter));
}

#[test]
fn invalid_pointer_inputs_and_finish_thresholds_preserve_drafts_and_history() {
    let mut drawing = AnnotationEngine::default();
    draw_rectangle(&mut drawing, 10.0);
    assert!(drawing.start_draw((20.0, 30.0), Some(selection()), true));
    assert!(drawing.update_interaction((50.0, 60.0), Some(selection())));
    let mut editing = AnnotationEngine::default();
    editing.set_tool(AnnotationTool::Text);
    assert!(editing.start_draw((20.0, 50.0), Some(selection()), true));
    assert!(editing.append_text_edit(" draft"));
    for before in [drawing, editing] {
        let mut engine = before.clone();
        let id = engine.selected_id.unwrap();
        for bad in [
            RectF::new(10.0, 10.0, -20.0, 20.0),
            RectF::new(10.0, 10.0, 20.0, -20.0),
            RectF::new(10.0, 10.0, 0.0, 20.0),
            RectF::new(f64::NAN, 10.0, 20.0, 20.0),
            RectF::new(10.0, 10.0, f64::INFINITY, 20.0),
            RectF::new(f64::MAX, 10.0, f64::MAX, 20.0),
        ] {
            assert!(!engine.start_draw((20.0, 30.0), Some(bad), true));
            assert!(!engine.start_move(id, (20.0, 30.0), Some(bad), true));
            assert!(!engine.update_interaction((20.0, 30.0), Some(bad)));
            assert_eq!(engine.hit_test((20.0, 30.0), Some(bad), true), None);
            assert_edit_state_unchanged(&before, &engine);
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for point in [(bad, 30.0), (20.0, bad)] {
                assert!(!engine.start_draw(point, Some(selection()), true));
                assert!(!engine.start_move(id, point, Some(selection()), true));
                assert!(!engine.update_interaction(point, Some(selection())));
                assert_eq!(engine.hit_test(point, Some(selection()), true), None);
                assert_edit_state_unchanged(&before, &engine);
            }
        }
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(!engine.finish_interaction(bad));
            assert_eq!(engine.prepare_output_snapshot(bad).unwrap_err(), AnnotationInputError::InvalidMinimumSize);
            assert_edit_state_unchanged(&before, &engine);
        }
    }
    // A signed logical canvas origin is valid; only negative area is rejected.
    let mut engine = AnnotationEngine::default();
    engine.set_tool(AnnotationTool::Counter);
    assert!(engine.start_draw((-30.0, -20.0), Some(RectF::new(-50.0, -50.0, 40.0, 40.0)), true));
    let mut engine = AnnotationEngine::default();
    engine.set_tool(AnnotationTool::Arrow);
    let huge = Some(RectF::new(-f64::MAX / 2.0, -f64::MAX / 2.0, f64::MAX, f64::MAX));
    assert!(engine.start_draw((-f64::MAX / 2.0, -f64::MAX / 2.0), huge, true));
    let before = engine.clone();
    assert!(!engine.update_interaction((f64::MAX / 2.0, f64::MAX / 2.0), huge));
    assert_edit_state_unchanged(&before, &engine);
}

#[test]
fn rejected_deltas_are_atomic_and_do_not_split_wheel_undo_groups() {
    let mut engine = AnnotationEngine::default();
    draw_rectangle(&mut engine, 10.0);
    let original = engine.selected_item().unwrap().clone();
    assert!(engine.adjust_stroke(1.0));
    let before = engine.clone();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(!engine.adjust_stroke(bad));
        assert!(!engine.adjust_mosaic_intensity(bad));
        assert!(!engine.adjust_selected_by_wheel((15.0, 30.0), bad, Some(selection()), true));
        assert!(!engine.translate_all_annotations(bad, 0.0));
        assert!(!engine.translate_all_annotations(0.0, bad));
        assert_edit_state_unchanged(&before, &engine);
    }
    assert!(!engine.start_draw((20.0, 30.0), Some(RectF::new(0.0, 0.0, -10.0, 20.0)), true));
    assert!(engine.adjust_stroke(1.0));
    assert_eq!(engine.history_usage().undo_steps, before.history_usage().undo_steps);
    assert!(engine.undo());
    assert_eq!(engine.selected_item(), Some(&original));

    // The first item could be translated, but the second would overflow. No
    // part of the document or its undo/redo history may be changed.
    engine.commit_item(AnnotationItem {
        id: 2,
        style: AnnotationStyleState::default(),
        kind: AnnotationKind::Counter {
            center: (f64::MAX / 2.0, 30.0),
            number: 1,
        },
    });
    let before = engine.clone();
    assert!(!engine.translate_all_annotations(f64::MAX, 0.0));
    assert_edit_state_unchanged(&before, &engine);
    assert!(engine.undo());
    assert_eq!(&*engine.snapshot().items, &[original]);
    assert!(engine.redo());
    assert_eq!(engine.snapshot().items, before.snapshot().items);
}

#[test]
fn caller_constructed_items_reject_invalid_tool_sizes_and_deltas_without_mutation() {
    let item = AnnotationItem {
        id: 1,
        style: AnnotationStyleState::default(),
        kind: AnnotationKind::Rectangle { rect: selection() },
    };
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        for field in 0..4 {
            let mut item = item.clone();
            match field {
                0 => item.style.stroke_width = invalid,
                1 => item.style.text_size = invalid,
                2 => item.style.counter_radius = invalid,
                _ => item.style.mosaic_intensity = invalid,
            }
            let before = format!("{item:?}");
            assert!(!item.is_valid());
            assert!(!item.resize_by_wheel(1.0));
            assert!(!item.move_by(2.0, 3.0));
            assert_eq!(format!("{item:?}"), before);
        }
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut next = item.clone();
        assert!(!next.resize_by_wheel(invalid));
        assert!(!next.move_by(invalid, 0.0));
        assert_eq!(next, item);
    }
}

#[test]
fn preview_translation_rejects_the_entire_projection_without_a_background() {
    let huge = Some(RectF::new(0.0, 0.0, f64::MAX, 100.0));
    let mut engine = AnnotationEngine::default();
    engine.set_tool(AnnotationTool::Counter);
    assert!(engine.start_draw((20.0, 30.0), huge, true));
    assert!(engine.start_draw((f64::MAX / 2.0, 30.0), huge, true));
    let before = engine.clone();
    let rejected = engine.ui_state(huge, None, 1.0, Some((f64::MAX, 0.0))).layer;
    assert_eq!(rejected.raster_error, Some(GeometryError::InvalidTransform));
    assert!(rejected.outlines.is_empty());
    assert!(rejected.image.is_none());
    assert!(rejected.image_bounds.is_none());
    assert_edit_state_unchanged(&before, &engine);

    let valid = engine.ui_state(huge, None, 1.0, Some((10.0, 0.0))).layer;
    assert_eq!(valid.raster_error, None);
    assert_eq!(valid.outlines.len(), 2);
    assert_eq!(valid.outlines[0].bounds.x, 12.0);
    assert!(valid.outlines.iter().all(|outline| outline.bounds.x.is_finite()));
}

#[test]
fn preview_translation_preflights_drawing_and_combined_move_text_drafts() {
    let huge = Some(RectF::new(0.0, 0.0, f64::MAX, 100.0));
    let mut drawing = AnnotationEngine::default();
    drawing.set_tool(AnnotationTool::Counter);
    assert!(drawing.start_draw((20.0, 30.0), huge, true));
    drawing.set_tool(AnnotationTool::Rectangle);
    assert!(drawing.start_draw((f64::MAX / 2.0, 20.0), huge, true));
    assert!(drawing.update_interaction((f64::MAX * 0.6, 60.0), huge));

    let mut editing = AnnotationEngine::default();
    editing.set_tool(AnnotationTool::Counter);
    assert!(editing.start_draw((20.0, 30.0), huge, true));
    editing.set_tool(AnnotationTool::Text);
    assert!(editing.start_draw((40.0, 50.0), huge, true));
    let id = editing.selected_item().unwrap().id;
    assert!(editing.start_move(id, (40.0, 50.0), huge, true));
    assert!(editing.update_interaction((f64::MAX / 2.0, 50.0), huge));
    assert!(editing.begin_text_edit_selected());
    assert!(editing.append_text_edit(" drafted"));

    for mut engine in [drawing, editing] {
        // Both drafts are valid output candidates before the rejected projection.
        assert_eq!(engine.clone().prepare_output_snapshot(8.0).unwrap().items.len(), 2);
        let before = engine.clone();
        let layer = engine.ui_state(huge, None, 1.0, Some((f64::MAX, 0.0))).layer;
        assert_eq!(layer.raster_error, Some(GeometryError::InvalidTransform));
        assert!(layer.outlines.is_empty());
        assert!(layer.image.is_none());
        assert_edit_state_unchanged(&before, &engine);
        let valid = engine.ui_state(huge, None, 1.0, Some((5.0, 0.0))).layer;
        assert_eq!(valid.raster_error, None);
        assert_eq!(valid.outlines.len(), 2);
        assert!(valid.outlines[1].transient);
        assert_eq!(valid.outlines[1].bounds.x, f64::MAX / 2.0);
    }
}

#[test]
fn history_operations_report_transient_cancellation_without_history() {
    let operations: [fn(&mut AnnotationEngine) -> bool; 2] = [AnnotationEngine::undo, AnnotationEngine::redo];
    for operation in operations {
        for tool in [AnnotationTool::Rectangle, AnnotationTool::Text] {
            let mut engine = AnnotationEngine::default();
            engine.set_history_budget(crate::HistoryBudget { max_steps: 0, max_bytes: 0 });
            engine.set_tool(tool);
            assert!(engine.start_draw((10.0, 20.0), Some(selection()), true));
            if tool == AnnotationTool::Text {
                assert!(engine.append_text_edit(" draft"));
            } else {
                assert!(engine.update_interaction((40.0, 60.0), Some(selection())));
            }
            assert!(!engine.can_undo());
            assert!(!engine.can_redo());
            let committed = engine.snapshot();
            let before = engine.revisions();
            assert!(operation(&mut engine));
            assert!(!engine.has_active_interaction());
            assert_eq!(engine.text_editing_id(), None);
            assert_eq!(engine.snapshot().items, committed.items);
            assert_eq!(engine.revisions().committed, before.committed);
            assert!(engine.revisions().transient > before.transient);
            let settled = engine.revisions();
            assert!(!operation(&mut engine));
            assert_eq!(engine.revisions(), settled);
        }
    }
}

#[test]
fn selection_reports_discarding_a_draft_even_without_a_selection_change() {
    let mut engine = AnnotationEngine::default();
    engine.set_tool(AnnotationTool::Text);
    assert!(engine.start_draw((10.0, 20.0), Some(selection()), true));
    assert!(engine.append_text_edit(" draft"));
    // Exercise the defensive repair path for a draft whose selected item was
    // already cleared; the draft's removal still needs to refresh the UI.
    engine.selected_id = None;
    let before = engine.revisions();
    assert!(engine.select(None));
    assert_eq!(engine.text_editing_id(), None);
    assert!(engine.revisions().transient > before.transient);
    let settled = engine.revisions();
    assert!(!engine.select(None));
    assert_eq!(engine.revisions(), settled);
}

#[test]
fn ending_an_uncommitted_pointer_interaction_reports_its_removal() {
    let mut engine = AnnotationEngine::default();
    engine.set_tool(AnnotationTool::Rectangle);
    assert!(engine.start_draw((10.0, 20.0), Some(selection()), true));
    let before = engine.revisions();
    assert!(engine.finish_interaction(8.0));
    assert!(!engine.has_active_interaction());
    assert!(engine.snapshot().items.is_empty());
    assert_eq!(engine.revisions().committed, before.committed);
    assert!(engine.revisions().transient > before.transient);
    assert!(!engine.finish_interaction(8.0));

    draw_rectangle(&mut engine, 10.0);
    let committed = engine.snapshot();
    let history = engine.history_usage();
    assert!(engine.start_move(committed.items[0].id, (10.0, 20.0), Some(selection()), true));
    assert!(engine.finish_interaction(8.0));
    assert!(!engine.has_active_interaction());
    assert_eq!(engine.snapshot().items, committed.items);
    assert_eq!(engine.history_usage(), history);
    assert!(!engine.finish_interaction(8.0));
}

fn background() -> Arc<RgbaImage> {
    Arc::new(RgbaImage::from_fn(200, 160, |x, y| {
        Rgba([(x * 7) as u8, (y * 11) as u8, (x ^ y) as u8, 255])
    }))
}

#[test]
fn fractional_selection_preview_and_export_share_pixels_and_bounds() {
    let source = background();
    let asset = ImageAsset::from_shared(source.clone());
    let selection = RectF::new(10.25, 7.25, 40.0, 35.0);
    let resolved = ResolvedSelection::resolve(ViewRect(selection), 1.5, source.dimensions()).unwrap();
    for kind in [
        AnnotationKind::Rectangle {
            rect: RectF::new(11.5, 8.5, 30.5, 30.5),
        },
        AnnotationKind::Mosaic {
            rect: RectF::new(5.0, 0.0, 60.0, 60.0),
            mode: MosaicMode::Pixelate,
            intensity: 8.0,
        },
        AnnotationKind::Mosaic {
            rect: RectF::new(5.0, 0.0, 60.0, 60.0),
            mode: MosaicMode::Blur,
            intensity: 8.0,
        },
    ] {
        let mut engine = AnnotationEngine::default();
        engine.commit_item(AnnotationItem {
            id: 1,
            style: AnnotationStyleState::default(),
            kind,
        });
        let preview = engine.ui_state(Some(selection), Some(&source), 1.5, None).layer;
        assert_eq!(preview.image_bounds, Some(resolved.view_rect().0));
        assert_eq!(preview.image.as_ref().unwrap().width(), 61);
        let exported = engine.prepare_output_snapshot(8.0).unwrap().compose_selection(&asset, resolved).unwrap();
        assert_eq!(preview.image.unwrap().as_ref(), exported.pixels());
    }
}

#[test]
fn cache_budget_keeps_complete_pixels_when_a_selection_does_not_fit() {
    let source = background();
    let mut engine = AnnotationEngine::default();
    draw_rectangle(&mut engine, 10.0);
    let expected = engine.ui_state(Some(selection()), Some(&source), 1.0, None).layer.image.unwrap();
    assert!(engine.raster_diagnostics().cached_bytes > 0);
    // The cache retains only weak source identities.
    assert_eq!(Arc::strong_count(&source), 1);
    engine.set_raster_budget(RasterBudget { max_cached_bytes: 1 });
    let actual = engine.ui_state(Some(selection()), Some(&source), 1.0, None).layer.image.unwrap();
    assert_eq!(expected, actual);
    let diagnostics = engine.raster_diagnostics();
    assert_eq!(diagnostics.cached_bytes, 0);
    assert_eq!(diagnostics.budget_bytes, 1);
    assert_eq!(diagnostics.uncached_renders, 1);
}

#[test]
fn output_selection_rejects_a_different_source_size() {
    let source = ImageAsset::from_shared(background());
    let resolved = ResolvedSelection::resolve(ViewRect(selection()), 1.0, source.dimensions()).unwrap();
    let other = ImageAsset::new(RgbaImage::new(201, 160));
    let snapshot = AnnotationEngine::default().snapshot();
    assert_eq!(
        snapshot.compose_selection(&other, resolved).unwrap_err(),
        AnnotationOutputError::Geometry(GeometryError::SizeMismatch)
    );
}

#[test]
fn a_rectangle_crossing_the_selection_is_clipped_without_an_artificial_border() {
    let source = background();
    let selection = RectF::new(10.25, 7.25, 40.0, 35.0);
    let resolved = ResolvedSelection::resolve(ViewRect(selection), 1.5, source.dimensions()).unwrap();
    let mut engine = AnnotationEngine::default();
    engine.commit_item(AnnotationItem {
        id: 1,
        style: AnnotationStyleState::default(),
        kind: AnnotationKind::Rectangle {
            rect: RectF::new(-10.0, -10.0, 120.0, 120.0),
        },
    });
    let preview = engine.ui_state(Some(selection), Some(&source), 1.5, None).layer.image.unwrap();
    let rect = resolved.pixel_rect();
    let expected = image::imageops::crop_imm(source.as_ref(), rect.x, rect.y, rect.width, rect.height).to_image();
    assert_eq!(preview.as_ref(), &expected);
}

// A single solid glyph keeps this clipping contract independent of installed
// system fonts and avoids shipping a font file just for this regression.
struct SolidGlyph;

impl ab_glyph::Font for SolidGlyph {
    fn units_per_em(&self) -> Option<f32> {
        Some(1.0)
    }
    fn ascent_unscaled(&self) -> f32 {
        1.0
    }
    fn descent_unscaled(&self) -> f32 {
        0.0
    }
    fn line_gap_unscaled(&self) -> f32 {
        0.0
    }
    fn glyph_id(&self, _: char) -> ab_glyph::GlyphId {
        ab_glyph::GlyphId(0)
    }
    fn h_advance_unscaled(&self, _: ab_glyph::GlyphId) -> f32 {
        1.0
    }
    fn h_side_bearing_unscaled(&self, _: ab_glyph::GlyphId) -> f32 {
        0.0
    }
    fn v_advance_unscaled(&self, _: ab_glyph::GlyphId) -> f32 {
        1.0
    }
    fn v_side_bearing_unscaled(&self, _: ab_glyph::GlyphId) -> f32 {
        0.0
    }
    fn kern_unscaled(&self, _: ab_glyph::GlyphId, _: ab_glyph::GlyphId) -> f32 {
        0.0
    }
    fn glyph_count(&self) -> usize {
        1
    }
    fn codepoint_ids(&self) -> ab_glyph::CodepointIdIter<'_> {
        unreachable!("rasterization does not enumerate codepoints")
    }
    fn glyph_raster_image2(&self, _: ab_glyph::GlyphId, _: u16) -> Option<ab_glyph::v2::GlyphImage<'_>> {
        None
    }
    fn outline(&self, _: ab_glyph::GlyphId) -> Option<ab_glyph::Outline> {
        use ab_glyph::{Outline, OutlineCurve, point};
        Some(Outline {
            bounds: ab_glyph::Rect {
                min: point(0.0, 1.0),
                max: point(1.0, 0.0),
            },
            curves: vec![
                OutlineCurve::Line(point(0.0, 0.0), point(1.0, 0.0)),
                OutlineCurve::Line(point(1.0, 0.0), point(1.0, 1.0)),
                OutlineCurve::Line(point(1.0, 1.0), point(0.0, 1.0)),
                OutlineCurve::Line(point(0.0, 1.0), point(0.0, 0.0)),
            ],
        })
    }
}

#[test]
fn text_and_counter_output_require_a_frozen_font_while_editing_and_preview_remain_available() {
    let background = background();
    let source = ImageAsset::from_shared(background.clone());
    let resolved = ResolvedSelection::resolve(ViewRect(selection()), 1.0, source.dimensions()).unwrap();
    for tool in [AnnotationTool::Text, AnnotationTool::Counter] {
        let mut engine = AnnotationEngine::default();
        engine.set_tool(tool);
        assert!(engine.start_draw((40.0, 50.0), Some(selection()), true));
        if tool == AnnotationTool::Text {
            assert!(engine.append_text_edit(" draft"));
        }
        let preview = engine.ui_state(Some(selection()), Some(&background), 1.0, None).layer;
        assert_eq!(preview.raster_error, None);
        assert!(preview.image.is_some());
        let without_font = engine.prepare_output_snapshot(8.0).unwrap();
        assert_eq!(without_font.validate_for_composition(1.0), Err(AnnotationOutputError::MissingFont));
        assert_eq!(
            without_font.compose_background(&background, 1.0).unwrap_err(),
            AnnotationOutputError::MissingFont
        );
        assert_eq!(without_font.compose_asset(&source, 1.0).unwrap_err(), AnnotationOutputError::MissingFont);
        assert_eq!(
            without_font.compose_selection(&source, resolved).unwrap_err(),
            AnnotationOutputError::MissingFont
        );

        engine.set_text_font(Some(ab_glyph::FontArc::new(SolidGlyph)));
        let with_font = engine.snapshot();
        assert_ne!(
            without_font.output_identity(),
            with_font.output_identity(),
            "a font change changes glyph output identity"
        );
        assert_eq!(with_font.validate_for_composition(1.0), Ok(()));
        let composed = with_font.compose_background(&background, 1.0).unwrap();
        assert_ne!(composed, background);
        assert_eq!(with_font.compose_asset(&source, 1.0).unwrap().pixels(), composed.as_ref());
        let cropped = with_font.compose_selection(&source, resolved).unwrap();
        let rect = resolved.pixel_rect();
        let expected = image::imageops::crop_imm(composed.as_ref(), rect.x, rect.y, rect.width, rect.height).to_image();
        assert_eq!(cropped.pixels(), &expected);
        // Later font injection cannot retroactively change a frozen output job.
        assert_eq!(without_font.compose_asset(&source, 1.0).unwrap_err(), AnnotationOutputError::MissingFont);
    }
}

#[test]
fn output_rejects_invalid_scale_even_when_the_document_would_share_source_pixels() {
    let background = background();
    let source = ImageAsset::from_shared(background.clone());
    let empty = AnnotationEngine::default();
    let mut annotated = AnnotationEngine::default();
    draw_rectangle(&mut annotated, 10.0);
    for snapshot in [empty.snapshot(), annotated.snapshot()] {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(snapshot.validate_for_composition(scale), Err(AnnotationOutputError::InvalidScale));
            assert_eq!(
                snapshot.compose_background(&background, scale).unwrap_err(),
                AnnotationOutputError::InvalidScale
            );
            assert_eq!(snapshot.compose_asset(&source, scale).unwrap_err(), AnnotationOutputError::InvalidScale);
        }
    }
}

#[test]
fn text_outside_the_selection_is_clipped_instead_of_moved_inside() {
    let source = background();
    let selection = RectF::new(10.25, 7.25, 40.0, 35.0);
    let resolved = ResolvedSelection::resolve(ViewRect(selection), 1.5, source.dimensions()).unwrap();
    let rect = resolved.pixel_rect();
    let untouched = image::imageops::crop_imm(source.as_ref(), rect.x, rect.y, rect.width, rect.height).to_image();
    for (origin, visible) in [((20.0, 30.0), true), ((-50.0, 30.0), false)] {
        let mut engine = AnnotationEngine::default();
        engine.set_text_font(Some(ab_glyph::FontArc::new(SolidGlyph)));
        engine.commit_item(AnnotationItem {
            id: 1,
            style: AnnotationStyleState::default(),
            kind: AnnotationKind::Text { origin, text: "X".into() },
        });
        let preview = engine.ui_state(Some(selection), Some(&source), 1.5, None).layer.image.unwrap();
        assert_eq!(preview.as_ref() != &untouched, visible);
        let asset = ImageAsset::from_shared(source.clone());
        let exported = engine.prepare_output_snapshot(8.0).unwrap().compose_selection(&asset, resolved).unwrap();
        assert_eq!(preview.as_ref(), exported.pixels());
    }
}

#[test]
fn output_revision_ignores_selection_defaults_and_unchanged_text_drafts() {
    let mut engine = AnnotationEngine::default();
    draw_rectangle(&mut engine, 10.0);
    let id = engine.selected_item().unwrap().id;
    let output = engine.output_revision();
    let render = engine.revisions();
    // Synchronizing the disabled fill's RGB is document state, but has no
    // effect on the rectangle's output.
    assert!(engine.set_color(engine.style().stroke_color));
    assert_eq!(engine.output_revision(), output);
    assert!(engine.undo());
    assert_eq!(engine.output_revision(), output);
    assert!(engine.redo());
    assert_eq!(engine.output_revision(), output);
    assert!(engine.select(None));
    assert!(engine.set_color(0x00ff00ff));
    assert!(engine.toggle_fill());
    assert!(engine.adjust_stroke(1.0));
    assert!(engine.set_mosaic_mode(MosaicMode::Blur));
    assert!(engine.adjust_mosaic_intensity(2.0));
    assert!(engine.select(Some(id)));
    assert!(engine.set_tool(AnnotationTool::Text));
    assert_eq!(engine.output_revision(), output);
    assert!(engine.revisions().transient > render.transient);
    assert_eq!(engine.revisions().output, output);

    assert!(engine.start_draw((20.0, 50.0), Some(selection()), true));
    let text = engine.output_revision();
    assert!(engine.insert_newline_text_edit());
    assert_eq!(engine.output_revision(), text);
    assert!(engine.commit_text_edit());
    assert_eq!(engine.output_revision(), text);
    assert!(engine.begin_text_edit_selected());
    assert_eq!(engine.output_revision(), text);
    assert!(engine.append_text_edit(" draft"));
    assert!(engine.output_revision() > text);
    let draft = engine.output_revision();
    assert!(engine.cancel_text_edit());
    assert!(engine.output_revision() > draft);
}

#[test]
fn output_revision_tracks_nonempty_pointer_drafts_and_never_reuses_a_cleared_token() {
    let mut engine = AnnotationEngine::default();
    let empty = engine.output_revision();
    for tool in [
        AnnotationTool::Rectangle,
        AnnotationTool::Circle,
        AnnotationTool::Mosaic,
        AnnotationTool::Arrow,
    ] {
        engine.set_tool(tool);
        assert!(engine.start_draw((10.0, 20.0), Some(selection()), true));
        assert_eq!(engine.output_revision(), empty);
        assert!(engine.finish_interaction(8.0));
        assert_eq!(engine.output_revision(), empty);
    }
    assert!(engine.start_draw((10.0, 20.0), Some(selection()), true));
    assert!(engine.update_interaction((80.0, 20.0), Some(selection())));
    let draft = engine.output_revision();
    assert!(draft > empty);
    assert!(!engine.update_interaction((80.0, 20.0), Some(selection())));
    assert_eq!(engine.output_revision(), draft);
    assert!(engine.cancel_interaction_state());
    assert!(engine.output_revision() > draft);

    draw_rectangle(&mut engine, 10.0);
    let id = engine.selected_item().unwrap().id;
    let committed = engine.output_revision();
    assert!(engine.start_move(id, (15.0, 30.0), Some(selection()), true));
    assert_eq!(engine.output_revision(), committed);
    assert!(engine.update_interaction((25.0, 40.0), Some(selection())));
    assert!(engine.output_revision() > committed);
    let moving = engine.output_revision();
    assert!(engine.finish_interaction(8.0));
    assert!(engine.output_revision() > moving);
    let before_clear = engine.output_revision();
    engine.clear();
    assert!(engine.output_revision() > before_clear);
    let empty = engine.output_revision();
    engine.clear();
    assert_eq!(engine.output_revision(), empty);
}

#[test]
fn axis_aligned_arrows_hit_their_pixels_and_the_same_logical_tolerance() {
    let selection = RectF::new(0.0, 0.0, 120.0, 120.0);
    for (start, end, near, far) in [
        ((20.0, 60.0), (100.0, 60.0), (40.0, 65.0), (40.0, 75.0)),
        ((60.0, 20.0), (60.0, 100.0), (65.0, 40.0), (75.0, 40.0)),
    ] {
        let mut engine = AnnotationEngine::default();
        engine.commit_item(AnnotationItem {
            id: 1,
            style: AnnotationStyleState::default(),
            kind: AnnotationKind::Arrow { start, end },
        });
        let bounds = engine.selected_item().unwrap().bounds();
        assert!(bounds.width > 0.0 && bounds.height > 0.0);
        let pixels = engine.snapshot().compose_asset(&ImageAsset::new(RgbaImage::new(120, 120)), 1.0).unwrap();
        let mut visible = 0;
        for (x, y, pixel) in pixels.pixels().enumerate_pixels() {
            if pixel.0[3] != 0 {
                visible += 1;
                assert_eq!(engine.hit_test((f64::from(x), f64::from(y)), Some(selection), true), Some(1));
            }
        }
        assert!(visible > 0);
        assert_eq!(engine.hit_test(near, Some(selection), true), Some(1));
        assert_eq!(engine.hit_test(far, Some(selection), true), None);
    }
}

#[test]
fn mosaic_style_edits_during_a_move_are_visible_before_pointer_release() {
    let source = background();
    let asset = ImageAsset::from_shared(source.clone());
    let resolved = ResolvedSelection::resolve(ViewRect(selection()), 1.0, source.dimensions()).unwrap();
    let mut engine = AnnotationEngine::default();
    engine.commit_item(AnnotationItem {
        id: 1,
        style: AnnotationStyleState::default(),
        kind: AnnotationKind::Mosaic {
            rect: RectF::new(10.0, 20.0, 30.0, 25.0),
            mode: MosaicMode::Pixelate,
            intensity: 10.0,
        },
    });
    let output = engine.output_revision();
    assert!(engine.set_color(0x00ff00ff));
    assert!(engine.toggle_fill());
    assert_eq!(engine.output_revision(), output);
    assert!(engine.start_move(1, (15.0, 30.0), Some(selection()), true));
    assert!(engine.update_interaction((65.0, 40.0), Some(selection())));
    let old = engine.ui_state(Some(selection()), Some(&source), 1.0, None).layer.image.unwrap();
    assert_eq!(engine.hit_test((15.0, 30.0), Some(selection()), true), None);
    assert!(engine.adjust_selected_by_wheel((65.0, 40.0), 4.0, Some(selection()), true));
    assert!(engine.set_mosaic_mode(MosaicMode::Blur));
    let live = engine.ui_state(Some(selection()), Some(&source), 1.0, None).layer.image.unwrap();
    assert_ne!(old, live);
    let export = engine.prepare_output_snapshot(8.0).unwrap().compose_selection(&asset, resolved).unwrap();
    assert_eq!(live.as_ref(), export.pixels());
    assert!(engine.undo());
    assert!(matches!(
        engine.kind_for(1).unwrap(),
        AnnotationKind::Mosaic { rect, mode: MosaicMode::Blur, intensity: 18.0 } if rect.x == 10.0 && rect.y == 20.0
    ));
}

fn alpha_bounds(image: &RgbaImage) -> (u32, u32, u32, u32) {
    let (mut left, mut top, mut right, mut bottom) = (image.width(), image.height(), 0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0[3] != 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    assert!(left <= right && top <= bottom);
    (left, top, right, bottom)
}

#[test]
fn logical_counter_and_text_limits_scale_once_at_fractional_and_high_dpi() {
    let canvas = RectF::new(0.0, 0.0, 300.0, 300.0);
    for scale in [0.5, 1.0, 2.0] {
        for radius in [10.0, 64.0] {
            let mut engine = AnnotationEngine::default();
            engine.set_text_font(Some(ab_glyph::FontArc::new(SolidGlyph)));
            engine.commit_item(AnnotationItem {
                id: 1,
                style: AnnotationStyleState {
                    counter_radius: radius,
                    text_size: 96.0,
                    ..AnnotationStyleState::default()
                },
                kind: AnnotationKind::Counter {
                    center: (150.0, 150.0),
                    number: u32::MAX,
                },
            });
            let image = engine
                .snapshot()
                .compose_asset(&ImageAsset::new(RgbaImage::new(600, 600)), scale)
                .unwrap();
            let (left, top, right, bottom) = alpha_bounds(image.pixels());
            let expected_radius = (radius * scale).round() as u32;
            let expected_center = (150.0 * scale).round() as u32;
            assert_eq!((left, top), (expected_center - expected_radius, expected_center - expected_radius));
            assert_eq!((right, bottom), (expected_center + expected_radius, expected_center + expected_radius));
            assert_eq!(engine.hit_test((150.0 + radius - 0.5, 150.0), Some(canvas), true), Some(1));
        }

        let mut engine = AnnotationEngine::default();
        engine.set_text_font(Some(ab_glyph::FontArc::new(SolidGlyph)));
        engine.commit_item(AnnotationItem {
            id: 1,
            style: AnnotationStyleState {
                text_size: 96.0,
                ..AnnotationStyleState::default()
            },
            kind: AnnotationKind::Text {
                origin: (10.0, 110.0),
                text: "XX\nX".into(),
            },
        });
        let image = engine
            .snapshot()
            .compose_asset(&ImageAsset::new(RgbaImage::new(600, 600)), scale)
            .unwrap();
        let (left, top, right, bottom) = alpha_bounds(image.pixels());
        assert_eq!(left, (10.0 * scale).round() as u32);
        assert_eq!(top, (14.0 * scale).round() as u32);
        assert_eq!(right - left + 1, (192.0 * scale).round() as u32);
        assert!((f64::from(bottom + 1) - (14.0 + 96.0 * 1.35 + 96.0) * scale).abs() <= 1.0);
        assert_eq!(engine.hit_test((200.0, 50.0), Some(canvas), true), Some(1));
    }
}
