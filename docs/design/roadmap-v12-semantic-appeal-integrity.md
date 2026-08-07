# Roadmap v12: Semantic Appeal Integrity

## 日本語

### 1. 目的

Roadmap v12 は、README を誇張する計画ではありません。リポジトリ内で観測できる能力、最初の操作と結果、制約、根拠を、限られた読書量でも復元できる形へ投影し、根拠上限を越えずに過小主張を減らす実装契約です。

中核は `ReadmeGrammarIR`、`RepositoryCapabilityIR`、`ClaimCapabilityMembrane` です。既存の `UnderclaimLoss` は claim candidate の局所的な投影損失として残し、README 全体の価値欠落は `ReadmeValueCoverageReport` が所有します。訴求力を単一スコアへ潰しません。

固定する境界は次のとおりです。

1. `Unknown`、`Missing`、`Contested`、`Inferred`、`Explicit` を区別する。
2. 証拠のない capability や outcome を `Supported` にしない。
3. profile ごとの claim floor と、証拠・権限ごとの claim ceiling を別々に保持する。
4. 曲率・幾何は提示優先度だけに使い、証拠、support、floor、ceiling、admissibilityを変更しない。
5. 疎データフローと増分計算は scalar full recompute と同じ意味を返す最適化とする。
6. 特殊ハードウェアは研究拡張であり、既定実装、性能主張、完成条件に含めない。
7. Codex query kind は10種類、Codex wire identifierは `seiri.codex.v2` のまま維持する。

### 2. 実装バッチ

| Batch | Owner | 完了条件 |
| --- | --- | --- |
| B0 | baseline and protocol | v12正本、R12-SAIP-v1、template、baselineを固定する |
| B1 | appeal domain IR | grammar、capability、membraneの型と不変条件をcoreに置く |
| B2 | README grammar | visible proseからboundedな日英の意味・談話IRを構築する |
| B3 | value coverage shadow | 文書だけの価値欠落とnarrative gapを公開wire変更なしで計算する |
| B4 | program source session | program inputを予算内で既存source storeへ統合する |
| B5 | Rust frontend | parser固有表現を越境させずRust public surfaceをCapabilityIRへ投影する |
| B6 | claim-capability membrane | floor、ceiling、support、underclaim、overclaimを純粋関数で評価する |
| B7 | product and wire | 10 queryと`seiri.codex.v2`を維持してanalysis/report/queryへ原子的に接続する |
| B8 | planner | Safe、Guarded、Manualの境界付きREADME提案をdry-runで返す |
| B9 | calibration and completion | holdout、schema、privacy、determinism、日英文書、workspace gateを閉じる |
| B10 | sparse incremental | scalar oracleと同一digestを返す変更frontier計算を追加する |
| B11 | geometry shadow | claim意味不変のまま提示順の補助signalだけを追加する |

### 3. 主張境界

`ClaimCapabilityMembrane` は、READMEの文を大きくするための生成器ではありません。entrypointの存在だけから使いやすさを、testの存在だけから品質を、CIの存在だけから信頼性を、API名だけからruntime成功を推論しません。明示された操作、出力、example、制約、evidenceが結合できる場合だけ、出典付きの限定された価値主張を構成できます。

### 4. 完成境界

`ready_for_git` は、同じsourceに対して必要なlocal checkが通過したことだけを表します。人気、品質、安全性、法的適合、公開準備、外部host動作、性能向上を証明しません。R12-SAIP-v1はcommit、push、merge、release、publication、visibility change、plugin install、restartを許可しません。

---

## English

### 1. Purpose

Roadmap v12 is not a plan to make README language louder. It is the implementation contract for projecting repository-observable capabilities, first actions and results, constraints, and evidence into a form a reader can reconstruct within a bounded reading budget, reducing underclaim without crossing an evidence ceiling.

The core is `ReadmeGrammarIR`, `RepositoryCapabilityIR`, and `ClaimCapabilityMembrane`. Existing `UnderclaimLoss` remains the local projection loss for a claim candidate. Repository-wide value omission belongs to `ReadmeValueCoverageReport`. Appeal is not collapsed into one score.

The fixed boundaries are:

1. Keep `Unknown`, `Missing`, `Contested`, `Inferred`, and `Explicit` distinct.
2. Never mark a capability or outcome `Supported` without evidence.
3. Preserve profile-specific claim floors separately from evidence- and authority-specific claim ceilings.
4. Curvature and geometry may rank presentation only; they never change evidence, support, floors, ceilings, or admissibility.
5. Sparse dataflow and incremental computation are optimizations that must agree with scalar full recomputation.
6. Special hardware is a research extension, not a default backend, performance claim, or completion condition.
7. Preserve the ten Codex query kinds and the `seiri.codex.v2` wire identifier.

### 2. Implementation Batches

| Batch | Owner | Completion condition |
| --- | --- | --- |
| B0 | baseline and protocol | Freeze the v12 roadmap, R12-SAIP-v1, template, and baseline |
| B1 | appeal domain IR | Put grammar, capability, and membrane types and invariants in core |
| B2 | README grammar | Build bounded Japanese/English semantic and discourse IR from visible prose |
| B3 | value coverage shadow | Compute document-only value omissions and narrative gaps without a public wire change |
| B4 | program source session | Integrate program inputs into the existing source store under explicit budgets |
| B5 | Rust frontend | Project the Rust public surface into CapabilityIR without leaking parser-specific forms |
| B6 | claim-capability membrane | Evaluate floors, ceilings, support, underclaim, and overclaim through pure functions |
| B7 | product and wire | Atomically connect analysis, reports, and queries while retaining ten queries and `seiri.codex.v2` |
| B8 | planner | Return source-bound Safe, Guarded, and Manual README proposals as dry runs |
| B9 | calibration and completion | Close holdout, schema, privacy, determinism, bilingual docs, and workspace gates |
| B10 | sparse incremental | Add changed-frontier computation that agrees with the scalar-oracle digest |
| B11 | geometry shadow | Add presentation-order signals while claim semantics remain invariant |

### 3. Claim Boundary

`ClaimCapabilityMembrane` is not a generator for louder prose. It does not infer usability from an entrypoint, quality from tests, trust from CI, or runtime success from API names. It may compose a bounded, sourced value claim only when explicit operations, outputs, examples, constraints, and evidence can be joined.

### 4. Completion Boundary

`ready_for_git` means only that required local checks passed against the same source. It does not prove popularity, quality, security, legal fitness, publication readiness, external-host behavior, or performance improvement. R12-SAIP-v1 grants no authority to commit, push, merge, release, publish, change visibility, install a plugin, or restart a host.
