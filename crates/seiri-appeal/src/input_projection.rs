use seiri_core::{
    AppealIrError, CapabilityEdge, CapabilityKind, CapabilityNode, CapabilityNodeId,
    CapabilityRelation, CapabilitySemanticSignature, RepositoryCapabilityIR, ValueDimension,
};
use std::collections::{BTreeMap, BTreeSet};

/// A validated, deterministic view of the capability inputs that can affect
/// one membrane dimension.
///
/// The index borrows the validated IR, so it cannot outlive or silently drift
/// from its source. It is the shared input boundary for scalar evaluation and
/// incremental input digests.
#[derive(Debug)]
pub struct CapabilityEvaluationIndex<'a> {
    capabilities: &'a RepositoryCapabilityIR,
    nodes_by_id: BTreeMap<CapabilityNodeId, &'a CapabilityNode>,
    direct_nodes: BTreeMap<ValueDimension, Vec<CapabilityNodeId>>,
    relevant_nodes: BTreeMap<ValueDimension, Vec<CapabilityNodeId>>,
    signatures: BTreeMap<ValueDimension, Vec<&'a CapabilitySemanticSignature>>,
    edges: BTreeMap<ValueDimension, Vec<&'a CapabilityEdge>>,
}

impl<'a> CapabilityEvaluationIndex<'a> {
    pub fn try_new(capabilities: &'a RepositoryCapabilityIR) -> Result<Self, AppealIrError> {
        capabilities.validate()?;
        let nodes_by_id = capabilities
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        let mut direct = ValueDimension::ALL
            .into_iter()
            .map(|dimension| (dimension, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        for node in &capabilities.nodes {
            direct
                .entry(node.kind.dimension())
                .or_default()
                .insert(node.id);
            if node.kind == CapabilityKind::Example
                && capabilities.edges.iter().any(|edge| {
                    edge.relation == CapabilityRelation::DemonstratedBy
                        && (edge.from == node.id || edge.to == node.id)
                })
            {
                direct
                    .entry(ValueDimension::Evidence)
                    .or_default()
                    .insert(node.id);
            }
        }

        let mut signatures = ValueDimension::ALL
            .into_iter()
            .map(|dimension| (dimension, Vec::new()))
            .collect::<BTreeMap<_, _>>();
        let mut relevant = direct.clone();
        for signature in &capabilities.semantic_signatures {
            signatures
                .entry(signature.dimension)
                .or_default()
                .push(signature);
            let ids = relevant.entry(signature.dimension).or_default();
            ids.insert(signature.capability_node);
            ids.extend(signature.input_nodes.iter().copied());
            ids.extend(signature.output_nodes.iter().copied());
            ids.extend(signature.condition_nodes.iter().copied());
        }

        let direct_nodes = direct
            .into_iter()
            .map(|(dimension, ids)| (dimension, ids.into_iter().collect()))
            .collect::<BTreeMap<_, _>>();
        let relevant_nodes = relevant
            .iter()
            .map(|(dimension, ids)| (*dimension, ids.iter().copied().collect()))
            .collect::<BTreeMap<_, _>>();
        let edges = ValueDimension::ALL
            .into_iter()
            .map(|dimension| {
                let ids = relevant
                    .get(&dimension)
                    .expect("all dimensions initialized");
                let mut edges = capabilities
                    .edges
                    .iter()
                    .filter(|edge| ids.contains(&edge.from) || ids.contains(&edge.to))
                    .collect::<Vec<_>>();
                edges.sort_by_key(|edge| (edge.from, edge.to, relation_rank(edge.relation)));
                (dimension, edges)
            })
            .collect::<BTreeMap<_, _>>();

        Ok(Self {
            capabilities,
            nodes_by_id,
            direct_nodes,
            relevant_nodes,
            signatures,
            edges,
        })
    }

    #[must_use]
    pub const fn capabilities(&self) -> &'a RepositoryCapabilityIR {
        self.capabilities
    }

    #[must_use]
    pub fn node(&self, id: CapabilityNodeId) -> Option<&'a CapabilityNode> {
        self.nodes_by_id.get(&id).copied()
    }

    #[must_use]
    pub fn direct_nodes(&self, dimension: ValueDimension) -> Vec<&'a CapabilityNode> {
        self.direct_nodes
            .get(&dimension)
            .into_iter()
            .flatten()
            .filter_map(|id| self.node(*id))
            .collect()
    }

    #[must_use]
    pub fn relevant_nodes(&self, dimension: ValueDimension) -> Vec<&'a CapabilityNode> {
        self.relevant_nodes
            .get(&dimension)
            .into_iter()
            .flatten()
            .filter_map(|id| self.node(*id))
            .collect()
    }

    #[must_use]
    pub fn signatures(&self, dimension: ValueDimension) -> &[&'a CapabilitySemanticSignature] {
        self.signatures
            .get(&dimension)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn edges(&self, dimension: ValueDimension) -> &[&'a CapabilityEdge] {
        self.edges.get(&dimension).map(Vec::as_slice).unwrap_or(&[])
    }
}

pub(crate) const fn relation_rank(relation: CapabilityRelation) -> u8 {
    match relation {
        CapabilityRelation::Offers => 0,
        CapabilityRelation::Accepts => 1,
        CapabilityRelation::Produces => 2,
        CapabilityRelation::DemonstratedBy => 3,
        CapabilityRelation::ConstrainedBy => 4,
        CapabilityRelation::Exposes => 5,
        CapabilityRelation::ConditionedBy => 6,
    }
}
