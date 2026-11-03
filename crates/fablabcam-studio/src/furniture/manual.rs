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
            "Épaisseur : {} mm. {} tablette(s). {} dalle(s).",
            d.thickness,
            d.shelves,
            layout.sheets.len()
        ),
        "Assemblage V1 : panneaux vissés à chants droits. Cotes finies en mm.".into(),
        "".into(),
        "1. Repérer chaque pièce avant de retirer les panneaux de la dalle.".into(),
        "2. Ébavurer et vérifier les cotes ; préparer le vissage adapté au matériau.".into(),
        "3. Placer INF et SUP entre les côtés C1 et C2 ; vérifier l'équerrage.".into(),
        "4. Monter les tablettes Txx à intervalles réguliers entre INF et SUP.".into(),
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
            lines.push(format!(
                "{}\t{} × {} × {}\t{}\t{z}",
                p.panel.id,
                p.panel.width,
                p.panel.height,
                p.panel.thickness,
                i + 1
            ));
        }
    }
    let pages = lines.chunks(38).collect::<Vec<_>>();
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
