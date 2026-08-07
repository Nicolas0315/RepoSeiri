# Migration v5

## 日本語

R12は、公開wire名`seiri.analysis.v2`、`seiri.patch-plan.v2`、`seiri.codex.v2`を維持したまま、READMEの意味解析、repository capability、主張のfloor/ceiling、過小主張のdry-run提案を追加します。同じwire名の意味変更を黙って受理させないため、machine contractを`seiri.contract.v5`へ上げ、semantic revision集合を22個から27個へ拡張しました。

| 項目 | 旧revision | 現行revision |
| --- | --- | --- |
| source session | `seiri.source-session.v2` | `seiri.source-session.v3` |
| stable digest | `seiri.stable-digest.v3` | `seiri.stable-digest.v4` |
| calibration | `seiri.calibration-semantics.v4` | `seiri.calibration-semantics.v5` |
| patch planner | `seiri.patch-planner.v5` | `seiri.patch-planner.v7` |
| completion | `seiri.completion-semantics.v5` | `seiri.completion-semantics.v6` |

新しい必須revision keyは`readme_grammar`、`program_capability`、`narrative_topology`、`value_coverage`、`claim_capability_membrane`です。launcherは全27 keyの名前、値、個数を検証し、旧contractや部分集合へfallbackしません。

`seiri.analysis.v2`には`readme_grammar`、`repository_capabilities`、`value_coverage`、`claim_capability_membrane`が必須fieldとして加わります。analysis budgetにもprogram file数、総byte数、1 file byte数、node数、edge数の上限が加わります。consumerはUnknownをMissingへ変換せず、`coverage.kind=partial`と理由を保持してください。

`ClaimCapabilityMembrane`はREADMEを派手にする生成器ではありません。programで観測された能力だけがceilingを形成し、testの存在はevidenceを支えても品質、runtime成功、信頼、security、performanceを支えません。`MissingSupportedValue`はsupport relationが`Supported`の場合だけ生成されます。観測不足、unsupported syntax、budget超過はUnknownのままです。

`seiri.patch-plan.v2`には`appeal_suggestions`と`appeal_presentation`が加わります。`Safe`は既存README意味の配置・接続だけ、`Guarded`は`Supported`かつ非`Omitted` ceilingの場合だけ、その他は`Manual`です。suggestionはsource-session digest、grammar/capability anchor、support state、ceilingへ結合されますが、README文を生成せず、fileを書きません。`appeal_presentation`の距離・曲率は同一gate内の表示順だけに使われ、証拠、support、ceiling、opportunity、risk、主張意味を変更しません。

public synthetic holdoutはroute、wording、consistency、profile、plannerにappeal taskを加えた6 taskになりました。appeal holdoutは日英の過小主張、能力表明済み、unsupported syntax/Unknownを分けます。各taskのholdoutは4件で最低20件未満のため、完全一致しても`insufficient_sample`です。一般性能や実利用repositoryでの精度を意味しません。

Codex queryは従来の10種類のまま、schema identifierも`seiri.codex.v2`のままです。consumerはquery enumを増やさず、`summary`の追加count、`governance`の意味IR、`patches`内のappeal suggestionを新contractの下で処理してください。

---

## English

R12 adds README semantic analysis, repository capabilities, claim floors and ceilings, and dry-run underclaim suggestions while retaining the public wire names `seiri.analysis.v2`, `seiri.patch-plan.v2`, and `seiri.codex.v2`. To prevent silent adoption of changed meaning under those names, the machine contract moves to `seiri.contract.v5` and expands the closed semantic-revision set from 22 keys to 27.

| Area | Previous revision | Current revision |
| --- | --- | --- |
| source session | `seiri.source-session.v2` | `seiri.source-session.v3` |
| stable digest | `seiri.stable-digest.v3` | `seiri.stable-digest.v4` |
| calibration | `seiri.calibration-semantics.v4` | `seiri.calibration-semantics.v5` |
| patch planner | `seiri.patch-planner.v5` | `seiri.patch-planner.v7` |
| completion | `seiri.completion-semantics.v5` | `seiri.completion-semantics.v6` |

The new required revision keys are `readme_grammar`, `program_capability`, `narrative_topology`, `value_coverage`, and `claim_capability_membrane`. Launchers validate the names, values, and count of all 27 keys and do not fall back to an older contract or a subset.

`seiri.analysis.v2` now requires `readme_grammar`, `repository_capabilities`, `value_coverage`, and `claim_capability_membrane`. Analysis budgets also gain limits for program files, total bytes, per-file bytes, nodes, and edges. Consumers must preserve `coverage.kind=partial` and its reason instead of converting Unknown into Missing.

`ClaimCapabilityMembrane` is not a louder-README generator. Only observed program capability forms a ceiling. Tests may support the evidence dimension but do not establish quality, runtime success, trust, security, or performance. `MissingSupportedValue` is emitted only for a `Supported` relation. Incomplete observation, unsupported syntax, and exhausted budgets remain Unknown.

`seiri.patch-plan.v2` now includes `appeal_suggestions` and `appeal_presentation`. `Safe` is limited to positioning and connecting existing README meaning. `Guarded` requires both `Supported` and a non-`Omitted` ceiling. Every other case is `Manual`. Suggestions bind to a source-session digest, grammar/capability anchors, support state, and ceiling, but generate no README prose and write no files. Presentation distance and curvature may reorder items only within the same final gate; they do not change evidence, support, ceilings, opportunities, risks, or claim semantics.

The public synthetic holdout now has six tasks: route, wording, consistency, profile, planner, and appeal. Appeal cases separate Japanese and English underclaim, already expressed capability, and unsupported-syntax/Unknown behavior. Each task still has only four holdout cases, below the minimum of 20, so an exact result remains `insufficient_sample`. It is not evidence of general performance or accuracy on real repositories.

The Codex query set remains exactly ten kinds and the schema identifier remains `seiri.codex.v2`. Consumers should not add a query enum value; under the new contract they should accept the added summary counts, governance semantic IR, and appeal suggestions inside `patches`.
