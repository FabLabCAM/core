use super::*;

fn frame(ctx: &egui::Context, prompt: &mut String, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions(ui, 0, false, "**Réglage**\nProfondeur −2 mm", prompt);
            });
        },
    )
}

fn click(ctx: &egui::Context, prompt: &mut String, label: &str) -> egui::FullOutput {
    let output = frame(ctx, prompt, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::epaint::Shape::Text(t) = &shape.shape {
                if t.galley.text() == label {
                    return Some(t.pos + t.galley.size() * 0.5);
                }
            }
            None
        })
        .expect("button rendered");
    let event = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        ctx,
        prompt,
        vec![egui::Event::PointerMoved(pos), event(true)],
    );
    frame(ctx, prompt, vec![event(false)])
}

#[test]
fn copy_keeps_unicode_markdown_and_draft() {
    let ctx = egui::Context::default();
    let mut prompt = "Mon brouillon".to_owned();
    let out = click(&ctx, &mut prompt, "Copier");
    assert_eq!(
        out.platform_output.copied_text,
        "**Réglage**\nProfondeur −2 mm"
    );
    assert_eq!(prompt, "Mon brouillon");
}

#[test]
fn reuse_appends_to_draft_without_clipboard_side_effect() {
    let ctx = egui::Context::default();
    let mut prompt = "Vérifie ces valeurs :".to_owned();
    let out = click(&ctx, &mut prompt, "Ajouter au prompt");
    assert_eq!(
        prompt,
        "Vérifie ces valeurs :\n\n**Réglage**\nProfondeur −2 mm"
    );
    assert!(out.platform_output.copied_text.is_empty());
}

#[test]
fn selected_source_is_read_only_and_can_be_pasted_into_prompt() {
    let ctx = egui::Context::default();
    let source_id = egui::Id::new("source");
    let prompt_id = egui::Id::new("prompt");
    let text = "Réglages :\n- Profondeur −2 mm\n```gcode\nG21\n```";
    let mut prompt = String::new();
    let draw = |events, prompt: &mut String| {
        ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut source = text;
                    ui.add(egui::TextEdit::multiline(&mut source).id(source_id));
                    ui.add(egui::TextEdit::multiline(prompt).id(prompt_id));
                    assert_eq!(source, text);
                });
            },
        )
    };
    draw(vec![], &mut prompt);
    let mut state = egui::TextEdit::load_state(&ctx, source_id).unwrap();
    state.set_ccursor_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(13),
        egui::text::CCursor::new(30),
    )));
    state.store(&ctx, source_id);
    ctx.memory_mut(|m| m.request_focus(source_id));
    let out = draw(vec![egui::Event::Copy], &mut prompt);
    assert_eq!(out.platform_output.copied_text, "Profondeur −2 mm\n");
    assert!(
        ctx.wants_keyboard_input(),
        "drawing shortcuts must defer to text focus"
    );
    draw(
        vec![egui::Event::Paste("replacement".into()), egui::Event::Cut],
        &mut prompt,
    );
    ctx.memory_mut(|m| m.request_focus(prompt_id));
    draw(
        vec![egui::Event::Paste(out.platform_output.copied_text)],
        &mut prompt,
    );
    assert_eq!(prompt, "Profondeur −2 mm\n");
    // An external clipboard is the same native Paste event, including Unicode and newlines.
    draw(
        vec![egui::Event::Paste("Bois épais\nG21".into())],
        &mut prompt,
    );
    assert_eq!(prompt, "Profondeur −2 mm\nBois épais\nG21");
}
