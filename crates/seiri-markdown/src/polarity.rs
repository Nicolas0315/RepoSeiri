use seiri_core::GrammarPredicate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolarityAssessment {
    Positive,
    Negative,
    Ambiguous,
}

/// Assesses polarity at the predicate occurrence rather than promoting one
/// substring match to an entire paragraph or clause.
#[must_use]
pub fn assess_predicate_polarity(text: &str, predicate: GrammarPredicate) -> PolarityAssessment {
    let segments = scope_segments(text);
    let anchors = predicate_anchors(predicate);
    let matched = segments
        .iter()
        .copied()
        .filter(|segment| {
            anchors
                .iter()
                .any(|anchor| contains_anchor(segment, anchor))
        })
        .collect::<Vec<_>>();
    let candidates = if matched.is_empty() {
        if predicate == GrammarPredicate::StatesConstraint {
            segments
                .iter()
                .copied()
                .filter(|segment| has_explicit_negative_cue(segment))
                .collect::<Vec<_>>()
        } else {
            vec![text]
        }
    } else {
        matched
    };
    let mut polarity = candidates
        .into_iter()
        .map(|segment| {
            if has_explicit_negative_cue(segment) {
                PolarityAssessment::Negative
            } else {
                PolarityAssessment::Positive
            }
        })
        .collect::<Vec<_>>();
    polarity.sort_unstable_by_key(|value| match value {
        PolarityAssessment::Positive => 0,
        PolarityAssessment::Negative => 1,
        PolarityAssessment::Ambiguous => 2,
    });
    polarity.dedup();
    match polarity.as_slice() {
        [only] => *only,
        [] => PolarityAssessment::Positive,
        _ => PolarityAssessment::Ambiguous,
    }
}

#[must_use]
pub(crate) fn has_explicit_negative_cue(text: &str) -> bool {
    let words = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let english = words
        .iter()
        .any(|word| *word == "cannot" || *word == "never")
        || words.windows(2).any(|pair| {
            matches!(pair, ["does" | "do" | "did" | "is" | "are" | "will", "not"])
                || matches!(pair, ["no", "network" | "write" | "writes"])
        });
    let not_only = words.windows(2).any(|pair| matches!(pair, ["not", "only"]));
    let japanese = [
        "しない",
        "しません",
        "できない",
        "できません",
        "行わない",
        "行いません",
        "使わない",
        "使用しない",
        "接続しない",
        "書き込まない",
        "書き込みません",
        "生成しない",
        "変更しない",
        "禁止",
    ]
    .iter()
    .any(|cue| text.contains(cue));
    (english && !not_only) || japanese
}

fn scope_segments(text: &str) -> Vec<&str> {
    let mut segments = vec![text];
    for separator in [
        ";",
        " but ",
        " yet ",
        " while ",
        " whereas ",
        " and ",
        "。",
        "；",
        "一方で",
        "ただし",
    ] {
        segments = segments
            .into_iter()
            .flat_map(|segment| segment.split(separator))
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect();
    }
    segments
}

fn contains_anchor(segment: &str, anchor: &str) -> bool {
    if anchor.is_ascii() {
        segment
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|word| word == anchor)
    } else {
        segment.contains(anchor)
    }
}

fn predicate_anchors(predicate: GrammarPredicate) -> &'static [&'static str] {
    match predicate {
        GrammarPredicate::DefinesIdentity => &["reposeiri", "tool", "library", "repo整理"],
        GrammarPredicate::TargetsAudience => &["for", "teams", "maintainers", "向け", "対象"],
        GrammarPredicate::StatesProblem => &["problem", "hard", "difficult", "課題", "問題"],
        GrammarPredicate::PerformsOperation => &[
            "audit", "audits", "analyze", "analyzes", "scan", "scans", "inspect", "inspects",
            "監査", "分析", "解析", "走査", "検査",
        ],
        GrammarPredicate::ProducesOutcome => &[
            "produce", "produces", "return", "returns", "output", "result", "生成", "出力", "結果",
        ],
        GrammarPredicate::DescribesFirstAction => &[
            "install",
            "run",
            "start",
            "use",
            "インストール",
            "実行",
            "開始",
            "使い方",
        ],
        GrammarPredicate::DescribesFirstResult => &[
            "prints",
            "returns",
            "result",
            "output",
            "表示",
            "実行結果",
            "出力",
        ],
        GrammarPredicate::ProvidesEvidence => &[
            "test",
            "tests",
            "benchmark",
            "example",
            "テスト",
            "検証",
            "実例",
            "ベンチマーク",
        ],
        GrammarPredicate::StatesConstraint => &[
            "network",
            "write",
            "writes",
            "only",
            "limited",
            "constraint",
            "ネットワーク",
            "書き込み",
            "制約",
            "上限",
            "のみ",
        ],
        GrammarPredicate::StatesDifferentiation => &[
            "unlike",
            "instead",
            "different",
            "without",
            "一方",
            "異な",
            "なしで",
        ],
        GrammarPredicate::RequestsAction => {
            &["try", "start", "run", "see", "試す", "始め", "実行", "参照"]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_negation_uses_token_and_predicate_boundaries() {
        assert_eq!(
            assess_predicate_polarity(
                "reposeiri does not use network and audits repositories locally",
                GrammarPredicate::StatesConstraint,
            ),
            PolarityAssessment::Negative
        );
        assert_eq!(
            assess_predicate_polarity(
                "reposeiri does not use network and audits repositories locally",
                GrammarPredicate::PerformsOperation,
            ),
            PolarityAssessment::Positive
        );
        assert!(!has_explicit_negative_cue(
            "nevertheless reposeiri audits repositories"
        ));
    }

    #[test]
    fn japanese_adjective_is_not_a_negative_cue() {
        assert!(!has_explicit_negative_cue(
            "少ない設定でリポジトリを監査します"
        ));
        assert!(has_explicit_negative_cue(
            "ネットワークへ接続しないで監査します"
        ));
    }
}
