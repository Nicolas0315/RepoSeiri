#![forbid(unsafe_code)]

mod claim_draft;

pub use claim_draft::plan_claim_drafts;

use seiri_core::{
    AddExistingRouteLink, AppealAnchorKind, AppealPlanAction, AppealPlanAnchor, AppealPlanItem,
    AppealPresentationMethod, AppealPresentationReport, AppealPresentationSignal,
    ClaimDraftPlanHoldReason, ClaimDraftPlanState, ClaimMode, ClaimStrength, ExistingTargetId,
    GateKind, PatchAnalysisRun, PatchBaseDigest, PatchDecisionBasis, PatchHold, PatchHoldReason,
    PatchPlan, PatchProposal, PatchProposalBinding, PatchProposalDecision, PatchTextEdit,
    RepositoryAnalysis, RouteKind, RouteTargetRole, SupportState, TextDocumentBase, TextEditSpan,
    TextEncoding, UnderclaimOpportunity, UnderclaimOpportunityKind,
};
use std::cmp::Reverse;
use std::path::{Component, Path};

const PATCH_ROUTES: &[RouteKind] = &[
    RouteKind::Docs,
    RouteKind::Quickstart,
    RouteKind::Support,
    RouteKind::Intake,
    RouteKind::Contributing,
    RouteKind::Security,
    RouteKind::Release,
    RouteKind::Lifecycle,
    RouteKind::Governance,
    RouteKind::License,
    RouteKind::Automation,
    RouteKind::Ownership,
    RouteKind::Hygiene,
];

const PLANNER_SEMANTIC_REVISION: &str = seiri_core::PATCH_PLANNER_SEMANTIC_REVISION;

/// Produces bound, dry-run README links to targets that already exist locally.
#[must_use]
pub fn plan_patches(analysis: &RepositoryAnalysis) -> PatchPlan {
    plan_patches_with_geometry(analysis, seiri_appeal::GeometryShadowOptions::default())
}

/// Produces the same bounded patch semantics with an explicitly configurable presentation-only
/// geometry shadow. Geometry never changes gates, support, ceilings, opportunities, or risks.
#[must_use]
pub fn plan_patches_with_geometry(
    analysis: &RepositoryAnalysis,
    geometry: seiri_appeal::GeometryShadowOptions,
) -> PatchPlan {
    let (appeal_suggestions, appeal_presentation) = plan_appeal_presentation(analysis, geometry);
    let mut report = PatchPlan {
        appeal_suggestions,
        appeal_presentation,
        ..PatchPlan::default()
    };
    if PATCH_ROUTES
        .iter()
        .copied()
        .any(|route| try_decision_basis(analysis, route, GateKind::Manual).is_err())
    {
        report.claim_draft_state =
            ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::InvalidContract);
        hold_all(
            analysis,
            &mut report,
            PatchHoldReason::EvidenceContractInvalid,
        );
        return report;
    }
    let Some(readme) = analysis.readme_document.as_ref() else {
        report.claim_draft_state =
            ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::MissingReadme);
        hold_all(analysis, &mut report, PatchHoldReason::MissingReadme);
        return report;
    };
    let Some(document_id) = analysis
        .document_index
        .entries()
        .iter()
        .find(|entry| entry.path == readme.path())
        .and_then(|entry| entry.document_id)
    else {
        report.claim_draft_state =
            ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::MissingReadme);
        hold_all(analysis, &mut report, PatchHoldReason::MissingReadme);
        return report;
    };
    let current = match analysis.source_store().get(readme.path()) {
        Some(source) => source.bytes(),
        None => {
            report.claim_draft_state =
                ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::StaleSource);
            hold_all(analysis, &mut report, PatchHoldReason::StaleBase);
            return report;
        }
    };
    let base = TextDocumentBase::from_bytes(current);
    if base != *readme.base() || base.encoding() == TextEncoding::Unknown {
        report.claim_draft_state =
            ClaimDraftPlanState::Held(if base.encoding() == TextEncoding::Unknown {
                ClaimDraftPlanHoldReason::UnsupportedEncoding
            } else {
                ClaimDraftPlanHoldReason::StaleSource
            });
        hold_all(
            analysis,
            &mut report,
            if base.encoding() == TextEncoding::Unknown {
                PatchHoldReason::UnsupportedEncoding
            } else {
                PatchHoldReason::StaleBase
            },
        );
        return report;
    }

    match plan_claim_drafts(analysis) {
        Ok(claim_drafts) => {
            report.claim_drafts = claim_drafts;
            report.claim_draft_state = ClaimDraftPlanState::Ready;
        }
        Err(_) => {
            report.claim_draft_state =
                ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::InvalidContract);
        }
    }

    let run_digest = seiri_delta::portable_snapshot(analysis)
        .map(|portable| PatchBaseDigest::from_bytes(portable.digest.routes.to_string().as_bytes()))
        .unwrap_or_else(|_| PatchBaseDigest::from_bytes(analysis.schema_version.as_bytes()));
    let analysis_run = PatchAnalysisRun::new(format!("patch-plan-{run_digest}"), run_digest);
    let topology = analysis.language_topology().for_path(readme.path());

    for (ordinal, route) in PATCH_ROUTES.iter().copied().enumerate() {
        if readme_has_route(analysis, route) {
            continue;
        }
        let Some(target_path) = existing_target(analysis, route) else {
            report.held.push(PatchHold {
                route,
                target_path: None,
                reason: PatchHoldReason::NoExistingTarget,
                decision_basis: decision_basis(analysis, route, GateKind::Guarded),
            });
            continue;
        };
        if analysis
            .document_consistency
            .conflicts
            .iter()
            .any(|conflict| conflict.route == route)
        {
            report.held.push(PatchHold {
                route,
                target_path: Some(target_path.to_string()),
                reason: PatchHoldReason::CanonicalConflict,
                decision_basis: decision_basis(analysis, route, GateKind::Manual),
            });
            continue;
        }
        if analysis
            .document_consistency
            .relations
            .iter()
            .any(|relation| {
                relation.route == route && relation.relation == seiri_core::TargetRelation::Unknown
            })
        {
            report.held.push(PatchHold {
                route,
                target_path: Some(target_path.to_string()),
                reason: PatchHoldReason::UnknownTargetRelation,
                decision_basis: decision_basis(analysis, route, GateKind::Manual),
            });
            continue;
        }

        let insertion_points = match insertion_points(topology, current) {
            Some(points) => points,
            None => {
                report.held.push(PatchHold {
                    route,
                    target_path: Some(target_path.to_string()),
                    reason: PatchHoldReason::PairedLanguageIncomplete,
                    decision_basis: decision_basis(analysis, route, GateKind::Manual),
                });
                continue;
            }
        };
        let paired_language = insertion_points.len() == 2;
        let eol = base.line_ending().sequence().unwrap_or("\n");
        let edits = insertion_points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let label = route_label(route, point.language);
                PatchTextEdit::literal(
                    format!("patch-edit-{}-{}", ordinal + 1, index + 1),
                    TextEditSpan::insertion(point.offset),
                    format!("{eol}- [{label}]({target_path}){eol}"),
                )
            })
            .collect::<Vec<_>>();
        let proposal = PatchProposal::new(
            format!("patch-proposal-{}", ordinal + 1),
            readme.path(),
            base.clone(),
            edits,
        );
        if proposal.preflight_against(current).decision != PatchProposalDecision::Ready {
            report.held.push(PatchHold {
                route,
                target_path: Some(target_path.to_string()),
                reason: PatchHoldReason::StaleAnchor,
                decision_basis: decision_basis(analysis, route, GateKind::Guarded),
            });
            continue;
        }
        let Ok(binding) = PatchProposalBinding::bind(analysis_run.clone(), &proposal, current)
        else {
            report.held.push(PatchHold {
                route,
                target_path: Some(target_path.to_string()),
                reason: PatchHoldReason::StaleAnchor,
                decision_basis: decision_basis(analysis, route, GateKind::Guarded),
            });
            continue;
        };
        let Some(insertion_anchor) = binding.anchors.first().cloned() else {
            report.held.push(PatchHold {
                route,
                target_path: Some(target_path.to_string()),
                reason: PatchHoldReason::StaleAnchor,
                decision_basis: decision_basis(analysis, route, GateKind::Guarded),
            });
            continue;
        };
        report.operations.push(AddExistingRouteLink {
            route,
            target: ExistingTargetId((ordinal + 1) as u32),
            target_path: target_path.to_string(),
            target_role: RouteTargetRole::Canonical,
            document: document_id,
            insertion_anchor,
            analysis_run: analysis_run.clone(),
            proposal,
            binding,
            paired_language,
            decision_basis: decision_basis(analysis, route, GateKind::Safe),
        });
    }
    report.operations.sort_by_key(|operation| operation.route);
    report.held.sort_by_key(|item| item.route);
    report
}

fn plan_appeal_presentation(
    analysis: &RepositoryAnalysis,
    options: seiri_appeal::GeometryShadowOptions,
) -> (Vec<AppealPlanItem>, AppealPresentationReport) {
    let mut suggestions = plan_appeal_suggestions(analysis);
    let ranking = seiri_appeal::rank_appeal_presentation(
        &analysis.readme_grammar,
        &analysis.claim_capability_membrane,
        options,
    );
    let priorities = ranking
        .signals
        .iter()
        .map(|signal| {
            (
                format!("appeal-suggestion-{:04}", signal.opportunity_index + 1),
                signal.presentation_priority,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if ranking.enabled {
        suggestions.sort_by_key(|item| {
            (
                gate_rank(item.gate),
                Reverse(priorities.get(&item.id).copied().unwrap_or(0)),
                item.opportunity,
                item.dimension,
                item.id.clone(),
            )
        });
    }
    let ordered_suggestion_ids = suggestions
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let signals = ranking
        .signals
        .into_iter()
        .map(|signal| AppealPresentationSignal {
            suggestion_id: format!("appeal-suggestion-{:04}", signal.opportunity_index + 1),
            grammar_nodes: signal.grammar_nodes,
            first_value_distance: signal.first_value_distance,
            local_degree: signal.local_degree,
            forman_curvature: signal.forman_curvature,
            presentation_priority: signal.presentation_priority,
        })
        .collect::<Vec<_>>();
    let report = AppealPresentationReport {
        method: if ranking.enabled {
            AppealPresentationMethod::GeometryShadowV1
        } else {
            AppealPresentationMethod::Canonical
        },
        coverage: ranking.coverage,
        ordered_suggestion_ids,
        signals,
        membrane_semantic_digest: ranking.membrane_semantic_digest,
        boundary: "Presentation signals are bounded graph heuristics, not evidence. They cannot change a gate, support relation, claim ceiling, opportunity, risk, or authorize claim text."
            .to_string(),
    };
    (suggestions, report)
}

/// Projects membrane opportunities into source-bound review work without generating prose.
#[must_use]
pub fn plan_appeal_suggestions(analysis: &RepositoryAnalysis) -> Vec<AppealPlanItem> {
    let membrane = &analysis.claim_capability_membrane;
    let mut suggestions = membrane
        .opportunities
        .iter()
        .enumerate()
        .map(|(index, opportunity)| {
            let relation = membrane
                .relations
                .iter()
                .find(|relation| relation.dimension == opportunity.dimension);
            let support_state = relation.map_or(
                SupportState::Unknown(seiri_core::UnknownReason::NotRequested),
                |relation| relation.state,
            );
            let claim_ceiling = membrane
                .ceiling
                .limits
                .get(&opportunity.dimension)
                .copied()
                .unwrap_or(ClaimMode::Omitted);
            let gate = bounded_gate(opportunity, support_state, claim_ceiling);
            AppealPlanItem {
                id: format!("appeal-suggestion-{:04}", index + 1),
                opportunity: opportunity.kind,
                dimension: opportunity.dimension,
                gate,
                action: appeal_action(opportunity.kind),
                support_state,
                claim_ceiling,
                anchors: appeal_anchors(analysis, opportunity),
                source_session_digest: analysis.analysis_configuration.source_session_digest,
                membrane_semantic_revision: seiri_core::CLAIM_CAPABILITY_MEMBRANE_REVISION
                    .to_string(),
                planner_semantic_revision: PLANNER_SEMANTIC_REVISION.to_string(),
                boundary: "Review the cited existing README and capability evidence. This item supplies no claim text and cannot authorize wording above the recorded ceiling."
                    .to_string(),
            }
        })
        .collect::<Vec<_>>();
    suggestions.sort_by_key(|item| {
        (
            gate_rank(item.gate),
            item.opportunity,
            item.dimension,
            item.id.clone(),
        )
    });
    suggestions
}

const fn bounded_gate(
    opportunity: &UnderclaimOpportunity,
    support: SupportState,
    ceiling: ClaimMode,
) -> GateKind {
    match opportunity.gate {
        GateKind::Safe => GateKind::Safe,
        GateKind::Guarded
            if matches!(support, SupportState::Supported)
                && !matches!(ceiling, ClaimMode::Omitted) =>
        {
            GateKind::Guarded
        }
        GateKind::Guarded | GateKind::Manual => GateKind::Manual,
    }
}

const fn appeal_action(kind: UnderclaimOpportunityKind) -> AppealPlanAction {
    match kind {
        UnderclaimOpportunityKind::BuriedPrimaryValue => AppealPlanAction::MoveExistingValueEarlier,
        UnderclaimOpportunityKind::CapabilityOutcomeDisconnect
        | UnderclaimOpportunityKind::FirstValueDisconnect
        | UnderclaimOpportunityKind::FragmentedValue => AppealPlanAction::ConnectExistingValuePath,
        UnderclaimOpportunityKind::EvidenceDisconnect => {
            AppealPlanAction::AssociateExistingEvidence
        }
        UnderclaimOpportunityKind::MissingSupportedValue
        | UnderclaimOpportunityKind::UnexpressedConstraintBackedAdvantage => {
            AppealPlanAction::ExpressSupportedValue
        }
        UnderclaimOpportunityKind::ModeBelowFloor => AppealPlanAction::RaiseSupportedSpecificity,
        UnderclaimOpportunityKind::TranslationDivergence => {
            AppealPlanAction::ReviewTranslationAlignment
        }
        UnderclaimOpportunityKind::GenericVerbCollapse
        | UnderclaimOpportunityKind::JargonOcclusion
        | UnderclaimOpportunityKind::QualifierDominance => AppealPlanAction::ClarifyExistingValue,
    }
}

fn appeal_anchors(
    analysis: &RepositoryAnalysis,
    opportunity: &UnderclaimOpportunity,
) -> Vec<AppealPlanAnchor> {
    let mut anchors = opportunity
        .grammar_nodes
        .iter()
        .filter_map(|id| {
            analysis
                .readme_grammar
                .nodes
                .iter()
                .find(|node| node.id == *id)
                .map(|node| AppealPlanAnchor {
                    kind: AppealAnchorKind::ReadmeGrammar,
                    path: analysis.readme_grammar.path.clone(),
                    span: node.span,
                    grammar_node: Some(node.id),
                    capability_node: None,
                })
        })
        .chain(opportunity.capability_nodes.iter().flat_map(|id| {
            analysis
                .repository_capabilities
                .nodes
                .iter()
                .filter(move |node| node.id == *id)
                .flat_map(|node| {
                    node.provenance
                        .iter()
                        .map(move |provenance| AppealPlanAnchor {
                            kind: AppealAnchorKind::CapabilityProvenance,
                            path: provenance.path.clone(),
                            span: provenance.span,
                            grammar_node: None,
                            capability_node: Some(node.id),
                        })
                })
        }))
        .collect::<Vec<_>>();
    anchors.sort_by_key(|anchor| {
        (
            anchor.kind,
            anchor.path.clone(),
            anchor.span.map_or(usize::MAX, |span| span.byte_start),
            anchor.grammar_node,
            anchor.capability_node,
        )
    });
    anchors.dedup();
    anchors
}

const fn gate_rank(gate: GateKind) -> u8 {
    match gate {
        GateKind::Safe => 0,
        GateKind::Guarded => 1,
        GateKind::Manual => 2,
    }
}

fn hold_all(analysis: &RepositoryAnalysis, report: &mut PatchPlan, reason: PatchHoldReason) {
    report
        .held
        .extend(PATCH_ROUTES.iter().copied().map(|route| PatchHold {
            route,
            target_path: None,
            reason,
            decision_basis: decision_basis(analysis, route, GateKind::Manual),
        }));
}

fn decision_basis(
    analysis: &RepositoryAnalysis,
    route: RouteKind,
    gate: GateKind,
) -> PatchDecisionBasis {
    try_decision_basis(analysis, route, gate)
        .unwrap_or_else(|_| decision_basis_without_evidence(analysis, route, gate))
}

fn try_decision_basis(
    analysis: &RepositoryAnalysis,
    route: RouteKind,
    gate: GateKind,
) -> Result<PatchDecisionBasis, seiri_delta::DeltaError> {
    let mut claims = analysis
        .claims
        .iter()
        .filter(|claim| claim.route() == route)
        .collect::<Vec<_>>();
    claims.sort_by_key(|claim| {
        (
            claim.strength() != ClaimStrength::Observed,
            claim.id().clone(),
        )
    });
    let claim_ids = claims
        .iter()
        .map(|claim| claim.id().clone())
        .collect::<Vec<_>>();
    let mut evidence_ids = claims
        .iter()
        .flat_map(|claim| claim.evidence_ids().iter().copied())
        .collect::<Vec<_>>();
    evidence_ids.sort_unstable();
    evidence_ids.dedup();
    let evidence_fingerprints =
        seiri_delta::evidence_fingerprints_for_ids(analysis, &evidence_ids)?;
    let priority_rank = analysis
        .missing_route_priority
        .priorities
        .iter()
        .position(|priority| priority.route == route)
        .map(|index| index + 1);
    Ok(PatchDecisionBasis {
        gate,
        priority_rank,
        claim_ids,
        evidence_fingerprints,
        claim_semantic_revision: seiri_core::CLAIM_SEMANTIC_REVISION.to_string(),
        planner_semantic_revision: PLANNER_SEMANTIC_REVISION.to_string(),
        source_session_digest: analysis.analysis_configuration.source_session_digest,
    })
}

fn decision_basis_without_evidence(
    analysis: &RepositoryAnalysis,
    route: RouteKind,
    gate: GateKind,
) -> PatchDecisionBasis {
    let mut claims = analysis
        .claims
        .iter()
        .filter(|claim| claim.route() == route)
        .collect::<Vec<_>>();
    claims.sort_by_key(|claim| {
        (
            claim.strength() != ClaimStrength::Observed,
            claim.id().clone(),
        )
    });
    PatchDecisionBasis {
        gate,
        priority_rank: analysis
            .missing_route_priority
            .priorities
            .iter()
            .position(|priority| priority.route == route)
            .map(|index| index + 1),
        claim_ids: claims.iter().map(|claim| claim.id().clone()).collect(),
        evidence_fingerprints: Vec::new(),
        claim_semantic_revision: seiri_core::CLAIM_SEMANTIC_REVISION.to_string(),
        planner_semantic_revision: PLANNER_SEMANTIC_REVISION.to_string(),
        source_session_digest: analysis.analysis_configuration.source_session_digest,
    }
}

fn readme_has_route(analysis: &RepositoryAnalysis, route: RouteKind) -> bool {
    analysis
        .route_assessments
        .iter()
        .find(|assessment| assessment.route() == route)
        .is_some_and(|assessment| assessment.readme().routing().is_present())
}

fn existing_target(analysis: &RepositoryAnalysis, route: RouteKind) -> Option<&str> {
    route.target_candidates().iter().copied().find(|candidate| {
        let canonical_candidate = candidate.trim_end_matches('/');
        is_safe_relative(candidate)
            && analysis.files.iter().any(|record| {
                record.path == canonical_candidate
                    || (candidate.ends_with('/') && record.path.starts_with(candidate))
            })
    })
}

fn is_safe_relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

#[derive(Debug, Clone, Copy)]
struct InsertionPoint {
    offset: usize,
    language: seiri_core::DocumentLanguage,
}

fn insertion_points(
    topology: Option<seiri_core::LanguageTopology>,
    source: &[u8],
) -> Option<Vec<InsertionPoint>> {
    let points = match topology? {
        seiri_core::LanguageTopology::Monolingual(language) => vec![InsertionPoint {
            offset: source.len(),
            language,
        }],
        seiri_core::LanguageTopology::Parallel {
            japanese_insertion,
            english_insertion,
        } => vec![
            InsertionPoint {
                offset: japanese_insertion,
                language: seiri_core::DocumentLanguage::Japanese,
            },
            InsertionPoint {
                offset: english_insertion,
                language: seiri_core::DocumentLanguage::English,
            },
        ],
        seiri_core::LanguageTopology::Ambiguous => return None,
    };
    let text = std::str::from_utf8(source).ok()?;
    if points.len() == 2 && points[0].offset == points[1].offset {
        return None;
    }
    if points
        .iter()
        .any(|point| point.offset > source.len() || !text.is_char_boundary(point.offset))
    {
        None
    } else {
        Some(points)
    }
}

fn route_label(route: RouteKind, language: seiri_core::DocumentLanguage) -> &'static str {
    route.label(language)
}
