use crate::{
    analyze_readme_grammar_with_source, scan_document_with_options, DocumentScanOptions,
    MarkdownError, ReadmeGrammarOptions,
};
use seiri_core::{
    AnswerState, AppealIrError, ClaimAtom, ClaimDraft, ClaimDraftIR, ClaimDraftId,
    ClaimDraftReauditObservation, ClaimDraftReauditResult, ClaimDraftSemantics, ClaimMode,
    ClaimRealization, CoverageStatus, DocumentScan, GrammarNode, ReadmeGrammarIR, TextDocumentBase,
};
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ClaimDraftCandidateReauditError {
    Markdown(MarkdownError),
    Appeal(AppealIrError),
}

impl Display for ClaimDraftCandidateReauditError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(error) => write!(formatter, "candidate README scan failed: {error}"),
            Self::Appeal(error) => write!(formatter, "candidate README audit failed: {error}"),
        }
    }
}

impl std::error::Error for ClaimDraftCandidateReauditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Markdown(error) => Some(error),
            Self::Appeal(error) => Some(error),
        }
    }
}

impl From<MarkdownError> for ClaimDraftCandidateReauditError {
    fn from(error: MarkdownError) -> Self {
        Self::Markdown(error)
    }
}

impl From<AppealIrError> for ClaimDraftCandidateReauditError {
    fn from(error: AppealIrError) -> Self {
        Self::Appeal(error)
    }
}

/// Counts typed Unknown-bearing grammar facts on one stable scale.
///
/// Coverage contributes one when it is incomplete, each Unknown node and
/// grammar diagnostic contributes one, and the translation IR contributes its
/// canonical (sorted and deduplicated) reason count. The categories are
/// intentionally additive: a diagnostic and the partial coverage it caused are
/// distinct facts, while duplicate translation reasons have already been
/// removed by the core IR constructor.
#[must_use]
pub fn readme_grammar_unknown_count(grammar: &ReadmeGrammarIR) -> usize {
    let coverage = usize::from(grammar.coverage != CoverageStatus::Complete);
    let nodes = grammar
        .nodes
        .iter()
        .filter(|node| matches!(node.state, AnswerState::Unknown(_)))
        .count();
    coverage
        .saturating_add(nodes)
        .saturating_add(grammar.diagnostics.len())
        .saturating_add(grammar.translation_alignment.unknown_reasons.len())
}

/// Reparses one candidate README entirely in memory and evaluates it against a
/// source-bound, prose-free claim draft.
///
/// The observed source digest and byte length are checked before invoking the
/// candidate scanner. A stale source therefore returns the core stale hold even
/// when the candidate itself would exceed the supplied scan budget.
#[allow(clippy::too_many_arguments)]
pub fn reaudit_claim_draft_candidate(
    drafts: &ClaimDraftIR,
    draft_id: ClaimDraftId,
    observed_document: &DocumentScan,
    candidate_path: &str,
    candidate_source: &str,
    scan_options: &DocumentScanOptions,
    grammar_options: &ReadmeGrammarOptions,
) -> Result<ClaimDraftReauditResult, ClaimDraftCandidateReauditError> {
    let draft = drafts
        .draft(draft_id)
        .ok_or(AppealIrError::DanglingClaimDraft(draft_id))?;
    let observed_base = observed_document.base();
    let candidate_base = TextDocumentBase::from_bytes(candidate_source.as_bytes());
    let scope_preserved = observed_document.path() == drafts.path && candidate_path == drafts.path;

    if observed_base.digest() != drafts.source_digest
        || observed_base.byte_len() != drafts.source_byte_len
    {
        let observation = ClaimDraftReauditObservation::new(
            observed_base.digest(),
            observed_base.byte_len(),
            candidate_path,
            candidate_base.digest(),
            candidate_base.byte_len(),
            drafts.baseline_unknown_count,
            Some(draft.semantics.clone()),
            draft.realization_mode,
            scope_preserved,
        );
        return ClaimDraftReauditResult::evaluate(drafts, draft_id, observation)
            .map_err(Into::into);
    }

    let candidate_document =
        match scan_document_with_options(candidate_path, candidate_source, scan_options) {
            Ok(document) => document,
            Err(error) if is_bounded_candidate_unknown(&error) => {
                let observation = ClaimDraftReauditObservation::new(
                    observed_base.digest(),
                    observed_base.byte_len(),
                    candidate_path,
                    candidate_base.digest(),
                    candidate_base.byte_len(),
                    drafts.baseline_unknown_count.saturating_add(1),
                    None,
                    ClaimMode::Omitted,
                    scope_preserved,
                );
                return ClaimDraftReauditResult::evaluate(drafts, draft_id, observation)
                    .map_err(Into::into);
            }
            Err(error) => return Err(error.into()),
        };
    let grammar =
        analyze_readme_grammar_with_source(&candidate_document, candidate_source, grammar_options)?;
    let mut candidate_unknown_count = readme_grammar_unknown_count(&grammar);
    let (candidate_semantics, candidate_mode, association_unknown) =
        candidate_claim_observation(draft, &grammar)?;
    candidate_unknown_count = candidate_unknown_count.saturating_add(association_unknown);
    let observation = ClaimDraftReauditObservation::new(
        observed_base.digest(),
        observed_base.byte_len(),
        candidate_document.path(),
        candidate_base.digest(),
        candidate_base.byte_len(),
        candidate_unknown_count,
        candidate_semantics,
        candidate_mode,
        scope_preserved,
    );
    ClaimDraftReauditResult::evaluate(drafts, draft_id, observation).map_err(Into::into)
}

fn is_bounded_candidate_unknown(error: &MarkdownError) -> bool {
    matches!(
        error,
        MarkdownError::SourceLimitExceeded { .. }
            | MarkdownError::EventLimitExceeded { .. }
            | MarkdownError::DiagnosticLimitExceeded { .. }
    )
}

fn candidate_claim_observation(
    draft: &ClaimDraft,
    grammar: &ReadmeGrammarIR,
) -> Result<(Option<ClaimDraftSemantics>, ClaimMode, usize), AppealIrError> {
    let mut candidates = Vec::new();
    let mut unrepresentable = 0usize;
    for atom in grammar.claim_atoms.atoms.iter().filter(|atom| {
        atom.dimension == draft.semantics.dimension
            && atom.subject == draft.semantics.subject
            && atom.action == draft.semantics.action
            && atom.object == draft.semantics.object
            && atom.language == draft.semantics.language
    }) {
        let Some(node) = grammar
            .nodes
            .iter()
            .find(|node| node.id == atom.grammar_node)
        else {
            return Err(AppealIrError::DanglingClaimAtomGrammarNode(atom.id));
        };
        match semantics_and_mode(atom, node) {
            Ok(candidate) => candidates.push(candidate),
            Err(AppealIrError::NonCanonicalClaimDraftSemantics) => {
                unrepresentable = unrepresentable.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }

    if candidates.is_empty() {
        return Ok((None, ClaimMode::Omitted, unrepresentable));
    }

    let mut distinct = Vec::<ClaimDraftSemantics>::new();
    for (semantics, _) in &candidates {
        if !distinct.contains(semantics) {
            distinct.push(semantics.clone());
        }
    }
    if distinct.len() != 1 {
        return Ok((None, ClaimMode::Omitted, unrepresentable.saturating_add(1)));
    }
    let semantics = distinct.pop().expect("one distinct candidate semantics");
    let mode = candidates
        .iter()
        .filter(|(candidate, _)| candidate == &semantics)
        .map(|(_, mode)| *mode)
        .max()
        .unwrap_or(ClaimMode::Omitted);
    Ok((Some(semantics), mode, unrepresentable))
}

fn semantics_and_mode(
    atom: &ClaimAtom,
    node: &GrammarNode,
) -> Result<(ClaimDraftSemantics, ClaimMode), AppealIrError> {
    let semantics = ClaimDraftSemantics::try_new(
        atom.dimension,
        atom.subject.clone(),
        atom.action.clone(),
        atom.object.clone(),
        atom.qualifiers.clone(),
        atom.condition.clone(),
        atom.polarity,
        atom.modality,
        atom.language,
    )?;
    let mode = ClaimRealization::try_new(atom, node)?.mode;
    Ok((semantics, mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use seiri_core::{
        CapabilityNodeId, ClaimDraftReauditDecision, ClaimDraftReauditHoldReason, SourceSpan,
    };
    use std::num::NonZeroU32;

    const SOURCE: &str = "# RepoSeiri\n\nRepoSeiri audits repositories locally.\n";

    fn fixture() -> (DocumentScan, ClaimDraftIR, ClaimDraftId) {
        let scan_options = DocumentScanOptions::derived_for_source(SOURCE.len());
        let document = scan_document_with_options("README.md", SOURCE, &scan_options).unwrap();
        let grammar =
            analyze_readme_grammar_with_source(&document, SOURCE, &ReadmeGrammarOptions::default())
                .unwrap();
        let atom = grammar
            .claim_atoms
            .atoms
            .iter()
            .find(|atom| atom.action.as_deref() == Some("audit"))
            .unwrap();
        let node = grammar
            .nodes
            .iter()
            .find(|node| node.id == atom.grammar_node)
            .unwrap();
        let (semantics, realization_mode) = semantics_and_mode(atom, node).unwrap();
        let draft_id = ClaimDraftId::new(NonZeroU32::new(1).unwrap());
        let capability = CapabilityNodeId::new(NonZeroU32::new(1).unwrap());
        let draft = ClaimDraft::try_new(
            draft_id,
            SourceSpan::new(1, 1, 0, SOURCE.len()),
            vec![atom.id],
            vec![capability],
            semantics,
            realization_mode,
            ClaimMode::Direct,
        )
        .unwrap();
        let drafts = ClaimDraftIR::try_new(
            "README.md",
            document.base().digest(),
            document.source_bytes(),
            readme_grammar_unknown_count(&grammar),
            vec![draft],
        )
        .unwrap();
        (document, drafts, draft_id)
    }

    #[test]
    fn unchanged_candidate_is_accepted_without_writes() {
        let (document, drafts, draft_id) = fixture();
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &document,
            "README.md",
            SOURCE,
            &DocumentScanOptions::derived_for_source(SOURCE.len()),
            &ReadmeGrammarOptions::default(),
        )
        .unwrap();

        assert_eq!(result.decision, ClaimDraftReauditDecision::Accepted);
        assert!(result.holds.is_empty());
        assert!(result.candidate_checks_performed);
        assert!(!result.writes_files());
    }

    #[test]
    fn stale_source_holds_before_candidate_budget_is_checked() {
        let (_, drafts, draft_id) = fixture();
        let stale_source = "# Different\n";
        let stale_document = scan_document_with_options(
            "README.md",
            stale_source,
            &DocumentScanOptions::derived_for_source(stale_source.len()),
        )
        .unwrap();
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &stale_document,
            "README.md",
            SOURCE,
            &DocumentScanOptions {
                max_source_bytes: 1,
                max_events: 1,
                max_diagnostics: 1,
            },
            &ReadmeGrammarOptions::default(),
        )
        .unwrap();

        assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
        assert_eq!(
            result.holds,
            vec![ClaimDraftReauditHoldReason::StaleSourceDigest]
        );
        assert!(!result.candidate_checks_performed);
    }

    #[test]
    fn semantic_drift_and_scope_escape_are_typed_holds() {
        let (document, drafts, draft_id) = fixture();
        let candidate = "# RepoSeiri\n\nRepoSeiri audits repositories offline.\n";
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &document,
            "docs/README.md",
            candidate,
            &DocumentScanOptions::derived_for_source(candidate.len()),
            &ReadmeGrammarOptions::default(),
        )
        .unwrap();

        assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
        assert!(result
            .holds
            .contains(&ClaimDraftReauditHoldReason::SemanticDrift));
        assert!(result
            .holds
            .contains(&ClaimDraftReauditHoldReason::ScopeEscape));
    }

    #[test]
    fn unknown_count_uses_additive_canonical_categories() {
        let source = SOURCE;
        let document = scan_document_with_options(
            "README.md",
            source,
            &DocumentScanOptions::derived_for_source(source.len()),
        )
        .unwrap();
        let grammar_options = ReadmeGrammarOptions {
            max_nodes: 0,
            max_edges: 0,
        };
        let grammar =
            analyze_readme_grammar_with_source(&document, source, &grammar_options).unwrap();
        let expected = usize::from(grammar.coverage != CoverageStatus::Complete)
            + grammar
                .nodes
                .iter()
                .filter(|node| matches!(node.state, AnswerState::Unknown(_)))
                .count()
            + grammar.diagnostics.len()
            + grammar.translation_alignment.unknown_reasons.len();

        assert!(expected > 0);
        assert_eq!(readme_grammar_unknown_count(&grammar), expected);
    }

    #[test]
    fn candidate_unknown_increase_is_held() {
        let (document, drafts, draft_id) = fixture();
        let candidate = SOURCE;
        let grammar_options = ReadmeGrammarOptions {
            max_nodes: 0,
            max_edges: 0,
        };
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &document,
            "README.md",
            candidate,
            &DocumentScanOptions::derived_for_source(candidate.len()),
            &grammar_options,
        )
        .unwrap();

        assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
        assert!(result
            .holds
            .contains(&ClaimDraftReauditHoldReason::UnknownIncreased));
    }

    #[test]
    fn candidate_scan_limit_is_projected_to_typed_unknown_hold() {
        let (document, drafts, draft_id) = fixture();
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &document,
            "README.md",
            SOURCE,
            &DocumentScanOptions {
                max_source_bytes: 1,
                max_events: 1,
                max_diagnostics: 1,
            },
            &ReadmeGrammarOptions::default(),
        )
        .unwrap();

        assert!(result.candidate_checks_performed);
        assert!(result
            .holds
            .contains(&ClaimDraftReauditHoldReason::UnknownIncreased));
        assert!(result
            .holds
            .contains(&ClaimDraftReauditHoldReason::SemanticDrift));
    }

    #[test]
    fn inline_formatting_preserves_candidate_semantics() {
        let (document, drafts, draft_id) = fixture();
        let candidate = "# RepoSeiri\n\n**RepoSeiri** `audits` repositories **locally**.\n";
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &document,
            "README.md",
            candidate,
            &DocumentScanOptions::derived_for_source(candidate.len()),
            &ReadmeGrammarOptions::default(),
        )
        .unwrap();

        assert_eq!(result.decision, ClaimDraftReauditDecision::Accepted);
        assert!(result.holds.is_empty());
    }

    #[test]
    fn candidate_digest_and_length_are_bound_without_retaining_body() {
        let (document, drafts, draft_id) = fixture();
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id,
            &document,
            "README.md",
            SOURCE,
            &DocumentScanOptions::derived_for_source(SOURCE.len()),
            &ReadmeGrammarOptions::default(),
        )
        .unwrap();

        assert_eq!(
            result.candidate_source_digest,
            seiri_core::PatchBaseDigest::from_bytes(SOURCE.as_bytes())
        );
        assert_eq!(result.candidate_source_byte_len, SOURCE.len());
        let json = serde_json::to_value(&result).unwrap();
        let object = json.as_object().unwrap();
        assert!(!object.contains_key("candidate_source"));
        assert!(!object.contains_key("candidate_source_body"));
        assert!(!object.contains_key("prose"));
        assert!(!json
            .to_string()
            .contains("RepoSeiri audits repositories locally."));
    }
}
