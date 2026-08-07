use seiri_core::{
    AppealAnchorKind, AppealPlanAction, ClaimMode, GateKind, ProfileKind, SupportState,
    UnderclaimOpportunityKind, ValueDimension,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn supported_program_value_becomes_guarded_source_bound_review_work() {
    let root = temp_repository("supported");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"appeal-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    fs::create_dir(root.join("src")).expect("src");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn normalize(input: &str) -> String { input.to_owned() }\n",
    )
    .expect("source");
    fs::write(root.join("README.md"), "# Appeal fixture\n").expect("README");

    let analysis =
        seiri_report::audit_repository_with_profile(&root, ProfileKind::Library).expect("audit");
    let plan = seiri_planner::plan_patches(&analysis);
    let item = plan
        .appeal_suggestions
        .iter()
        .find(|item| {
            item.opportunity == UnderclaimOpportunityKind::MissingSupportedValue
                && item.dimension == ValueDimension::Capability
        })
        .expect("supported missing capability suggestion");

    assert_eq!(item.gate, GateKind::Guarded);
    assert_eq!(item.action, AppealPlanAction::ExpressSupportedValue);
    assert_eq!(item.support_state, SupportState::Supported);
    assert_ne!(item.claim_ceiling, ClaimMode::Omitted);
    assert!(item.anchors.iter().any(|anchor| {
        anchor.kind == AppealAnchorKind::CapabilityProvenance
            && (anchor.path == "Cargo.toml" || anchor.path == "src/lib.rs")
            && anchor.capability_node.is_some()
    }));
    assert!(item.boundary.contains("supplies no claim text"));
    assert!(!plan.writes_files);

    cleanup(root);
}

#[test]
fn unknown_or_unsupported_dimensions_never_become_guarded_copy_work() {
    let mut analysis = seiri_core::RepositoryAnalysis::new(".");
    analysis
        .claim_capability_membrane
        .opportunities
        .push(seiri_core::UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::ModeBelowFloor,
            dimension: ValueDimension::Outcome,
            gate: GateKind::Guarded,
            grammar_nodes: Vec::new(),
            capability_nodes: Vec::new(),
        });
    analysis
        .claim_capability_membrane
        .ceiling
        .limits
        .insert(ValueDimension::Outcome, ClaimMode::Qualified);

    let suggestions = seiri_planner::plan_appeal_suggestions(&analysis);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].gate, GateKind::Manual);
    assert_eq!(
        suggestions[0].support_state,
        SupportState::Unknown(seiri_core::UnknownReason::NotRequested)
    );
    assert!(suggestions[0].anchors.is_empty());
}

#[test]
fn safe_narrative_work_only_repositions_existing_readme_meaning() {
    let mut analysis = seiri_core::RepositoryAnalysis::new(".");
    let grammar_id = seiri_core::GrammarNodeId::new(std::num::NonZeroU32::MIN);
    analysis.readme_grammar = seiri_core::ReadmeGrammarIR::try_new(
        "README.md",
        seiri_core::CoverageStatus::Complete,
        vec![seiri_core::GrammarNode {
            id: grammar_id,
            predicate: seiri_core::GrammarPredicate::DefinesIdentity,
            state: seiri_core::AnswerState::Explicit,
            language: seiri_core::DocumentLanguage::English,
            modality: seiri_core::ClaimModality::Asserted,
            negated: false,
            span: Some(seiri_core::SourceSpan::new(24, 1, 100, 120)),
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("grammar");
    analysis
        .claim_capability_membrane
        .opportunities
        .push(seiri_core::UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::BuriedPrimaryValue,
            dimension: ValueDimension::Identity,
            gate: GateKind::Safe,
            grammar_nodes: vec![grammar_id],
            capability_nodes: Vec::new(),
        });

    let suggestions = seiri_planner::plan_appeal_suggestions(&analysis);
    assert_eq!(suggestions[0].gate, GateKind::Safe);
    assert_eq!(
        suggestions[0].action,
        AppealPlanAction::MoveExistingValueEarlier
    );
    assert_eq!(suggestions[0].anchors.len(), 1);
    assert_eq!(suggestions[0].anchors[0].path, "README.md");
    assert_eq!(suggestions[0].anchors[0].grammar_node, Some(grammar_id));
}

#[test]
fn geometry_shadow_only_reorders_equal_gate_suggestions() {
    let mut analysis = seiri_core::RepositoryAnalysis::new(".");
    let ids = (1..=4)
        .map(|value| {
            seiri_core::GrammarNodeId::new(
                std::num::NonZeroU32::new(value).expect("non-zero grammar id"),
            )
        })
        .collect::<Vec<_>>();
    let predicates = [
        seiri_core::GrammarPredicate::DefinesIdentity,
        seiri_core::GrammarPredicate::TargetsAudience,
        seiri_core::GrammarPredicate::StatesProblem,
        seiri_core::GrammarPredicate::ProducesOutcome,
    ];
    let nodes = ids
        .iter()
        .zip(predicates)
        .enumerate()
        .map(|(index, (id, predicate))| seiri_core::GrammarNode {
            id: *id,
            predicate,
            state: seiri_core::AnswerState::Explicit,
            language: seiri_core::DocumentLanguage::English,
            modality: seiri_core::ClaimModality::Asserted,
            negated: false,
            span: Some(seiri_core::SourceSpan::new(
                index + 1,
                1,
                index * 10,
                index * 10 + 5,
            )),
        })
        .collect::<Vec<_>>();
    let edges = ids
        .windows(2)
        .map(|pair| seiri_core::GrammarEdge {
            from: pair[0],
            to: pair[1],
            relation: seiri_core::NarrativeRelation::Precedes,
        })
        .collect::<Vec<_>>();
    analysis.readme_grammar = seiri_core::ReadmeGrammarIR::try_new(
        "README.md",
        seiri_core::CoverageStatus::Complete,
        nodes,
        edges,
        Vec::new(),
    )
    .expect("grammar");
    analysis.claim_capability_membrane.opportunities = vec![
        seiri_core::UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::QualifierDominance,
            dimension: ValueDimension::Audience,
            gate: GateKind::Safe,
            grammar_nodes: vec![ids[1]],
            capability_nodes: Vec::new(),
        },
        seiri_core::UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::BuriedPrimaryValue,
            dimension: ValueDimension::Outcome,
            gate: GateKind::Safe,
            grammar_nodes: vec![ids[3]],
            capability_nodes: Vec::new(),
        },
        seiri_core::UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::FirstValueDisconnect,
            dimension: ValueDimension::FirstResult,
            gate: GateKind::Manual,
            grammar_nodes: vec![ids[3]],
            capability_nodes: Vec::new(),
        },
    ];

    let canonical = seiri_planner::plan_patches_with_geometry(
        &analysis,
        seiri_appeal::GeometryShadowOptions::disabled(),
    );
    let geometry = seiri_planner::plan_patches_with_geometry(
        &analysis,
        seiri_appeal::GeometryShadowOptions::default(),
    );

    assert_eq!(canonical.operations, geometry.operations);
    assert_eq!(canonical.held, geometry.held);
    assert_eq!(
        canonical.appeal_presentation.membrane_semantic_digest,
        geometry.appeal_presentation.membrane_semantic_digest
    );
    assert_eq!(
        canonical.appeal_presentation.method,
        seiri_core::AppealPresentationMethod::Canonical
    );
    assert_eq!(
        geometry.appeal_presentation.method,
        seiri_core::AppealPresentationMethod::GeometryShadowV1
    );
    assert_ne!(
        canonical.appeal_presentation.ordered_suggestion_ids,
        geometry.appeal_presentation.ordered_suggestion_ids
    );
    let mut canonical_by_id = canonical.appeal_suggestions.clone();
    canonical_by_id.sort_by(|left, right| left.id.cmp(&right.id));
    let mut geometry_by_id = geometry.appeal_suggestions.clone();
    geometry_by_id.sort_by(|left, right| left.id.cmp(&right.id));
    assert_eq!(canonical_by_id, geometry_by_id);
    assert_eq!(
        geometry.appeal_suggestions.last().map(|item| item.gate),
        Some(GateKind::Manual),
        "geometry must not move a Manual item ahead of Safe items"
    );
    assert!(!canonical.writes_files);
    assert!(!geometry.writes_files);
}

fn temp_repository(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("reposeiri-appeal-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("temporary repository");
    root
}

fn cleanup(root: PathBuf) {
    fs::remove_dir_all(root).expect("remove temporary repository");
}
