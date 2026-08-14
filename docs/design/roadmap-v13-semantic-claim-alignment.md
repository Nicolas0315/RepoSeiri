# Roadmap v13: Semantic Claim Alignment

## 日本語

### 1. 目的

Roadmap v13は、Roadmap v12で導入した`ReadmeGrammarIR`、`RepositoryCapabilityIR`、`ClaimCapabilityMembrane`を、次元単位の存在照合から命題単位の意味照合へ進める実装契約です。READMEを派手にすることではなく、観測できる能力を過小主張せず、同時に証拠上限を越えないことを目的とします。

固定する境界は次の通りです。

1. 中核は`ReadmeGrammarIR`、`RepositoryCapabilityIR`、`ClaimCapabilityMembrane`とする。
2. ValueDimensionだけの一致は`Supported`の十分条件にしない。
3. `Unknown`を消去、既定値化、または暗黙に`Observed`へ昇格しない。
4. claim floorとevidence ceilingを別の型と計算として維持する。
5. geometryは提示順だけを変え、証拠、support、floor、ceiling、admissibilityを変えない。
6. plannerはprose-free、source-bound、preview-onlyで、`writes_files`は常にfalseとする。
7. 既存の10個のCodex query kindと`seiri.codex.v2`外側wireを維持する。
8. canonical orderingとsource/session digestの決定論を維持する。
9. 疎データフローと増分計算はscalar full recomputationと同じ意味digestを返す。
10. unsafe、SIMD、GPU、mmap、特殊ハードウェアは測定済みの必要性が生じるまで既定経路へ入れない。
11. 性能主張は対象、環境、corpus、sample数を束縛したbenchmark receiptがある場合だけ行う。
12. README候補は再解析し、Unknown増加、ceiling超過、またはstale digestがあればholdする。

### 2. 実装batch

| Batch | 所有範囲 | 完了条件 |
| --- | --- | --- |
| B0 | baseline freeze | roadmap、protocol、template、10 query characterization、adversarial corpusを固定する |
| B1 | visible clause IR | inline構造を復元し、negationとmodalityを句スコープへ限定する |
| B2 | claim atom IR | subject、action、object、qualifier、condition、polarityを型付き命題へ投影する |
| B3 | capability frontend v2 | Rust能力を字句・構文境界から抽出し、未対応領域をspan-local Unknownにする |
| B4 | membrane v2 | claim atomとcapability signatureを明示的に照合し、dimension-only一致を不十分とする |
| B5 | ceiling and Unknown | FirstResult、Example、Constraint、Differentiationと全次元のceiling/riskを整合させる |
| B6 | bilingual semantics | JA/EN/Mixed/Ambiguousと翻訳の極性・qualifier・section対応を扱う |
| B7 | claim draft round-trip | prose-freeな`ClaimDraftIR`と候補READMEの再監査gateを追加する |
| B8 | contract hardening | schema、portable snapshot、scope、path/span validationをfail-closedにする |
| B9 | query and plugin parity | 10 query、JSON/Markdown、Windows/Unix launcherの意味同値性を閉じる |
| B10 | performance core | benchmark後にindex、batch session、疎増分、決定論的並列化を行う |
| B11 | README product surface | 実装・検証済みの能力、制約、実例、性能receiptだけをREADMEへ投影する |

### 3. 停止規則

- 意味論不一致、Unknown消失、evidence ceiling退行はclaim生成とREADME依存コーンを停止する。
- 10 queryまたは`seiri.codex.v2`破壊はquery、plugin、公開README依存コーンを停止する。
- digest非決定性はportable snapshot、incremental、parallel、公開例を停止する。
- scope逸脱はaction、portable compare、plugin surfaceを停止する。
- performance gate失敗は該当最適化だけを停止し、意味論変更を巻き戻さない。
- geometryがclaim意味へ流入した場合はgeometry依存コーン全体を停止する。

### 4. 完了境界

`ready_for_git`は同じsourceに対する必要なlocal gateが通ったことだけを表します。commit、push、merge、release、publication、plugin installation、restartの権限ではなく、人気、品質、安全性、法的適合、外部host動作、一般性能の証明でもありません。

---

## English

### 1. Purpose

Roadmap v13 advances the `ReadmeGrammarIR`, `RepositoryCapabilityIR`, and `ClaimCapabilityMembrane` introduced by Roadmap v12 from dimension-level presence checks to proposition-level semantic alignment. Its purpose is not louder README prose. It is to avoid understating observed capability without crossing an evidence ceiling.

The fixed boundaries are:

1. Keep `ReadmeGrammarIR`, `RepositoryCapabilityIR`, and `ClaimCapabilityMembrane` as the core.
2. A matching ValueDimension is not sufficient for `Supported`.
3. Never erase, default, or silently promote `Unknown` to `Observed`.
4. Keep claim floors and evidence ceilings as separate types and computations.
5. Geometry may change presentation order only; it never changes evidence, support, floors, ceilings, or admissibility.
6. The planner remains prose-free, source-bound, preview-only, and always emits `writes_files: false`.
7. Preserve the existing ten Codex query kinds and the outer `seiri.codex.v2` wire.
8. Preserve canonical ordering and deterministic source/session digests.
9. Sparse dataflow and incremental computation must return the same semantic digest as scalar full recomputation.
10. Do not place unsafe code, SIMD, GPU, mmap, or special hardware on the default path until measured need justifies it.
11. Make performance claims only with a benchmark receipt bound to the target, environment, corpus, and sample count.
12. Reparse every README candidate and hold it if Unknown increases, a ceiling is exceeded, or the source digest is stale.

### 2. Implementation Batches

| Batch | Ownership | Completion condition |
| --- | --- | --- |
| B0 | baseline freeze | Freeze the roadmap, protocol, template, ten-query characterization, and adversarial corpus |
| B1 | visible clause IR | Reconstruct inline structure and scope negation and modality to clauses |
| B2 | claim atom IR | Project subject, action, object, qualifier, condition, and polarity into typed propositions |
| B3 | capability frontend v2 | Extract Rust capability at lexical and syntactic boundaries and retain unsupported regions as span-local Unknown |
| B4 | membrane v2 | Explicitly align claim atoms with capability signatures and treat dimension-only matches as insufficient |
| B5 | ceiling and Unknown | Align FirstResult, Example, Constraint, Differentiation, and ceiling/risk handling for every dimension |
| B6 | bilingual semantics | Handle JA/EN/Mixed/Ambiguous states and translation polarity, qualifiers, and section alignment |
| B7 | claim draft round-trip | Add prose-free `ClaimDraftIR` and candidate README re-audit gates |
| B8 | contract hardening | Fail closed across schemas, portable snapshots, scopes, paths, and spans |
| B9 | query and plugin parity | Close semantic parity across ten queries, JSON/Markdown, and Windows/Unix launchers |
| B10 | performance core | After measurement, add indexes, batch sessions, sparse incremental updates, and deterministic parallelism |
| B11 | README product surface | Project only implemented and verified capabilities, limits, examples, and performance receipts into README |

### 3. Stop Rules

- A semantic mismatch, Unknown loss, or evidence-ceiling regression stops the claim-generation and README dependency cone.
- Breaking the ten queries or `seiri.codex.v2` stops the query, plugin, and public README dependency cone.
- Digest nondeterminism stops portable snapshots, incremental computation, parallel computation, and public examples.
- Scope escape stops actions, portable comparison, and plugin surfaces.
- A performance-gate failure stops only the affected optimization; it does not roll back semantic work.
- If geometry changes claim meaning, stop the entire geometry dependency cone.

### 4. Completion Boundary

`ready_for_git` means only that required local gates passed against the same source. It grants no authority to commit, push, merge, release, publish, install a plugin, or restart a host, and it proves neither popularity, quality, security, legal fitness, external-host behavior, nor general performance.
