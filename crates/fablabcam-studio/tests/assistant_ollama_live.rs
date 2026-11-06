//! Opt-in local acceptance: natural requests, fresh cell IDs, no cloud or credentials.
use fablabcam_studio::{
    assistant::{actions, context::ContextMeta, provider, Action, Connection, ContextOptions, ContextSnapshot, Mode, Provider, Workspace},
    furniture::{self, Composition, Document, Furniture, Template},
};
use std::sync::{atomic::AtomicBool, Arc};

#[tokio::test]
#[ignore = "requires local Ollama and a model configured with num_ctx 16384"]
async fn local_agent_divides_selected_cells_and_continues_after_review() {
    let model = std::env::var("OLLAMA_MODEL").expect("Configurer explicitement OLLAMA_MODEL");
    let c = Connection { model, tools: true, timeout_secs: 180, max_tokens: 4096, ..Connection::for_provider(Provider::Ollama) };
    let mut f = Furniture::from_template(Template::cabinet());
    f.dimensions.width = 600.0;
    f.dimensions.depth = 400.0;
    f.dimensions.height = 800.0;
    f.dimensions.thickness = 18.0;
    f.dimensions.shelves = 2;
    f.dimensions.back = true;
    f.dimensions.composition = Some(Box::new(Composition::from_dimensions(&f.dimensions).unwrap()));
    let mut w = Workspace {
        document_id: "public-local-division".into(),
        revision: 0,
        project: Default::default(),
        script: String::new(),
        furniture: Some(Document::Furniture(Box::new(f))),
    };
    for (vertical, count, prompt) in [
        (true, 2, "Divise la case sélectionnée en deux cases de même largeur, côte à côte. Ne modifie rien d'autre."),
        (false, 3, "Divise la case sélectionnée en trois cases de même hauteur, superposées. Ne modifie rien d'autre."),
    ] {
        let before = w.fingerprint();
        let structure = furniture::structure(w.furniture.as_ref().unwrap().dimensions()).unwrap();
        let id = structure.cells.iter().find(|c| c.leaf).unwrap().id.clone();
        let context = ContextSnapshot::capture(
            &w,
            &ContextMeta { page: "Meubles".into(), cell: Some(id.clone()), ..Default::default() },
            &ContextOptions { furniture: true, ..Default::default() },
        )
        .unwrap();
        let request = provider::request(&c, Mode::Agent, &context, &[], prompt).unwrap();
        let reply = provider::chat(&c, "", request, Mode::Agent, |_| {}).await.expect("Réponse native Ollama");
        let plan = reply.plan.clone().expect("Plan typé attendu");
        assert_eq!(plan.steps.len(), 1);
        let Action::DivideCell { cell_id, vertical: v, count: n } = &plan.steps[0].action else {
            panic!("Action de division attendue");
        };
        assert_eq!(cell_id, &id, "Case sélectionnée");
        assert_eq!(*v, vertical, "Orientation");
        assert_eq!(*n, count, "Nombre de cases");
        let prepared = actions::prepare(&w, &context, plan, Mode::Agent, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(w.fingerprint(), before, "La proposition ne modifie pas le document");
        assert!(actions::authorize(&prepared, &w, &context, Mode::Plan, false).is_err());
        actions::authorize(&prepared, &w, &context, Mode::Agent, false).unwrap();
        w = prepared.after;
        assert_eq!(
            furniture::structure(w.furniture.as_ref().unwrap().dimensions()).unwrap().panels.len(),
            structure.panels.len() + count - 1
        );
        let next = reply.continuation("Lot appliqué après vérification dans le projet synthétique. Résume le résultat en une phrase, sans proposer de nouveau lot.").unwrap();
        let done = provider::chat(&c, "", next, Mode::Agent, |_| {}).await.unwrap();
        assert!(done.plan.is_none());
        assert!(!done.text.trim().is_empty());
        println!(
            "{} : division {} en {} cases, aperçu, autorisation et retour natif réussis",
            c.model,
            if vertical { "verticale" } else { "horizontale" },
            count
        );
    }
}
