# Migration v7: RepoSeiri 1.1 Semantic Hardening

## 日本語

RepoSeiri 1.1は、10種類の`seiri.codex.v2` queryと`seiri.analysis.v2`、
`seiri.patch-plan.v2`の外側wireを維持しながら、README命題、program能力、
UTF-8 source binding、evidence contract、wording coverageをfail-closedにします。

### 変更される契約

| Surface | 1.0 | 1.1 |
| --- | --- | --- |
| README grammar | `seiri.readme-grammar.v1` | `seiri.readme-grammar.v2` |
| Claim atom | `seiri.readme-claim-atom.v1` | `seiri.readme-claim-atom.v2` |
| Translation alignment | `seiri.readme-translation-alignment.v1` | `seiri.readme-translation-alignment.v2` |
| Patch planner semantics | `seiri.patch-planner.v8` | `seiri.patch-planner.v9` |
| Wording lint | `seiri.wording-lint.v1` | `seiri.wording-lint.v2` |
| Plugin runtime manifest | `reposeiri.runtime-manifest.v3` | `reposeiri.runtime-manifest.v4` |

`seiri.contract.v6`の構造と31個のclosed revision keyは維持されますが、上表の
revision値が変わるため、旧binaryと新launcher、新binaryと旧launcherの組合せは
typed contract errorで停止します。silent fallbackはありません。

### 実装上の差分

- README negationは段落単位ではなくpredicate-localです。曖昧な極性は
  `UnsupportedSyntax`を伴うUnknownとして残ります。
- deserialized IR、DocumentScan、claim draft、incremental pathは公開validatorを
  通り、source path、digest、byte長、UTF-8 char boundary、line/columnを再検査します。
- evidence ID欠落は省略されずtyped errorになり、plannerは
  `evidence_contract_invalid`で全routeをholdします。
- wording lint v2は日本語と英語の規則coverageをlanguage別のvisible segment/byte数で
  公開し、visible proseのsource spanだけをfindingに使います。
- performance receiptはscalar/incremental同値を必須にします。時間値は観測であり、
  固定閾値や一般性能の根拠ではありません。

plugin bundleを更新するときは、1.1 binary、v4 runtime manifest、同梱schema digest、
launcher revision setを同じsource bindingから生成してください。

---

## English

RepoSeiri 1.1 makes README propositions, program capabilities, UTF-8 source
binding, evidence contracts, and wording coverage fail closed while retaining
the exact ten `seiri.codex.v2` queries and the outer `seiri.analysis.v2` and
`seiri.patch-plan.v2` wires.

### Contract changes

| Surface | 1.0 | 1.1 |
| --- | --- | --- |
| README grammar | `seiri.readme-grammar.v1` | `seiri.readme-grammar.v2` |
| Claim atom | `seiri.readme-claim-atom.v1` | `seiri.readme-claim-atom.v2` |
| Translation alignment | `seiri.readme-translation-alignment.v1` | `seiri.readme-translation-alignment.v2` |
| Patch planner semantics | `seiri.patch-planner.v8` | `seiri.patch-planner.v9` |
| Wording lint | `seiri.wording-lint.v1` | `seiri.wording-lint.v2` |
| Plugin runtime manifest | `reposeiri.runtime-manifest.v3` | `reposeiri.runtime-manifest.v4` |

The `seiri.contract.v6` shape and its 31 closed revision keys remain, but the
values above change. Pairing an old binary with the new launcher, or the new
binary with the old launcher, therefore stops with a typed contract error.
There is no silent fallback.

### Implementation differences

- README negation is predicate-local rather than paragraph-wide. Ambiguous
  polarity remains Unknown with `UnsupportedSyntax`.
- Public validators recheck deserialized IR, `DocumentScan`, claim drafts, and
  incremental inputs against source path, digest, byte length, UTF-8 character
  boundaries, line, and column.
- Missing evidence IDs are typed errors rather than omissions; the planner
  holds every route with `evidence_contract_invalid`.
- Wording lint v2 exposes Japanese and English rule coverage as typed visible
  segment and byte counts, and findings use visible-prose source spans only.
- Performance receipts require scalar/incremental equivalence. Elapsed values
  are observations, not fixed thresholds or general-performance evidence.

When updating the plugin bundle, generate the 1.1 binary, v4 runtime manifest,
bundled-schema digests, and launcher revision set from the same source binding.
