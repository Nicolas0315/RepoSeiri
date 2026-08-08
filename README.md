# RepoSeiri

**リポジトリが実際に示している能力と、README が伝えている価値の差を監査する。**

**Audit the gap between what a repository demonstrates and what its README communicates.**

[日本語](#日本語) · [English](#english)

[![CI](https://github.com/ViszCham/RepoSeiri/actions/workflows/ci.yml/badge.svg)](https://github.com/ViszCham/RepoSeiri/actions/workflows/ci.yml)

## 日本語

RepoSeiri は、リポジトリの入口、文書、GitHub 設定、ローカル Git 構造を bounded local evidence から読み、「実装されているのに README で伝わっていない能力」と「ローカル根拠を越えている主張」を分けてレビューする Rust 製 CLI / Codex plugin です。

### 解く問題

README の弱さを不足ファイルの一覧だけで扱わず、何が存在するか、どの主張が観測済み能力に支えられるか、どの判断を `Unknown` または `Manual` に残すべきかを、一つの source session に結び付けます。

### 1回の監査を5つの問いで見る

| 確認したいこと | RepoSeiri が返す review data |
| --- | --- |
| リポジトリには何があるか | entry point、文書、GitHub 設定、ローカル Git、program surface の bounded evidence |
| README は能力を十分に伝えているか | `ReadmeGrammarIR` と `RepositoryCapabilityIR` を結ぶ underclaim opportunity |
| 主張が根拠を越えていないか | claim floor、evidence ceiling、support、overclaim risk を分離した判定 |
| 次に何をレビューするか | route priority、wording finding、existing-target-only edit、appeal suggestion |
| Codex へ何を渡すか | 同じ source session から用途別に切り出す10種類の `seiri.codex.v2` query |

提案は source-bound かつ prose-free の dry-run review data です。標準モードはファイルを書かず、network・GitHub operation や policy invention を開始しません。

<a id="quickstart-ja"></a>

### Quickstart

Rust 1.88 以上が必要です。次の3コマンドで source checkout を取得し、RepoSeiri 自身の `summary` を表示します。

```powershell
git clone https://github.com/ViszCham/RepoSeiri.git
Set-Location RepoSeiri
cargo run --locked --quiet -p seiri-cli -- codex --path . --profile cli --query summary --format markdown
```

出力は schema と query を明示し、その後に evidence、route、README grammar、program capability、underclaim、overclaim、Unknown、patch gate の件数をまとめます。

```text
# RepoSeiri Codex Query

- Schema: `seiri.codex.v2`
- Repository: `.`
- Query: `summary`
```

別のリポジトリを調べる場合は `--path` をその root へ変更します。source checkout の workspace regression も確認する場合は次を実行します。

```powershell
cargo test --workspace --locked
```

### 実出力例

次は tracked fixture `fixtures/readme-route-repo` に対する `summary` の全出力です。

```powershell
cargo run --locked --quiet -p seiri-cli -- codex --path fixtures/readme-route-repo --scope subtree --profile common --query summary --format markdown
```

```text
# RepoSeiri Codex Query

- Schema: `seiri.codex.v2`
- Repository: `.`
- Query: `summary`

- Entries: `13`
- Evidence facts: `78`
- Route assessments: `14`
- Content slots: `63`
- README grammar nodes: `10`
- Repository capability nodes: `1`; program unknown reasons: `0`
- Underclaim opportunities: `4`; overclaim risks: `1`
- Findings: `0`
- Documents: `8` selected / `8` candidates; primary `8` / `8`
- Document budget skips: `0`; byte budget skips: `0`
- Coverage: `20` complete / `0` partial / `1` not requested; limit exceeded `0`
- Markdown coverage: `Complete`; conflict coverage: `Complete`
- Observations: `28` present / `47` absent / `1` unknown (`0` unacknowledged) / `0` conflict
- Review priorities: `55`; top route `Some(Security)` / authority `Some(MaintainerDecision)`
- Top recommendation: Review the missing content separately from route presence.
- Patch operations: `1`
- Patch holds: `3`

- Boundary: Codex queries are bounded projections of canonical local analysis. They do not write files, execute commands, call GitHub, adopt policy, or guarantee popularity, trust, security, quality, or publication readiness.
```

`Findings: 0` は、表示された coverage と budget の範囲で finding がなかったことだけを表します。リポジトリ全体の品質、安全性、信頼性を証明しません。

### 主要な使い方

| 目的 | コマンド |
| --- | --- |
| 読みやすい監査 | `cargo run --locked --quiet -p seiri-cli -- audit --path . --profile common --format markdown` |
| dry-run patch plan | `cargo run --locked --quiet -p seiri-cli -- plan --path . --profile common --format markdown` |
| Codex query | `cargo run --locked --quiet -p seiri-cli -- codex --path . --profile common --query summary --format markdown` |
| wording lint | `cargo run --locked --quiet -p seiri-cli -- lint-wording --path . --profile common --format markdown` |
| pattern registry | `cargo run --locked --quiet -p seiri-cli -- patterns --format markdown` |
| public calibration dataset | `cargo run --locked --quiet -p seiri-cli -- calibrate --input fixtures/calibration-dataset.json --format markdown` |
| public synthetic holdout | `cargo run --locked --quiet -p xtask -- calibration-holdout --format json` |
| machine contract | `cargo run --locked --quiet -p seiri-cli -- contract --format json` |
| completion evidence | `cargo run --locked --quiet -p xtask -- completion --format json` |

同じ source session を、目的に応じて次の10種類の query へ投影します。

| Query | 確認する問い |
| --- | --- |
| `summary` | 監査範囲、件数、主要な review priority は何か |
| `routes` | README と repository-local target の入口はどう結び付いているか |
| `evidence` | 各判断を支える typed evidence と coverage は何か |
| `documents` | bounded Markdown / GitHub-local 文書解析で何が観測されたか |
| `governance` | facet、content slot、整合、scope、freshness はどうなっているか |
| `patches` | existing-target edit、appeal suggestion、hold の dry-run plan は何か |
| `linter` | 可視 prose に evidence-scoped wording risk があるか |
| `actions` | review 用の typed program / argv suggestion は何か |
| `remote` | opt-in remote analysis の typed terminal state は何か |
| `pr-body` | 観測済み evidence と境界からどんな draft PR body を組み立てられるか |

profile は `common`, `library`, `cli`, `infra`, `product`, `runtime`, `docs`, `tutorial`, `ml`, `research`, `template` です。通常は repository root で `--scope repository` を使います。

holdout report は route、wording、consistency、profile、planner、appeal のprecision、recall、false positive/negative、coverage、Wilson 95% interval、実行時間を出します。現在のtracked corpusは各task 4 holdout caseの低N回帰用なので、最低20 caseを満たさず`insufficient_sample`です。一般性能の校正結果ではありません。

### 出力の読み方

- `Verified` は、存在確認済みの repository-local target と対応する構造 evidence が一致した route state です。一般的な正しさの保証ではありません。
- `Structured` は構造 evidence があり、README route が明示されていない状態です。
- `Routed` は README に入口がある状態です。target の存在までは示しません。
- `Weak`, `Overloaded`, `Stale`, `Conflicting` は review が必要な route 状態です。
- `Absent`, `Unknown`, `UnsafeToInvent` は、それぞれ非観測、観測不足、人間の policy 判断が先に必要な状態を分けます。
- `Safe`, `Guarded`, `Manual` は dry-run operation の権限境界です。planner 自身は書き込みません。

### Rust 実装の焦点

- bounded filesystem traversal、bounded UTF-8 source read、byte-accurate source span
- framed SHA-256 identity、source-session binding、portable repository-relative evidence
- code fence、inline code、HTML comment、raw code を可視 prose から分離する Markdown event IR
- `ReadmeGrammarIR`と`RepositoryCapabilityIR`を`ClaimCapabilityMembrane`で結合し、Unknown、claim floor、evidence ceilingを分離するappeal解析
- typed input digestとpath dependencyからdimension frontierだけを再評価し、validation時にscalar oracle digestとの一致を検査する疎な増分層
- graph距離とForman型曲率で同一gate内のappeal提示順だけを補助し、証拠、support、floor、ceiling、opportunity、risk、主張意味を変えないbounded geometry shadow
- visible eventを一度だけ正規化する`SemanticIndex`と、route slug・日英label・target候補を所有する`ROUTE_SPECS`
- filesystemを再読込せず、README言語topologyからsource-boundな日英ペアeditを作るplanner
- `Present`, `Absent`, `Unknown`, `Conflict`, `Disabled` を混同しない typed state
- private calibration body、exact prior、host absolute path を public artifact に出さない境界

低レイヤ設計、semantic revision、completion 条件は [Design Documentation](docs/design/README.md) にあります。これらは人気、信頼、安全性、品質、法的適合性、production readiness の保証ではありません。

### Codex plugin

plugin source は `plugins/reposeiri` にあります。`1.0.0`はtool/package versionであり、現行machine contractは`seiri.contract.v5`と27個のsemantic revisionです。launcher は `REPOSEIRI_BIN`、bundle-local binary、`PATH` の順に native runtime を解決し、contract、semantic revision、bundle manifest、binary SHA-256、同梱schema SHA-256を検証します。

plugin は Rust core の10 queryを使う薄い adapter です。query output と mutation authority は分離され、file write、command execution、branch、commit、push、PR、merge には個別の明示権限が必要です。

### 文書と方針

| 読みたいもの | 入口 |
| --- | --- |
| 文書地図 | [Documentation Topology](docs/README.md) |
| release | [Release Process](docs/release.md) |
| lifecycle | [Lifecycle Boundary](docs/lifecycle.md) |
| self-audit | [Self-Audit Loop](docs/self-audit.md) |
| security report | [SECURITY.md](SECURITY.md) |
| support | [SUPPORT.md](SUPPORT.md) |
| Issue受付 | [Issue受付](.github/ISSUE_TEMPLATE/) |
| contribution | [CONTRIBUTING.md](CONTRIBUTING.md) |
| governance | [GOVERNANCE.md](GOVERNANCE.md) |
| license | [LICENSE](LICENSE) |
| ownership | [CODEOWNERS](.github/CODEOWNERS) |
| change history | [CHANGELOG.md](CHANGELOG.md) |

RepoSeiri v1.0.0 は個人開発・Rust coding practice として公開しています。固定 SLA、release cadence、compatibility duration、外部 contribution 採用を約束しません。

---

## English

RepoSeiri is a Rust CLI and Codex plugin that reads repository entry points, documents, GitHub configuration, and local Git structure from bounded local evidence, then separates capabilities the README fails to communicate from claims that exceed the observed evidence.

### Problem

Instead of treating README weakness as only a list of missing files, it binds three questions to one source session: what exists, which claims are supported by observed capability, and which decisions must remain `Unknown` or `Manual`.

### Five Questions From One Audit

| Question | Review data returned by RepoSeiri |
| --- | --- |
| What is in the repository? | Bounded evidence for entry points, documents, GitHub configuration, local Git, and program surfaces |
| Does the README communicate observed capability? | Underclaim opportunities joining `ReadmeGrammarIR` with `RepositoryCapabilityIR` |
| Does a claim exceed local evidence? | Separate claim floors, evidence ceilings, support states, and overclaim risks |
| What should be reviewed next? | Route priorities, wording findings, existing-target-only edits, and appeal suggestions |
| What should Codex receive? | Ten purpose-specific `seiri.codex.v2` queries projected from the same source session |

Suggestions are source-bound, prose-free dry-run review data. Standard mode does not write files or initiate network operations, GitHub operations, or policy invention.

<a id="quickstart-en"></a>

### Quickstart

Rust 1.88 or newer is required. These three commands fetch the source checkout and show a `summary` audit of RepoSeiri itself.

```powershell
git clone https://github.com/ViszCham/RepoSeiri.git
Set-Location RepoSeiri
cargo run --locked --quiet -p seiri-cli -- codex --path . --profile cli --query summary --format markdown
```

The output identifies its schema and query before summarizing evidence, routes, README grammar, program capability, underclaim, overclaim, Unknown states, and patch gates.

```text
# RepoSeiri Codex Query

- Schema: `seiri.codex.v2`
- Repository: `.`
- Query: `summary`
```

To inspect another repository, change `--path` to its root. To also check the source checkout's workspace regressions, run:

```powershell
cargo test --workspace --locked
```

### Real Output Example

The following is the complete `summary` output for the tracked `fixtures/readme-route-repo` fixture.

```powershell
cargo run --locked --quiet -p seiri-cli -- codex --path fixtures/readme-route-repo --scope subtree --profile common --query summary --format markdown
```

```text
# RepoSeiri Codex Query

- Schema: `seiri.codex.v2`
- Repository: `.`
- Query: `summary`

- Entries: `13`
- Evidence facts: `78`
- Route assessments: `14`
- Content slots: `63`
- README grammar nodes: `10`
- Repository capability nodes: `1`; program unknown reasons: `0`
- Underclaim opportunities: `4`; overclaim risks: `1`
- Findings: `0`
- Documents: `8` selected / `8` candidates; primary `8` / `8`
- Document budget skips: `0`; byte budget skips: `0`
- Coverage: `20` complete / `0` partial / `1` not requested; limit exceeded `0`
- Markdown coverage: `Complete`; conflict coverage: `Complete`
- Observations: `28` present / `47` absent / `1` unknown (`0` unacknowledged) / `0` conflict
- Review priorities: `55`; top route `Some(Security)` / authority `Some(MaintainerDecision)`
- Top recommendation: Review the missing content separately from route presence.
- Patch operations: `1`
- Patch holds: `3`

- Boundary: Codex queries are bounded projections of canonical local analysis. They do not write files, execute commands, call GitHub, adopt policy, or guarantee popularity, trust, security, quality, or publication readiness.
```

`Findings: 0` means only that no finding was emitted within the displayed coverage and budgets. It does not prove repository-wide quality, safety, or trustworthiness.

### Main Uses

| Purpose | Command |
| --- | --- |
| Human-readable audit | `cargo run --locked --quiet -p seiri-cli -- audit --path . --profile common --format markdown` |
| Dry-run patch plan | `cargo run --locked --quiet -p seiri-cli -- plan --path . --profile common --format markdown` |
| Codex query | `cargo run --locked --quiet -p seiri-cli -- codex --path . --profile common --query summary --format markdown` |
| Wording lint | `cargo run --locked --quiet -p seiri-cli -- lint-wording --path . --profile common --format markdown` |
| Pattern registry | `cargo run --locked --quiet -p seiri-cli -- patterns --format markdown` |
| Public calibration dataset | `cargo run --locked --quiet -p seiri-cli -- calibrate --input fixtures/calibration-dataset.json --format markdown` |
| Public synthetic holdout | `cargo run --locked --quiet -p xtask -- calibration-holdout --format json` |
| Machine contract | `cargo run --locked --quiet -p seiri-cli -- contract --format json` |
| Completion evidence | `cargo run --locked --quiet -p xtask -- completion --format json` |

The same source session is projected into ten query kinds for different review questions.

| Query | Question answered |
| --- | --- |
| `summary` | What was covered, how large was the result, and what review priority is visible? |
| `routes` | How do README entry points connect to repository-local targets? |
| `evidence` | Which typed evidence and coverage support each decision? |
| `documents` | What did bounded Markdown and GitHub-local document analysis observe? |
| `governance` | What are the facet, content-slot, consistency, scope, and freshness states? |
| `patches` | Which existing-target edits, appeal suggestions, and holds are in the dry-run plan? |
| `linter` | Does visible prose contain an evidence-scoped wording risk? |
| `actions` | Which typed program or argv suggestions are available for review? |
| `remote` | What is the typed terminal state of opt-in remote analysis? |
| `pr-body` | What draft PR body can be assembled from observed evidence and boundaries? |

Profiles are `common`, `library`, `cli`, `infra`, `product`, `runtime`, `docs`, `tutorial`, `ml`, `research`, and `template`. Normally, use `--scope repository` from the repository root.

The holdout report emits precision, recall, false positives/negatives, coverage, a Wilson 95% interval, and runtime for routes, wording, consistency, profiles, planning, and appeal. The tracked corpus currently has four holdout cases per task, below the minimum of 20, so it remains `insufficient_sample`. It is regression data, not general performance calibration.

### Reading Output

- `Verified` is a route state where an existence-checked repository-local target agrees with matching structural evidence. It is not a general correctness guarantee.
- `Structured` means structural evidence exists without an explicit README route.
- `Routed` means the README contains an entry point. It does not establish that the target exists.
- `Weak`, `Overloaded`, `Stale`, and `Conflicting` are route states that require review.
- `Absent`, `Unknown`, and `UnsafeToInvent` separate non-observation, insufficient observation, and cases where a human policy decision must come first.
- `Safe`, `Guarded`, and `Manual` are authority boundaries for dry-run operations. The planner itself does not write.

### Rust Implementation Focus

- Bounded filesystem traversal, bounded UTF-8 source reads, and byte-accurate source spans
- Framed SHA-256 identities, source-session binding, and portable repository-relative evidence
- A Markdown event IR that separates code fences, inline code, HTML comments, and raw code from visible prose
- Appeal analysis that joins `ReadmeGrammarIR` and `RepositoryCapabilityIR` through `ClaimCapabilityMembrane` while keeping Unknown, claim floors, and evidence ceilings separate
- A sparse incremental layer that reevaluates a dimension frontier from typed input digests and path dependencies, with validation against the scalar-oracle digest
- A bounded geometry shadow that may reorder equal-gate appeal suggestions using graph distance and Forman-style curvature, while leaving evidence, support, floors, ceilings, opportunities, risks, and claim semantics unchanged
- A `SemanticIndex` that normalizes visible events once and `ROUTE_SPECS` that owns route slugs, bilingual labels, and target candidates
- A planner that does not reread the filesystem and derives source-bound paired Japanese/English edits from README language topology
- Typed `Present`, `Absent`, `Unknown`, `Conflict`, and `Disabled` states
- Boundaries that keep private calibration bodies, exact priors, and host absolute paths out of public artifacts

Low-level design, semantic revisions, and completion conditions are in [Design Documentation](docs/design/README.md). They are not guarantees of popularity, trust, security, quality, legal fitness, or production readiness.

### Codex Plugin

Plugin source lives in `plugins/reposeiri`. `1.0.0` is the tool/package version; the current machine contract is `seiri.contract.v5` with 27 semantic revisions. The launcher resolves the native runtime in the order `REPOSEIRI_BIN`, bundle-local binary, then `PATH`, and validates the contract, semantic revisions, bundle manifest, binary SHA-256, and bundled-schema SHA-256 values.

The plugin is a thin adapter over the ten Rust-core queries. Query output remains separate from mutation authority; file writes, command execution, branches, commits, pushes, PRs, and merges each require separate explicit authorization.

### Documentation And Policy

| Topic | Entry |
| --- | --- |
| Documentation map | [Documentation Topology](docs/README.md) |
| Release | [Release Process](docs/release.md) |
| Lifecycle | [Lifecycle Boundary](docs/lifecycle.md) |
| Self-audit | [Self-Audit Loop](docs/self-audit.md) |
| Security reporting | [SECURITY.md](SECURITY.md) |
| Support | [SUPPORT.md](SUPPORT.md) |
| Issue intake | [Issue intake](.github/ISSUE_TEMPLATE/) |
| Contributions | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Governance | [GOVERNANCE.md](GOVERNANCE.md) |
| License | [LICENSE](LICENSE) |
| Ownership | [CODEOWNERS](.github/CODEOWNERS) |
| Change history | [CHANGELOG.md](CHANGELOG.md) |

RepoSeiri v1.0.0 is public as personal development and Rust coding practice. It does not promise a fixed SLA, release cadence, compatibility duration, or acceptance of external contributions.
