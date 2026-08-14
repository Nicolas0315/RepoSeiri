# R13-HF-v1: Hardening Follow-up

## 日本語

R13-HF-v1 は Roadmap v13 の B9 と B10 の間に置く、意味論と入力境界の
hardening 列です。目的は機能追加そのものではなく、公開・直列化可能な IR、
増分評価、Markdown/Rust の byte span、evidence 参照が壊れた場合に panic、
黙殺、または誤った再利用へ落ちないようにすることです。

次の前提は変更しません。

- 中核は `ReadmeGrammarIR`、`RepositoryCapabilityIR`、`ClaimCapabilityMembrane`。
- `Unknown` を消去せず、契約違反を `Unknown` に偽装しない。
- evidence ceiling を越える主張を生成しない。
- scalar full recomputation を意味論オラクルとする。
- Codex query は正確に10種類、外側wireは `seiri.codex.v2`。
- planner と plugin の提案は review-only で、`writes_files` は常に false。
- unsafe、特殊ハードウェア、永続cacheは導入しない。

| Batch | 所有範囲 | Gate |
| --- | --- | --- |
| H0 | baseline / reproducer contract | adversarial corpus、固定不変条件、現行wireを固定 |
| H1 | checked membrane boundary | 不正IRをtyped errorで拒否しpanicしない |
| H2 | shared semantic input projection | scalarとincrementalが同一projectionを使用しdigestが一致 |
| H3 | predicate-local polarity | 語境界・述語局所の極性判定、曖昧さを明示 |
| H4 | evidence / planner fail-closed | 欠落evidence参照を黙殺せずtyped hold/errorにする |
| H5 | source-bound UTF-8 spans | path、digest、byte長、UTF-8境界、line/columnをsourceへ照合 |
| H6 | Japanese wording coverage | 日本語規則と検査coverageを型付きで公開 |
| H7 | contract / schema / query / plugin parity | revision、schema、10 query、両launcherの同値性を固定 |
| H8 | deterministic performance receipt | indexのscalar同値性と再現可能なreceiptだけをhard gateにする |

停止規則は依存コーン単位です。H1の契約違反、H2のdigest不一致、H4の
evidence欠落黙殺、H5のUTF-8境界違反、H7の10 queryまたは外側wire破壊は、
それに依存する後続batchを停止します。性能時間に固定閾値は置かず、H8の失敗は
最適化だけを停止します。

## English

R13-HF-v1 is a semantic and input-boundary hardening column inserted between
Roadmap v13 B9 and B10. It does not add features for their own sake. It prevents
public or deserializable IR, incremental evaluation, Markdown/Rust byte spans,
and evidence references from degrading into panics, silent omission, or stale
reuse when their contracts are violated.

The following premises remain fixed:

- `ReadmeGrammarIR`, `RepositoryCapabilityIR`, and `ClaimCapabilityMembrane`
  remain the core pipeline.
- Preserve `Unknown`, but never disguise a contract violation as `Unknown`.
- Never generate a claim above its evidence ceiling.
- Scalar full recomputation remains the semantic oracle.
- Keep exactly ten Codex query kinds and the outer `seiri.codex.v2` wire.
- Planner and plugin proposals remain review-only and always report
  `writes_files: false`.
- Introduce no unsafe code, special hardware, or persistent cache.

| Batch | Ownership | Gate |
| --- | --- | --- |
| H0 | baseline / reproducer contract | Freeze the adversarial corpus, fixed invariants, and current wire |
| H1 | checked membrane boundary | Reject invalid IR with typed errors and no panic |
| H2 | shared semantic input projection | Scalar and incremental paths use one projection and equal digests |
| H3 | predicate-local polarity | Use token boundaries and predicate-local polarity; expose ambiguity |
| H4 | evidence / planner fail-closed | Missing evidence references become typed holds or errors |
| H5 | source-bound UTF-8 spans | Bind path, digest, byte length, UTF-8 boundaries, line, and column to source |
| H6 | Japanese wording coverage | Publish typed Japanese rules and inspection coverage |
| H7 | contract / schema / query / plugin parity | Freeze revisions, schemas, ten queries, and both launchers |
| H8 | deterministic performance receipt | Hard-gate scalar equivalence and reproducible receipts, not elapsed thresholds |

Stop only the affected dependency cone. H1 contract failures, H2 digest
divergence, H4 silently missing evidence, H5 UTF-8 boundary violations, or H7
breakage of the ten-query/outer-wire contract stop their dependants. H8 has no
fixed timing threshold; a performance failure stops only that optimization.
