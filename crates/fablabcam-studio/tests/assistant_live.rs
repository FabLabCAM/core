//! Opt-in, billable provider acceptance test. Never print credentials or response bodies.
use fablabcam_studio::{
    assistant::{actions, context::ContextMeta, provider, Connection, ContextOptions, ContextSnapshot, Mode, Provider, Workspace},
    furniture,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

#[tokio::test]
#[ignore = "needs explicitly configured real providers; sends billable synthetic requests"]
async fn real_providers_agent_cabinet_roundtrip() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Read the known development file without sourcing shell code or modifying process env.
    let env: HashMap<String, String> = dotenvy::from_path_iter(root.join(".env"))
        .expect("Créer .env depuis .env.example")
        .map(|r| r.unwrap_or_else(|_| panic!("Syntaxe .env invalide")))
        .collect();
    let value = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty()).or_else(|| env.get(key).filter(|v| !v.is_empty()).cloned());
    let selected = value("FABLABCAM_TEST_PROVIDER");
    assert!(selected.as_ref().is_none_or(|s| Provider::ALL.iter().any(|p| p.label() == s)), "Fournisseur de test inconnu");
    for (p, model_var) in [
        (Provider::OpenAi, "OPENAI_MODEL"),
        (Provider::Claude, "ANTHROPIC_MODEL"),
        (Provider::Mistral, "MISTRAL_MODEL"),
        (Provider::Ollama, "OLLAMA_MODEL"),
    ] {
        if selected.as_ref().is_some_and(|s| s != p.label()) {
            continue;
        }
        let model = value(model_var).unwrap_or_else(|| panic!("Configurer {model_var}"));
        let secret = if p == Provider::Ollama { String::new() } else { value(p.env()).unwrap_or_else(|| panic!("Configurer {}", p.env())) };
        let mut c = Connection { model, tools: true, timeout_secs: 180, max_tokens: 8192, ..Connection::for_provider(p) };
        if p == Provider::Ollama {
            if let Some(host) = value("OLLAMA_HOST") {
                c.endpoint = host;
            }
        }
        let w = Workspace {
            document_id: "test-public-caisson".into(),
            revision: 0,
            project: Default::default(),
            script: String::new(),
            furniture: None,
        };
        let context = ContextSnapshot::capture(
            &w,
            &ContextMeta { page: "Meubles".into(), ..Default::default() },
            &ContextOptions { furniture: true, ..Default::default() },
        )
        .unwrap();
        let prompt="Propose exactement une action create_cabinet via submit_plan : name Caisson test, width 600 mm, depth 400 mm, height 800 mm, thickness 18 mm, shelves 2, back true. Toutes les informations sont fournies. N'ajoute aucune autre action.";
        let request = provider::request(&c, Mode::Agent, &context, &[], prompt).unwrap();
        let reply = provider::chat(&c, &secret, request, Mode::Agent, |_| {}).await.unwrap_or_else(|e| panic!("{} : {e}", p.label()));
        println!("{} : création du caisson reçue", p.label());
        let plan = reply.plan.clone().unwrap_or_else(|| panic!("{} : aucun plan typé", p.label()));
        assert_eq!(plan.steps.len(), 1, "{} : lot inattendu", p.label());
        let prepared = actions::prepare(&w, &context, plan, Mode::Agent, Arc::new(AtomicBool::new(false))).unwrap();
        actions::authorize(&prepared, &w, &context, Mode::Agent, false).unwrap();
        let doc = prepared.after.furniture.as_ref().expect("caisson absent");
        assert_eq!((doc.dimensions().width, doc.dimensions().depth, doc.dimensions().height), (600.0, 400.0, 800.0));
        assert_eq!(furniture::panels(doc.template(), doc.dimensions()).unwrap().len(), 7);
        let request=reply.continuation("Lot validé dans un projet de test isolé : caisson 600 × 400 × 800, 7 panneaux. Résume en une phrase ; aucun autre appel d'outil.").unwrap();
        let done =
            provider::chat(&c, &secret, request, Mode::Agent, |_| {}).await.unwrap_or_else(|e| panic!("{} retour outil : {e}", p.label()));
        println!("{} : retour outil reçu", p.label());
        assert!(done.plan.is_none());
        assert!(!done.text.is_empty());
        let mut w = prepared.after;
        let doc = w.furniture.as_ref().unwrap();
        let cell = furniture::structure(doc.dimensions()).unwrap().cells.into_iter().find(|c| c.leaf).unwrap().id;
        let divided=propose(&c,&secret,&w,Mode::Agent,&format!("Divise uniquement la case {cell} en deux cases de même largeur, côte à côte. Ne modifie rien d'autre. Toutes les informations sont fournies.")).await;
        assert!(matches!(&divided.plan.steps[0].action,
            fablabcam_studio::assistant::Action::DivideCell { cell_id, vertical: true, count: 2 } if cell_id == &cell));
        w = divided.after;
        assert_eq!(furniture::structure(w.furniture.as_ref().unwrap().dimensions()).unwrap().panels.len(), 8);
        let edited=propose(&c,&secret,&w,Mode::Plan,"Remplace uniquement le texte du script par circle(10,10,2.0); via replace_script. N'exécute pas le script. Aucun paramètre ne manque.").await;
        assert_eq!(edited.after.project, w.project);
        assert!(actions::authorize(&edited, &w, &live_context(&w), Mode::Plan, false).is_err());
        w = edited.after;
        let executed = propose(
            &c,
            &secret,
            &w,
            Mode::Agent,
            "Propose exactement run_script avec le texte du script transmis. Aucun autre changement. Tous les paramètres sont connus.",
        )
        .await;
        w = executed.after;
        assert_eq!(w.project.shapes.len(), 1);
        let tools=propose(&c,&secret,&w,Mode::Agent,"Pour ce test synthétique, crée un outil nommé Fraise 6, kind endmill, diameter 6 mm, feed 600 mm/min, plunge 200 mm/min, spindle 12000 tr/min, puis duplique-le sous le nom Fraise 4 avec diameter 4 mm. Ces valeurs sont les données du test, pas une recommandation d'usinage. Deux actions create_tool puis duplicate_tool uniquement.").await;
        assert_eq!(tools.after.project.tool("Fraise 6").unwrap().diameter, 6.0);
        assert_eq!(tools.after.project.tool("Fraise 4").unwrap().diameter, 4.0);
        assert_eq!(tools.after.project.tool("Fraise 4").unwrap().feed, 600.0);
        assert!(tools.after.project.ops.is_empty());
        let mut context = live_context(&w);
        context.data["simulation"] = serde_json::json!({"progress":0.58,"complete":false,"gouges":0});
        let request = provider::request(
            &c,
            Mode::Ask,
            &context,
            &[],
            "La simulation est-elle terminée ? Donne son pourcentage et sa limite en une phrase.",
        )
        .unwrap();
        let reply = provider::chat(&c, &secret, request, Mode::Ask, |_| {}).await.unwrap();
        assert!(reply.plan.is_none());
        assert!(reply.text.contains("58"));
        println!("{} / {} : flux, caisson, case, Plan/Agent, script, outils, simulation et retour d'outil réussis", p.label(), c.model);
    }
}

fn live_context(w: &Workspace) -> ContextSnapshot {
    ContextSnapshot::capture(
        w,
        &ContextMeta { page: "Test atelier".into(), ..Default::default() },
        &ContextOptions { furniture: true, script: true, selected_only: false, ..Default::default() },
    )
    .unwrap()
}
async fn propose(c: &Connection, secret: &str, w: &Workspace, mode: Mode, prompt: &str) -> fablabcam_studio::assistant::PreparedPlan {
    let context = live_context(w);
    let request = provider::request(c, mode, &context, &[], prompt).unwrap();
    let reply = provider::chat(c, secret, request, mode, |_| {}).await.unwrap();
    println!("{} : proposition {:?} reçue", c.provider.label(), mode);
    let plan = reply.plan.expect("Plan typé attendu");
    let prepared = actions::prepare(w, &context, plan, mode, Arc::new(AtomicBool::new(false))).unwrap();
    actions::authorize(&prepared, w, &context, Mode::Agent, false).unwrap();
    prepared
}
