use super::{Furniture, Layout};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

/// Uses the PDF base font with WinAnsi for fixed French labels and numeric cotes.
/// Free-form Unicode names are retained without loss in the native JSON/manifest.
pub(super) fn pdf(job: &Furniture, layout: &Layout) -> Vec<u8> {
    let d = &job.dimensions;
    let mut lines = vec![
        format!(
            "FabLabCAM {} — Notice de montage",
            env!("CARGO_PKG_VERSION")
        ),
        format!(
            "Dimensions extérieures : {} × {} × {} mm (L × P × H)",
            d.width, d.depth, d.height
        ),
        format!(
            "Épaisseur principale : {} mm. {} pièce(s). {} dalle(s).",
            d.thickness,
            layout
                .sheets
                .iter()
                .map(|s| s.placements.len())
                .sum::<usize>(),
            layout.sheets.len()
        ),
        if d.composition.is_some() {
            "Assemblages : voir les liaisons et operations.json. Cotes en mm.".into()
        } else {
            "Panneaux vissés à chants droits. Cotes en mm.".into()
        },
        "".into(),
        "1. Repérer chaque pièce avant de retirer les panneaux de la dalle.".into(),
        "2. Ébavurer et vérifier les cotes ; préparer le vissage adapté au matériau.".into(),
        "3. Assembler INF, SUP, C1 et C2 selon les liaisons ; vérifier l'équerrage.".into(),
        if d.composition.is_some() {
            "4. Monter séparations et bacs selon leurs repères et coordonnées.".into()
        } else {
            "4. Monter les tablettes Txx à intervalles réguliers entre INF et SUP.".into()
        },
        if d.back {
            "5. Fixer le DOS rapporté à l'arrière, après contrôle des diagonales.".into()
        } else {
            "5. Contrôler la rigidité et prévoir le contreventement nécessaire.".into()
        },
        "6. Vérifier la stabilité et l'usage prévu avant de charger le meuble.".into(),
        "".into(),
        "Vis, avant-trous, ancrages et charges admissibles ne sont pas calculés.".into(),
        "Plans géométriques uniquement : préparer et valider la FAO séparément.".into(),
        "Un placement réussi ne valide ni la machine, ni le bridage, ni le G-code.".into(),
        "".into(),
        "REPÈRE\tCOTES FINIES (mm)\tDALLE\tTABLETTE Z (mm)".into(),
    ];
    for (i, sheet) in layout.sheets.iter().enumerate() {
        for p in &sheet.placements {
            let z = if p.panel.id.starts_with('T') {
                format!("{:.2}", p.panel.origin[2])
            } else {
                "—".into()
            };
            lines.push(p.panel.id.clone());
            lines.push(format!(
                "{}\t{:.2} × {:.2} × {:.2}\t{}\t{z}",
                "",
                p.panel.width,
                p.panel.height,
                p.panel.thickness,
                i + 1
            ));
        }
    }
    if d.composition.is_some() {
        lines.push("".into());
        lines.push("ASSEMBLAGES ET MONTAGES".into());
        lines.push("Verso avant contours recto ; retournement autour de Y, miroir X.".into());
        lines.push("Repérage, bridage et origines à définir pour chaque montage.".into());
        lines.push("Sans dégagement : finir les angles internes manuellement.".into());
        for sheet in &layout.sheets {
            for p in &sheet.placements {
                for f in &p.panel.features {
                    lines.push(p.panel.id.clone());
                    lines.push(format!(
                        "  {:?}, profondeur {:.2} mm{}",
                        f.face,
                        f.depth,
                        if f.through { " traversant" } else { " borgne" }
                    ));
                }
            }
        }
    }
    let lines = lines
        .into_iter()
        .flat_map(|line| {
            if line.contains('\t') || line.chars().count() <= 78 {
                return vec![line];
            }
            let mut result = Vec::new();
            let mut current = String::new();
            for ch in line.chars() {
                current.push(ch);
                if current.chars().count() == 78 {
                    result.push(std::mem::take(&mut current));
                }
            }
            if !current.is_empty() {
                result.push(current);
            }
            result
        })
        .collect::<Vec<_>>();
    // Keep each stable part ID with its dimensions/operation on the same page.
    let mut pages = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let mut end = (start + 38).min(lines.len());
        if end < lines.len() && (lines[end].starts_with('\t') || lines[end].starts_with("  ")) {
            end -= 1;
        }
        pages.push(&lines[start..end]);
        start = end;
    }
    let mut pdf = Pdf::new();
    let root = Ref::new(1);
    let tree = Ref::new(2);
    let font = Ref::new(3);
    pdf.catalog(root).pages(tree);
    pdf.pages(tree)
        .kids((0..pages.len()).map(|i| Ref::new(4 + i as i32 * 2)))
        .count(pages.len() as i32);
    pdf.type1_font(font)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    for (i, lines) in pages.iter().enumerate() {
        let page_id = Ref::new(4 + i as i32 * 2);
        let content_id = Ref::new(5 + i as i32 * 2);
        let mut page = pdf.page(page_id);
        page.parent(tree)
            .media_box(Rect::new(0.0, 0.0, 595.0, 842.0))
            .contents(content_id);
        page.resources().fonts().pair(Name(b"F1"), font);
        page.finish();
        let mut content = Content::new();
        for (row, line) in lines.iter().enumerate() {
            let y = 795.0 - row as f32 * 19.0;
            for (column, cell) in line.split('\t').enumerate() {
                let x = [40.0, 115.0, 300.0, 380.0][column];
                let size = if i == 0 && row == 0 { 15.0 } else { 11.0 };
                content
                    .begin_text()
                    .set_font(Name(b"F1"), size)
                    .next_line(x, y)
                    .show(Str(&winansi(cell)))
                    .end_text();
            }
        }
        content
            .begin_text()
            .set_font(Name(b"F1"), 9.0)
            .next_line(40.0, 28.0)
            .show(Str(format!(
                "FabLabCAM - Notice de montage - {}/{}",
                i + 1,
                pages.len()
            )
            .as_bytes()))
            .end_text();
        pdf.stream(content_id, &content.finish());
    }
    pdf.finish()
}
fn winansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match c {
            '—' => 151,
            '’' => 146,
            _ if c as u32 <= 255 => c as u8,
            _ => b'?',
        })
        .collect()
}
