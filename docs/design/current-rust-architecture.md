# Current Rust Architecture

## 日本語

この文書はRoadmapではなく、Roadmap v13とR13-HF-v1実装後の現行Rust構造を記録します。RepoSeiriは一度のbounded filesystem走査で得たrepository-relative recordとbounded source bytesから、Markdown命題、program capability、GitHubローカル文書、evidence、route、review、dry-run patchを導出します。

### データフロー

1. `seiri-fs`がroot境界、symlink、件数、深さ、サイズbudgetを検査し、repository-relative entryを生成します。
2. `seiri-markdown`が選択文書を一度だけ読み、`SourceStore`とbyte-accurateな`DocumentIndex`を生成します。
3. `seiri-core::SemanticIndex`がvisible Markdown eventを一度正規化し、code fence、inline code、HTML commentを検索対象から除外します。`seiri-markdown`は同じeventからpredicate-local polarity、source-bound claim atom、日英translation alignmentを持つboundedな`ReadmeGrammarIR`を構築します。
4. `seiri-program-local`がfilesystem recordから未読program sourceだけをbudget内で`SourceStore`へ追加し、文字列・commentを長さ保存maskしたlocal Rust logical-item frontendで`RepositoryCapabilityIR`へ投影します。macroやunsupported regionはspan付きの局所Unknownです。
5. `seiri-github-local`は同じ`SourceStore`からYAML、JSON、CODEOWNERSを解析します。
6. `seiri-appeal`がclaim atomとcapability semantic signatureを`ClaimCapabilityMembrane`で命題単位に結合し、floor、claim-specific ceiling、support、Unknown、underclaim、overclaimを分離します。scalarとincrementalは一つの`CapabilityEvaluationIndex`を使い、別のbounded geometry shadowが不変のmembrane上で提示専用graph signalを計算します。
7. `seiri-report`がこれらをevidence、coverage、route axes、content slots、consistency、reviewと一つの`RepositoryAnalysis`へ組み立て、派生evidence参照を検証します。
8. `seiri-planner`は`RepositoryAnalysis`だけを読み、filesystemを再読込せず、既存targetへのsource-bound dry-run edit、typed hold、prose-freeなappeal suggestionとclaim draftを生成します。candidate re-auditはsource binding、Unknown、ceiling、semantic drift、scopeを再検査します。
9. `seiri-codex`はcanonical analysisとplanのborrowed projectionだけを10 queryで表示します。

### 所有境界

| Crate | 所有する責務 | 所有しない責務 |
| --- | --- | --- |
| `seiri-core` | typed state、source store、semantic index、route registry、analysis integrity | filesystem I/O、CLI表示 |
| `seiri-fs` | bounded traversalとrepository-relative path | Markdown意味解析 |
| `seiri-markdown` | bounded document selection、source read、event IR、README grammar IR | program capability |
| `seiri-program-local` | bounded program source追加、manifest/Rust public surfaceのcapability投影 | build実行、runtime成功推定 |
| `seiri-appeal` | value coverage、narrative topology、claim-capability membrane、incremental equivalence、deterministic performance receipt、提示専用geometry shadow | README文生成、evidence ceiling変更、一般性能主張 |
| `seiri-github-local` | bounded GitHub構造文書parser | network GitHub API |
| `seiri-report` | 一回のaudit組立てと派生整合性 | patch write |
| `seiri-planner` | existing-target edit、skeleton/manual分類、stale binding、gated appeal suggestion、prose-free claim draft | filesystem read、file write、policy/宣伝文生成 |
| `seiri-delta` | portable semantic fingerprintと比較 | host absolute pathのidentity化 |
| `seiri-codex` | bounded query projection | command実行、Git/GitHub操作 |

### 低レイヤ不変条件

- source bodyは`Arc<[u8]>`または`Arc<str>`としてsession内で共有し、公開wireへserializeしません。
- source spanはpath、source digest、byte長、UTF-8 char boundary、byte offset、line、Unicode scalar columnへ再結合し、planner bindingはbase digestとanchor contextを再検証します。
- routeのartifact、entrypoint、reachability、freshness、conflict、policyを独立軸で保持し、単一stateは表示用projectionです。
- route slug、日英label、target候補、policy境界は`ROUTE_SPECS`だけが所有します。
- public identityはframed SHA-256とrepository-relative inputから作り、host absolute pathとprivate calibration bodyを含めません。
- `AnalysisCoreView`はcanonical evidence、route、content、reviewを借用し、claim、finding、priorityのevidence参照をaudit完了前に検証します。
- plannerは`seiri-fs`へ依存せず、`SourceStore`と`LanguageTopologyIndex`から日英ペアeditを生成します。
- program parseのbudget超過、invalid UTF-8、unsupported syntaxはMissingへcollapseせず、typedなPartial/Unknownとして膜とplannerまで保持します。
- capability ceilingは観測されたprogram shapeだけから作り、testはEvidence以外のOutcome、品質、信頼、安全性、performanceを昇格させません。
- 増分層はREADME/profile/global coverage変更をscalar rebuildへ戻し、局所program変更だけをdimension frontierで再評価します。validation用scalar比較は別呼び出しであり、performance測定ではありません。
- performance receiptはtarget、environment、corpus、command、sample数、scalar/incremental digestと時間観測を束縛します。semantic receipt digestとobservation digestを分け、時間閾値は適用しません。
- geometry shadowはboundedな無向grammar graphです。距離と非重みForman型曲率は提示signalとしてだけserializeされます。plannerは最終`Safe`/`Guarded`/`Manual` gateを第一キーに保ち、geometry on/offでmembrane digestと、順序・提示metadata以外のsuggestion fieldが不変であることを要求します。

### 境界

この構造は人気、信頼、安全性、品質、法的適合性、公開準備完了を保証しません。standard auditはnetwork、file write、Git、GitHub操作を開始しません。

---

## English

This document records the current Rust structure after Roadmap v13 and R13-HF-v1 implementation; it is not a roadmap. RepoSeiri derives Markdown propositions, program capability, local GitHub documents, evidence, routes, reviews, and dry-run patches from repository-relative records and bounded source bytes obtained by one bounded filesystem traversal.

### Data Flow

1. `seiri-fs` checks root boundaries, symlinks, entry, depth, and size budgets, then emits repository-relative entries.
2. `seiri-markdown` reads selected documents once and builds the `SourceStore` and byte-accurate `DocumentIndex`.
3. `seiri-core::SemanticIndex` normalizes visible Markdown events once and excludes code fences, inline code, and HTML comments from search. `seiri-markdown` builds bounded `ReadmeGrammarIR` with predicate-local polarity, source-bound claim atoms, and bilingual translation alignment from the same events.
4. `seiri-program-local` adds only unread program sources to the `SourceStore` under explicit budgets and projects manifest and Rust logical public items into `RepositoryCapabilityIR` after length-preserving masking of strings and comments. Macros and unsupported regions remain localized span-bound Unknown.
5. `seiri-github-local` parses YAML, JSON, and CODEOWNERS from the same `SourceStore`.
6. `seiri-appeal` aligns claim atoms with capability semantic signatures through `ClaimCapabilityMembrane`, keeping floors, claim-specific ceilings, support, Unknown, underclaim, and overclaim separate. Scalar and incremental paths share one `CapabilityEvaluationIndex`; a separate bounded geometry shadow computes presentation-only graph signals over the immutable membrane.
7. `seiri-report` assembles these with evidence, coverage, route axes, content slots, consistency, and review into one `RepositoryAnalysis`, then validates derived evidence references.
8. `seiri-planner` reads only `RepositoryAnalysis`, performs no filesystem reread, and emits existing-target edits, typed holds, prose-free appeal suggestions, and claim drafts as dry runs. Candidate re-audit rechecks source binding, Unknown growth, ceilings, semantic drift, and scope.
9. `seiri-codex` renders borrowed projections of the canonical analysis and plan through exactly ten queries.

### Ownership Boundaries

| Crate | Owns | Does not own |
| --- | --- | --- |
| `seiri-core` | Typed state, source store, semantic index, route registry, analysis integrity | Filesystem I/O and CLI rendering |
| `seiri-fs` | Bounded traversal and repository-relative paths | Markdown semantics |
| `seiri-markdown` | Bounded document selection, source reads, event IR, README grammar IR | Program capability |
| `seiri-program-local` | Bounded program-source extension and capability projection from manifests/Rust public shapes | Builds and runtime-success inference |
| `seiri-appeal` | Value coverage, narrative topology, claim-capability membrane, incremental equivalence, deterministic performance receipts, presentation-only geometry shadow | README prose generation, evidence-ceiling changes, and general-performance claims |
| `seiri-github-local` | Bounded GitHub structured-document parsers | Network GitHub APIs |
| `seiri-report` | Single audit assembly and derived consistency | Patch writes |
| `seiri-planner` | Existing-target edits, skeleton/manual classification, stale binding, gated appeal suggestions, prose-free claim drafts | Filesystem reads, file writes, policy or promotional-prose invention |
| `seiri-delta` | Portable semantic fingerprints and comparison | Host absolute paths as identity |
| `seiri-codex` | Bounded query projections | Command execution and Git/GitHub operations |

### Low-Level Invariants

- Source bodies are shared inside the session as `Arc<[u8]>` or `Arc<str>` and are not serialized into public wires.
- Source spans rebind path, source digest, byte length, UTF-8 character boundaries, byte offsets, lines, and Unicode-scalar columns; planner bindings recheck base digests and anchor context.
- Route artifact, entrypoint, reachability, freshness, conflict, and policy remain independent axes; the single state is a display projection.
- `ROUTE_SPECS` is the sole owner of route slugs, Japanese and English labels, target candidates, and policy boundaries.
- Public identities use framed SHA-256 over repository-relative inputs and exclude host absolute paths and private calibration bodies.
- `AnalysisCoreView` borrows canonical evidence, routes, content, and reviews; audit completion validates evidence references from claims, findings, and priorities.
- The planner has no `seiri-fs` dependency and generates paired Japanese/English edits from `SourceStore` and `LanguageTopologyIndex`.
- Program budget exhaustion, invalid UTF-8, and unsupported syntax remain typed Partial/Unknown through the membrane and planner instead of collapsing into Missing.
- Capability ceilings come only from observed program shape. Tests do not promote outcome, quality, trust, safety, or performance beyond the Evidence dimension.
- The incremental layer falls back to a scalar rebuild for README, profile, or global-coverage changes and reevaluates only a dimension frontier for local program changes. Its validation-only scalar comparison is not a performance measurement.
- Performance receipts bind target, environment, corpus, command, sample count, scalar/incremental digests, and elapsed observations. They separate the semantic receipt digest from the observation digest and apply no timing threshold.
- The geometry shadow is an undirected, bounded grammar graph. Distance and unweighted Forman-style curvature are serialized as presentation signals only. Planner ordering keeps the final `Safe`/`Guarded`/`Manual` gate as the primary key, and geometry on/off must preserve the membrane digest and every suggestion field other than order and presentation metadata.

### Boundary

This structure does not guarantee popularity, trust, safety, quality, legal fitness, or publication readiness. Standard audits do not initiate network access, file writes, Git operations, or GitHub operations.
