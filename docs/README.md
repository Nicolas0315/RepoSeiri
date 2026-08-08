# Documentation Topology

## 日本語

RepoSeiri は root README をアプリの入口、`docs/README.md` を文書地図、専門文書を詳細面として分けます。README には概要、quickstart、主要 command、root policy への route だけを置き、設計と運用手順は docs に置きます。

この topology は文書の完全性や品質を保証しません。どの問いをどの文書が所有するかを明確にします。

### 目的から選ぶ

- まず監査する: [README Quickstart](../README.md#quickstart-ja) から `summary` を表示し、[主要な使い方](../README.md#主要な使い方) で10 queryを選びます。
- stateとgateを読む: [出力の読み方](../README.md#出力の読み方) で `Verified`、`Unknown`、`Safe`、`Manual` の境界を確認します。
- 自己検証を回す: [Self-Audit Loop](self-audit.md) で local、CI、Codex、manual reviewを分けます。
- 実装責務を追う: [Current Rust Architecture](design/current-rust-architecture.md) でscanからqueryまでのdata flowとcrate所有境界を読みます。
- appeal設計を追う: [Roadmap v12](design/roadmap-v12-semantic-appeal-integrity.md) からunderclaim、evidence ceiling、Unknown保持、増分層、geometry shadowの設計へ入ります。
- 運用とpolicyを確認する: release、lifecycle、security、support、contributionは下のSource of truthから各authorityへ進みます。

### リポジトリ構造の入口

workspaceはbounded input、identity/analysis、appeal/calibration、review projection、product surfaceに責務を分けています。完全なcomponent一覧、「所有する／しない」責務、低レイヤ不変条件は [Current Rust Architecture](design/current-rust-architecture.md) を正とし、ここでは別のarchitecture定義を作りません。

### First-read order

| Step | Entry | Role |
| --- | --- | --- |
| 1 | [README](../README.md) | アプリの目的、quickstart、主要 command、root route |
| 2 | [Documentation Topology](README.md) | docs 全体の地図と所有境界 |
| 3 | [Design Documentation](design/README.md) | trust graph、baseline/profile、Roadmap v5-v12、R12-SAIP-v1 |
| 4 | [Self-Audit Loop](self-audit.md) | local/CI/Codex/manual check の固定 loop |
| 5 | [Publication Readiness](publication-readiness.md) | 公開状態を確認する checklist |
| 6 | [Release Process](release.md) | release の手動手順 |
| 7 | [Lifecycle Boundary](lifecycle.md) | 保守、非推奨、archive の約束を自動推定しない境界 |
| 8 | [Repository Hygiene](hygiene.md) | tracked/generated/ignored artifact の境界 |
| 9 | [Changelog](../CHANGELOG.md) | 利用者向け変更履歴 |

### Source of truth

| Question | Read first |
| --- | --- |
| RepoSeiri とは何か、どう動かすか | [README](../README.md) |
| canonical 0.2.0 Rust architecture | [Roadmap v5](design/roadmap-v5-legacy-removal.md) |
| 1.0.0 completion baselineの実装記録 | [Roadmap v6](design/roadmap-v6-completion.md) |
| 一括完成実装の実行契約 | [RCBP-v1](design/completion-batch-protocol.md) |
| 直前のtrust contractと一括実装記録 | [Roadmap v8](design/roadmap-v8-trust-contract-integrity.md) / [RTIP-v1](design/rtip-v1-protocol.md) |
| 現行の完成改善と非対話一括実装契約 | [Roadmap v10](design/roadmap-v10-closure-and-product-integrity.md) / [R10-SIP-v1](design/r10-sip-v1-protocol.md) |
| 現行のsemantic compressionとproduct closure | [Roadmap v11](design/roadmap-v11-semantic-compression.md) / [R11-SCCP-v1](design/r11-sccp-v1-protocol.md) |
| 現行Rust crate/data flow | [Current Rust Architecture](design/current-rust-architecture.md) |
| v2 wire移行 | [v2 Migration](migration-v2.md) |
| Roadmap v9-v10 portable/completion/runtime移行 | [v3 Migration](migration-v3.md) |
| R11 source/semantic/planner移行 | [v4 Migration](migration-v4.md) |
| R12 appeal IR/program/membrane移行 | [v5 Migration](migration-v5.md) |
| public schema snapshot | [`schemas/`](../schemas) |
| repository route と claim の長期前提 | [Repository Trust Graph](design/repository-trust-graph.md) |
| baseline、profile、calibration input | [Baseline And Profiles](design/baseline-and-profiles.md) |
| self-audit | [Self-Audit Loop](self-audit.md) |
| 公開状態 | [Publication Readiness](publication-readiness.md) |
| release | [Release Process](release.md) |
| lifecycle | [Lifecycle Boundary](lifecycle.md) |
| security report | [Security Policy](../SECURITY.md) |
| support | [Support](../SUPPORT.md) |
| contribution | [Contributing](../CONTRIBUTING.md) |
| governance | [Governance](../GOVERNANCE.md) |
| repository hygiene | [Repository Hygiene](hygiene.md) |

### Ownership boundaries

| Area | Owns | Does not own |
| --- | --- | --- |
| root README | first-read route、quickstart、主要 command | 詳細設計、release 手順 |
| `docs/design/` | architecture、analysis model、roadmap | support/security policy、GitHub 操作権限 |
| release docs | changelog、release procedure | 自動 publication、保守保証 |
| lifecycle docs | tool/package version、machine contract、判断境界 | 将来の保守期間、support SLA、archive日程 |
| root policy files | license、security、support、contribution | implementation roadmap |
| `.github/` | intake、PR template、CI、dependency update | maintainer の最終判断 |
| fixtures | deterministic test input | 実 policy、実利用者 data、private analysis data |

### Documentation rules

1. 新しい文書は `design`、`release`、`policy`、`operations`、`hygiene` の owner を決めてから追加します。
2. 同じ責務の文書を並存させず、現行の正を更新します。
3. root policy を docs で再定義せず、正の file へ route します。
4. 主要文書は日本語前半、英語後半で同じ内容を維持します。
5. command を変更した場合は README、self-audit、CI、plugin skill を同時に更新します。
6. RepoSeiri の score、state、finding は review aid であり、品質や信頼の保証として書きません。

---

## English

RepoSeiri separates the root README as the application entry point, `docs/README.md` as the document map, and specialized documents as detail surfaces. The README contains only the overview, quickstart, main commands, and routes to root policies. Design and operating procedures live in docs.

This topology does not guarantee documentation completeness or quality. It clarifies which document owns each question.

### Choose By Goal

- Start an audit: use the [README Quickstart](../README.md#quickstart-en) to display `summary`, then choose among the ten queries in [Main Uses](../README.md#main-uses).
- Interpret states and gates: use [Reading Output](../README.md#reading-output) for boundaries such as `Verified`, `Unknown`, `Safe`, and `Manual`.
- Run the self-check: [Self-Audit Loop](self-audit.md) separates local, CI, Codex, and manual review.
- Trace implementation ownership: [Current Rust Architecture](design/current-rust-architecture.md) maps the data flow and crate boundaries from scanning through queries.
- Trace appeal design: [Roadmap v12](design/roadmap-v12-semantic-appeal-integrity.md) leads into underclaim, evidence ceilings, Unknown retention, incremental analysis, and the geometry shadow.
- Review operations and policy: use Source Of Truth below to reach the authorities for release, lifecycle, security, support, and contributions.

### Repository Structure Entry Point

The workspace separates bounded input, identity and analysis, appeal and calibration, review projection, and product-surface responsibilities. [Current Rust Architecture](design/current-rust-architecture.md) remains authoritative for the complete component list, owned and excluded responsibilities, and low-level invariants; this page does not create a parallel architecture definition.

### First-Read Order

| Step | Entry | Role |
| --- | --- | --- |
| 1 | [README](../README.md) | Application purpose, quickstart, main commands, and root routes |
| 2 | [Documentation Topology](README.md) | Map and ownership boundaries for all docs |
| 3 | [Design Documentation](design/README.md) | Trust graph, baseline/profile model, Roadmaps v5-v12, and R12-SAIP-v1 |
| 4 | [Self-Audit Loop](self-audit.md) | Fixed local, CI, Codex, and manual-check loop |
| 5 | [Publication Readiness](publication-readiness.md) | Checklist for reviewing public state |
| 6 | [Release Process](release.md) | Manual release procedure |
| 7 | [Lifecycle Boundary](lifecycle.md) | Boundary against inferring maintenance, deprecation, or archival promises |
| 8 | [Repository Hygiene](hygiene.md) | Boundaries for tracked, generated, and ignored artifacts |
| 9 | [Changelog](../CHANGELOG.md) | User-facing change history |

### Source Of Truth

| Question | Read first |
| --- | --- |
| What RepoSeiri is and how to run it | [README](../README.md) |
| Canonical 0.2.0 Rust architecture | [Roadmap v5](design/roadmap-v5-legacy-removal.md) |
| 1.0.0 completion-baseline implementation record | [Roadmap v6](design/roadmap-v6-completion.md) |
| Completion-batch execution contract | [RCBP-v1](design/completion-batch-protocol.md) |
| Preceding trust contract and batch-execution record | [Roadmap v8](design/roadmap-v8-trust-contract-integrity.md) / [RTIP-v1](design/rtip-v1-protocol.md) |
| Current completion improvements and noninteractive batch execution | [Roadmap v10](design/roadmap-v10-closure-and-product-integrity.md) / [R10-SIP-v1](design/r10-sip-v1-protocol.md) |
| Current semantic compression and product closure | [Roadmap v11](design/roadmap-v11-semantic-compression.md) / [R11-SCCP-v1](design/r11-sccp-v1-protocol.md) |
| Current Rust crate and data flow | [Current Rust Architecture](design/current-rust-architecture.md) |
| v2 wire migration | [v2 Migration](migration-v2.md) |
| Roadmaps v9-v10 portable/completion/runtime migration | [v3 Migration](migration-v3.md) |
| R11 source, semantic, and planner migration | [v4 Migration](migration-v4.md) |
| R12 appeal IR, program, and membrane migration | [v5 Migration](migration-v5.md) |
| Public schema snapshots | [`schemas/`](../schemas) |
| Long-term repository-route and claim premises | [Repository Trust Graph](design/repository-trust-graph.md) |
| Baselines, profiles, and calibration inputs | [Baseline And Profiles](design/baseline-and-profiles.md) |
| Self-audit | [Self-Audit Loop](self-audit.md) |
| Public state | [Publication Readiness](publication-readiness.md) |
| Release | [Release Process](release.md) |
| Lifecycle | [Lifecycle Boundary](lifecycle.md) |
| Security reporting | [Security Policy](../SECURITY.md) |
| Support | [Support](../SUPPORT.md) |
| Contributions | [Contributing](../CONTRIBUTING.md) |
| Governance | [Governance](../GOVERNANCE.md) |
| Repository hygiene | [Repository Hygiene](hygiene.md) |

### Ownership Boundaries

| Area | Owns | Does not own |
| --- | --- | --- |
| root README | First-read route, quickstart, and main commands | Detailed design and release procedure |
| `docs/design/` | Architecture, analysis model, and roadmap | Support/security policy and GitHub operational authority |
| release docs | Changelog and release procedure | Automatic publication or maintenance commitments |
| lifecycle docs | Tool/package version, machine contract, and decision boundaries | Future maintenance duration, support SLAs, or archival schedules |
| root policy files | License, security, support, and contribution policy | Implementation roadmap |
| `.github/` | Intake, PR template, CI, and dependency updates | Final maintainer judgment |
| fixtures | Deterministic test input | Real policy, real user data, and private analysis data |

### Documentation Rules

1. Assign a new document to a `design`, `release`, `policy`, `operations`, or `hygiene` owner before adding it.
2. Do not keep documents with the same responsibility in parallel; update the current authority.
3. Do not redefine root policy in docs; route readers to the authoritative file.
4. Keep equivalent content in the Japanese-first and English-second halves of major documents.
5. When a command changes, update README, self-audit, CI, and the plugin skill together.
6. RepoSeiri scores, states, and findings are review aids, not guarantees of quality or trust.
