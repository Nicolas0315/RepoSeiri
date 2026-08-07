use seiri_core::{
    AnswerState, CoverageIncompleteReason, CoverageStatus, DocumentEvent, DocumentLanguage,
    DocumentScan, GrammarDiagnostic, GrammarDiagnosticKind, GrammarEdge, GrammarNode,
    GrammarNodeId, GrammarPredicate, NarrativeRelation, ReadmeGrammarIR, SourceSpan,
};
use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadmeGrammarOptions {
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for ReadmeGrammarOptions {
    fn default() -> Self {
        Self {
            max_nodes: 4_096,
            max_edges: 8_192,
        }
    }
}

pub fn analyze_readme_grammar(
    document: &DocumentScan,
    options: &ReadmeGrammarOptions,
) -> Result<ReadmeGrammarIR, seiri_core::AppealIrError> {
    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut truncated = false;
    for event in document.events() {
        let Some((text, span)) = grammar_text(event) else {
            continue;
        };
        let language = language_of(text);
        let normalized = normalize_for_matching(text);
        let modality = modality_of(&normalized);
        let negated = is_negated(&normalized);
        for predicate in predicates_for(&normalized) {
            if nodes.len() >= options.max_nodes || nodes.len() >= u32::MAX as usize {
                truncated = true;
                break;
            }
            let id = NonZeroU32::new((nodes.len() + 1) as u32)
                .map(GrammarNodeId::new)
                .expect("node count was checked");
            nodes.push(GrammarNode {
                id,
                predicate,
                state: AnswerState::Explicit,
                language,
                modality,
                negated,
                span: Some(span),
            });
        }
        if truncated {
            break;
        }
    }

    if truncated {
        diagnostics.push(GrammarDiagnostic {
            kind: GrammarDiagnosticKind::NodeLimitExceeded,
            span: None,
        });
    }
    let edges = build_edges(&nodes, options.max_edges, &mut diagnostics);
    let coverage = if truncated
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == GrammarDiagnosticKind::NodeLimitExceeded)
    {
        CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
    } else {
        CoverageStatus::Complete
    };
    ReadmeGrammarIR::try_new(document.path(), coverage, nodes, edges, diagnostics)
}

fn grammar_text(event: &DocumentEvent) -> Option<(&str, SourceSpan)> {
    match event {
        DocumentEvent::VisibleProse(prose) => Some((&prose.text, prose.span)),
        DocumentEvent::Heading(heading) => heading.span.map(|span| (heading.text.as_str(), span)),
        DocumentEvent::Link(_) | DocumentEvent::Badge(_) | DocumentEvent::RouteCandidate(_) => None,
    }
}

fn language_of(text: &str) -> DocumentLanguage {
    if text.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        )
    }) {
        DocumentLanguage::Japanese
    } else {
        DocumentLanguage::English
    }
}

fn normalize_for_matching(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() || !character.is_ascii() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn predicates_for(text: &str) -> Vec<GrammarPredicate> {
    let mut predicates = Vec::new();
    push_if(
        &mut predicates,
        GrammarPredicate::DefinesIdentity,
        contains_any(
            text,
            &[
                " is a ",
                " is an ",
                " tool",
                " cli",
                " plugin",
                "ライブラリ",
                "ツール",
                "プラグイン",
                "です",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::TargetsAudience,
        contains_any(
            text,
            &[
                " for developers",
                " for teams",
                " for maintainers",
                "designed for",
                "向け",
                "開発者",
                "利用者",
                "メンテナ",
                "チーム",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::StatesProblem,
        contains_any(
            text,
            &[
                "problem", "pain", "avoid", "reduce", "prevent", "課題", "問題", "防ぐ", "減ら",
                "迷わ",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::PerformsOperation,
        contains_any(
            text,
            &[
                "analy", "scan", "generate", "detect", "organize", "audit", "inspect", "review",
                "解析", "検出", "生成", "整理", "監査", "調べ", "確認",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::ProducesOutcome,
        contains_any(
            text,
            &[
                "so that",
                "allows",
                "enables",
                "result",
                "output",
                "returns",
                "produces",
                "できる",
                "できます",
                "得られ",
                "結果",
                "出力",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::DescribesFirstAction,
        contains_any(
            text,
            &[
                "quickstart",
                "quick start",
                "getting started",
                "install",
                "run ",
                "use ",
                "first step",
                "クイックスタート",
                "開始",
                "インストール",
                "実行",
                "使い方",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::DescribesFirstResult,
        contains_any(
            text,
            &[
                "you should see",
                "prints",
                "returns",
                "first result",
                "expected output",
                "表示され",
                "得られ",
                "期待する出力",
                "実行結果",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::ProvidesEvidence,
        contains_any(
            text,
            &[
                "test",
                " ci ",
                "benchmark",
                "example",
                "verified",
                "テスト",
                "検証",
                "実例",
                "ベンチマーク",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::StatesConstraint,
        contains_any(
            text,
            &[
                "does not",
                "never ",
                "only ",
                "limited",
                "requires",
                "no network",
                "しません",
                "しない",
                "のみ",
                "制約",
                "上限",
                "必要",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::StatesDifferentiation,
        contains_any(
            text,
            &[
                "unlike",
                "instead of",
                "single session",
                "without ",
                "一つの",
                "単一",
                "一度に",
                "なしで",
                "異な",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::RequestsAction,
        contains_any(
            text,
            &[
                "try ",
                "start ",
                "run ",
                "see ",
                "試す",
                "始め",
                "実行して",
                "参照して",
            ],
        ),
    );
    predicates.sort_unstable();
    predicates.dedup();
    predicates
}

fn push_if(values: &mut Vec<GrammarPredicate>, value: GrammarPredicate, condition: bool) {
    if condition {
        values.push(value);
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let padded = format!(" {value} ");
    needles.iter().any(|needle| padded.contains(needle))
}

fn modality_of(text: &str) -> seiri_core::ClaimModality {
    if is_negated(text) {
        seiri_core::ClaimModality::Prohibited
    } else if contains_any(text, &["must", "requires", "required", "必要", "必須"]) {
        seiri_core::ClaimModality::Required
    } else if contains_any(text, &["may", "might", "possible", "可能", "場合があり"]) {
        seiri_core::ClaimModality::Possible
    } else if contains_any(
        text,
        &[
            "only", "within", "bounded", "limited", "のみ", "範囲", "上限",
        ],
    ) {
        seiri_core::ClaimModality::Qualified
    } else {
        seiri_core::ClaimModality::Asserted
    }
}

fn is_negated(text: &str) -> bool {
    contains_any(
        text,
        &[
            "does not",
            "do not",
            "cannot",
            "never",
            "no network",
            "しません",
            "しない",
            "できません",
            "禁止",
        ],
    )
}

fn build_edges(
    nodes: &[GrammarNode],
    max_edges: usize,
    diagnostics: &mut Vec<GrammarDiagnostic>,
) -> Vec<GrammarEdge> {
    let mut candidates = Vec::new();
    for pair in nodes.windows(2) {
        candidates.push(GrammarEdge {
            from: pair[0].id,
            to: pair[1].id,
            relation: NarrativeRelation::Precedes,
        });
    }
    for (index, node) in nodes.iter().enumerate() {
        if node.predicate == GrammarPredicate::ProvidesEvidence {
            if let Some(subject) = nodes[..index].iter().rev().find(|candidate| {
                matches!(
                    candidate.predicate,
                    GrammarPredicate::PerformsOperation | GrammarPredicate::ProducesOutcome
                )
            }) {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: subject.id,
                    relation: NarrativeRelation::Supports,
                });
            }
        }
        if node.predicate == GrammarPredicate::StatesConstraint {
            if let Some(subject) = nodes[..index].iter().rev().find(|candidate| {
                matches!(
                    candidate.predicate,
                    GrammarPredicate::PerformsOperation | GrammarPredicate::ProducesOutcome
                )
            }) {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: subject.id,
                    relation: NarrativeRelation::Qualifies,
                });
            }
        }
        if node.predicate == GrammarPredicate::StatesProblem {
            if let Some(capability) = nodes[index + 1..]
                .iter()
                .find(|candidate| candidate.predicate == GrammarPredicate::PerformsOperation)
            {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: capability.id,
                    relation: NarrativeRelation::Explains,
                });
            }
        }
        if let Some(translation) = nodes[index + 1..].iter().find(|candidate| {
            candidate.predicate == node.predicate && candidate.language != node.language
        }) {
            candidates.push(GrammarEdge {
                from: node.id,
                to: translation.id,
                relation: NarrativeRelation::Translates,
            });
        }
    }
    candidates.sort_unstable_by_key(|edge| (edge.from, edge.to, relation_rank(edge.relation)));
    candidates.dedup_by_key(|edge| (edge.from, edge.to, relation_rank(edge.relation)));
    if candidates.len() > max_edges {
        candidates.truncate(max_edges);
        diagnostics.push(GrammarDiagnostic {
            kind: GrammarDiagnosticKind::NodeLimitExceeded,
            span: None,
        });
    }
    candidates
}

const fn relation_rank(relation: NarrativeRelation) -> u8 {
    match relation {
        NarrativeRelation::Precedes => 0,
        NarrativeRelation::Supports => 1,
        NarrativeRelation::Explains => 2,
        NarrativeRelation::Qualifies => 3,
        NarrativeRelation::Translates => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_document;

    #[test]
    fn grammar_uses_visible_prose_and_excludes_code_fences() {
        let scan = scan_document(
            "README.md",
            "# Tool\nA tool for maintainers that scans repositories and produces a report.\n```text\nThis benchmark proves quality.\n```\n",
        )
        .expect("scan");
        let grammar =
            analyze_readme_grammar(&scan, &ReadmeGrammarOptions::default()).expect("grammar");
        assert!(grammar
            .nodes
            .iter()
            .any(|node| node.predicate == GrammarPredicate::TargetsAudience));
        assert!(grammar
            .nodes
            .iter()
            .any(|node| node.predicate == GrammarPredicate::PerformsOperation));
        assert!(!grammar
            .nodes
            .iter()
            .any(|node| node.predicate == GrammarPredicate::ProvidesEvidence));
    }

    #[test]
    fn grammar_preserves_japanese_constraint_and_negation() {
        let scan = scan_document(
            "README.md",
            "# ツール\nこのツールはリポジトリを解析します。ネットワーク通信はしません。\n",
        )
        .expect("scan");
        let grammar =
            analyze_readme_grammar(&scan, &ReadmeGrammarOptions::default()).expect("grammar");
        let constraint = grammar
            .nodes
            .iter()
            .find(|node| node.predicate == GrammarPredicate::StatesConstraint)
            .expect("constraint");
        assert_eq!(constraint.language, DocumentLanguage::Japanese);
        assert!(constraint.negated);
        assert_eq!(constraint.modality, seiri_core::ClaimModality::Prohibited);
    }

    #[test]
    fn grammar_reports_node_budget_without_collapsing_to_absence() {
        let scan = scan_document(
            "README.md",
            "A tool for teams that scans repositories and produces output.",
        )
        .expect("scan");
        let grammar = analyze_readme_grammar(
            &scan,
            &ReadmeGrammarOptions {
                max_nodes: 1,
                max_edges: 1,
            },
        )
        .expect("grammar");
        assert_eq!(
            grammar.coverage,
            CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
        );
        assert_eq!(grammar.nodes.len(), 1);
    }
}
