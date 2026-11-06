//! Reuse message text without changing conversation history or sending a request.
use eframe::egui;

pub(super) fn actions(
    ui: &mut egui::Ui,
    index: usize,
    user: bool,
    text: &str,
    prompt: &mut String,
) {
    ui.push_id(("message-actions", index), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(if user { "Vous" } else { "Assistant" });
            if ui
                .small_button("Copier")
                .on_hover_text("Copier le message complet, avec sa mise en forme Markdown")
                .clicked()
            {
                ui.output_mut(|output| output.copied_text = text.to_owned());
            }
            if ui
                .small_button("Ajouter au prompt")
                .on_hover_text("Ajouter ce message à la saisie sans l'envoyer")
                .clicked()
            {
                if !prompt.is_empty() {
                    prompt.push_str("\n\n");
                }
                prompt.push_str(text);
            }
        });
        egui::CollapsingHeader::new("Texte sélectionnable").show(ui, |ui| {
            // An immutable TextBuffer keeps history read-only while retaining selection and Copy.
            // A disabled widget would also disable selection, so use &str, not add_enabled(false).
            let mut source = text;
            ui.add(
                egui::TextEdit::multiline(&mut source)
                    .id_source("message-source")
                    .desired_width(f32::INFINITY)
                    .desired_rows(3),
            );
        });
    });
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
