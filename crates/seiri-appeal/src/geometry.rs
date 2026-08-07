use super::{gate_rank, membrane_semantic_digest, opportunity_key};
use seiri_core::{
    ClaimCapabilityMembrane, CoverageIncompleteReason, CoverageStatus, GrammarNodeId,
    GrammarPredicate, ReadmeGrammarIR, UnderclaimOpportunityKind,
};
use seiri_digest::Digest32;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const GEOMETRY_SHADOW_REVISION: &str = "seiri.geometry-shadow.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryShadowOptions {
    pub enabled: bool,
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for GeometryShadowOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_nodes: 4_096,
            max_edges: 8_192,
        }
    }
}

impl GeometryShadowOptions {
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            max_nodes: 0,
            max_edges: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryShadowSignal {
    pub opportunity_index: usize,
    pub grammar_nodes: Vec<GrammarNodeId>,
    pub first_value_distance: Option<usize>,
    pub local_degree: usize,
    pub forman_curvature: i32,
    pub presentation_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryShadowRanking {
    pub enabled: bool,
    pub coverage: CoverageStatus,
    pub ordered_opportunity_indices: Vec<usize>,
    pub signals: Vec<GeometryShadowSignal>,
    pub membrane_semantic_digest: Digest32,
}

/// Ranks presentation only. The returned graph signals are not evidence and cannot mutate the
/// membrane, its support relations, claim ceilings, opportunities, risks, or gates.
#[must_use]
pub fn rank_appeal_presentation(
    grammar: &ReadmeGrammarIR,
    membrane: &ClaimCapabilityMembrane,
    options: GeometryShadowOptions,
) -> GeometryShadowRanking {
    let membrane_digest = membrane_semantic_digest(membrane);
    if !options.enabled {
        return GeometryShadowRanking {
            enabled: false,
            coverage: CoverageStatus::NotRequested,
            ordered_opportunity_indices: canonical_order(membrane),
            signals: Vec::new(),
            membrane_semantic_digest: membrane_digest,
        };
    }

    let selected_nodes = grammar
        .nodes
        .iter()
        .take(options.max_nodes)
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    let mut adjacency = selected_nodes
        .iter()
        .copied()
        .map(|id| (id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let eligible_edges = grammar
        .edges
        .iter()
        .filter(|edge| selected_nodes.contains(&edge.from) && selected_nodes.contains(&edge.to))
        .collect::<Vec<_>>();
    for edge in eligible_edges.iter().take(options.max_edges) {
        adjacency.entry(edge.from).or_default().insert(edge.to);
        adjacency.entry(edge.to).or_default().insert(edge.from);
    }

    let truncated =
        grammar.nodes.len() > options.max_nodes || eligible_edges.len() > options.max_edges;
    let coverage = if truncated {
        CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
    } else {
        grammar.coverage
    };
    let first_value = grammar
        .nodes
        .iter()
        .filter(|node| selected_nodes.contains(&node.id) && is_first_value(node.predicate))
        .min_by_key(|node| {
            (
                node.span.map_or(usize::MAX, |span| span.byte_start),
                node.id,
            )
        })
        .map(|node| node.id);
    let distances =
        first_value.map_or_else(BTreeMap::new, |start| distances_from(start, &adjacency));

    let mut signals = membrane
        .opportunities
        .iter()
        .enumerate()
        .map(|(opportunity_index, opportunity)| {
            let mut grammar_nodes = opportunity
                .grammar_nodes
                .iter()
                .copied()
                .filter(|id| selected_nodes.contains(id))
                .collect::<Vec<_>>();
            grammar_nodes.sort_unstable();
            grammar_nodes.dedup();
            let first_value_distance = grammar_nodes
                .iter()
                .filter_map(|id| distances.get(id).copied())
                .min();
            let local_degree = grammar_nodes
                .iter()
                .filter_map(|id| adjacency.get(id).map(BTreeSet::len))
                .max()
                .unwrap_or(0);
            let forman_curvature = incident_forman_curvature(&grammar_nodes, &adjacency);
            let presentation_priority = presentation_priority(
                opportunity.kind,
                first_value_distance,
                local_degree,
                forman_curvature,
            );
            GeometryShadowSignal {
                opportunity_index,
                grammar_nodes,
                first_value_distance,
                local_degree,
                forman_curvature,
                presentation_priority,
            }
        })
        .collect::<Vec<_>>();
    signals.sort_by_key(|signal| signal.opportunity_index);

    let by_index = signals
        .iter()
        .map(|signal| (signal.opportunity_index, signal))
        .collect::<BTreeMap<_, _>>();
    let mut ordered_opportunity_indices = (0..membrane.opportunities.len()).collect::<Vec<_>>();
    ordered_opportunity_indices.sort_by_key(|index| {
        let opportunity = &membrane.opportunities[*index];
        let priority = by_index
            .get(index)
            .map_or(0, |signal| signal.presentation_priority);
        (
            gate_rank(opportunity.gate),
            Reverse(priority),
            opportunity_key(opportunity),
            *index,
        )
    });

    GeometryShadowRanking {
        enabled: true,
        coverage,
        ordered_opportunity_indices,
        signals,
        membrane_semantic_digest: membrane_digest,
    }
}

fn canonical_order(membrane: &ClaimCapabilityMembrane) -> Vec<usize> {
    let mut indices = (0..membrane.opportunities.len()).collect::<Vec<_>>();
    indices.sort_by_key(|index| {
        let opportunity = &membrane.opportunities[*index];
        (
            gate_rank(opportunity.gate),
            opportunity_key(opportunity),
            *index,
        )
    });
    indices
}

const fn is_first_value(predicate: GrammarPredicate) -> bool {
    matches!(
        predicate,
        GrammarPredicate::DefinesIdentity
            | GrammarPredicate::PerformsOperation
            | GrammarPredicate::ProducesOutcome
            | GrammarPredicate::DescribesFirstResult
    )
}

fn distances_from(
    start: GrammarNodeId,
    adjacency: &BTreeMap<GrammarNodeId, BTreeSet<GrammarNodeId>>,
) -> BTreeMap<GrammarNodeId, usize> {
    let mut distances = BTreeMap::from([(start, 0)]);
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        let next_distance = distances[&current] + 1;
        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if !distances.contains_key(neighbor) {
                    distances.insert(*neighbor, next_distance);
                    queue.push_back(*neighbor);
                }
            }
        }
    }
    distances
}

fn incident_forman_curvature(
    nodes: &[GrammarNodeId],
    adjacency: &BTreeMap<GrammarNodeId, BTreeSet<GrammarNodeId>>,
) -> i32 {
    let node_set = nodes.iter().copied().collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::new();
    for node in nodes {
        if let Some(neighbors) = adjacency.get(node) {
            for neighbor in neighbors {
                let edge = if node <= neighbor {
                    (*node, *neighbor)
                } else {
                    (*neighbor, *node)
                };
                if node_set.contains(node) || node_set.contains(neighbor) {
                    edges.insert(edge);
                }
            }
        }
    }
    let curvature = edges.into_iter().fold(0_i64, |sum, (left, right)| {
        let left_degree = adjacency
            .get(&left)
            .map_or(0_i64, |items| items.len() as i64);
        let right_degree = adjacency
            .get(&right)
            .map_or(0_i64, |items| items.len() as i64);
        sum + 4 - left_degree - right_degree
    });
    curvature.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn presentation_priority(
    kind: UnderclaimOpportunityKind,
    first_value_distance: Option<usize>,
    local_degree: usize,
    forman_curvature: i32,
) -> i32 {
    let kind_weight: i32 = match kind {
        UnderclaimOpportunityKind::BuriedPrimaryValue => 600,
        UnderclaimOpportunityKind::FirstValueDisconnect => 550,
        UnderclaimOpportunityKind::CapabilityOutcomeDisconnect => 500,
        UnderclaimOpportunityKind::EvidenceDisconnect => 450,
        UnderclaimOpportunityKind::FragmentedValue => 400,
        UnderclaimOpportunityKind::QualifierDominance => 350,
        UnderclaimOpportunityKind::JargonOcclusion => 300,
        UnderclaimOpportunityKind::GenericVerbCollapse => 250,
        UnderclaimOpportunityKind::TranslationDivergence => 200,
        UnderclaimOpportunityKind::MissingSupportedValue
        | UnderclaimOpportunityKind::ModeBelowFloor
        | UnderclaimOpportunityKind::UnexpressedConstraintBackedAdvantage => 100,
    };
    let distance = first_value_distance.unwrap_or(0).min(i32::MAX as usize) as i32;
    let degree = local_degree.min(i32::MAX as usize) as i32;
    let bottleneck = forman_curvature.saturating_neg().max(0);
    kind_weight
        .saturating_add(distance.saturating_mul(10))
        .saturating_add(degree.min(100))
        .saturating_add(bottleneck.min(100))
}

#[cfg(test)]
mod tests {
    use super::*;
    use seiri_core::{
        AnswerState, ClaimCapabilityMembrane, ClaimModality, DocumentLanguage, GateKind,
        GrammarEdge, GrammarNode, NarrativeRelation, ReadmeGrammarIR, SourceSpan,
        UnderclaimOpportunity, ValueDimension,
    };
    use std::num::NonZeroU32;

    #[test]
    fn geometry_changes_presentation_order_without_changing_membrane() {
        let grammar = chain_grammar();
        let membrane = membrane();
        let before = membrane.clone();

        let disabled =
            rank_appeal_presentation(&grammar, &membrane, GeometryShadowOptions::disabled());
        let enabled =
            rank_appeal_presentation(&grammar, &membrane, GeometryShadowOptions::default());

        assert_eq!(membrane, before);
        assert_eq!(
            disabled.membrane_semantic_digest,
            enabled.membrane_semantic_digest
        );
        assert_eq!(disabled.ordered_opportunity_indices, vec![0, 1]);
        assert_eq!(enabled.ordered_opportunity_indices, vec![1, 0]);
        assert!(disabled.signals.is_empty());
        assert_eq!(enabled.signals.len(), 2);
    }

    #[test]
    fn geometry_limits_are_partial_without_changing_membrane_digest() {
        let grammar = chain_grammar();
        let membrane = membrane();
        let full = rank_appeal_presentation(&grammar, &membrane, GeometryShadowOptions::default());
        let bounded = rank_appeal_presentation(
            &grammar,
            &membrane,
            GeometryShadowOptions {
                enabled: true,
                max_nodes: 2,
                max_edges: 1,
            },
        );

        assert_eq!(
            bounded.coverage,
            CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
        );
        assert_eq!(
            full.membrane_semantic_digest,
            bounded.membrane_semantic_digest
        );
    }

    fn chain_grammar() -> ReadmeGrammarIR {
        let nodes = [
            GrammarPredicate::DefinesIdentity,
            GrammarPredicate::TargetsAudience,
            GrammarPredicate::StatesProblem,
            GrammarPredicate::ProducesOutcome,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, predicate)| GrammarNode {
            id: id(index + 1),
            predicate,
            state: AnswerState::Explicit,
            language: DocumentLanguage::English,
            modality: ClaimModality::Asserted,
            negated: false,
            span: Some(SourceSpan::new(index + 1, 1, index * 10, index * 10 + 5)),
        })
        .collect::<Vec<_>>();
        let edges = (1..4)
            .map(|index| GrammarEdge {
                from: id(index),
                to: id(index + 1),
                relation: NarrativeRelation::Precedes,
            })
            .collect();
        ReadmeGrammarIR::try_new(
            "README.md",
            CoverageStatus::Complete,
            nodes,
            edges,
            Vec::new(),
        )
        .expect("valid grammar")
    }

    fn membrane() -> ClaimCapabilityMembrane {
        ClaimCapabilityMembrane {
            opportunities: vec![
                UnderclaimOpportunity {
                    kind: UnderclaimOpportunityKind::QualifierDominance,
                    dimension: ValueDimension::Audience,
                    gate: GateKind::Safe,
                    grammar_nodes: vec![id(2)],
                    capability_nodes: Vec::new(),
                },
                UnderclaimOpportunity {
                    kind: UnderclaimOpportunityKind::BuriedPrimaryValue,
                    dimension: ValueDimension::Outcome,
                    gate: GateKind::Safe,
                    grammar_nodes: vec![id(4)],
                    capability_nodes: Vec::new(),
                },
            ],
            ..ClaimCapabilityMembrane::default()
        }
    }

    fn id(value: usize) -> GrammarNodeId {
        GrammarNodeId::new(NonZeroU32::new(value as u32).expect("non-zero"))
    }
}
