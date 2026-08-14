# Migration v6

## 日本語

Roadmap v13は、公開wireの`seiri.analysis.v2`、`seiri.patch-plan.v2`、`seiri.codex.v2`と10種類のCodex queryを維持したまま、命題単位のREADME解析と証拠上限を強化します。machine contractは`seiri.contract.v6`へ上がり、closed semantic revision集合は27個から31個へ増えます。

追加keyは`readme_claim_atom`、`readme_translation_alignment`、`claim_draft`、`claim_draft_reaudit`です。launcherは31個すべての名前・値・個数を検証し、部分集合や旧contractへfallbackしません。

`seiri.patch-plan.v2`はsource-boundでprose-freeな`claim_drafts`と、`not_requested`、`ready`、typed `held`の`claim_draft_state`を持てます。planner semantic revisionは`seiri.patch-planner.v8`です。draftはREADME本文を保持せず、fileを書きません。Unknown増加、証拠上限超過、stale source、semantic drift、scope escapeはre-auditでholdされます。

portable auditは`seiri.portable-audit.v3`です。document role・byte数・scan status・content digest・encoding、conflict relation、obligation facetをtyped preimageとして保持し、deserialize後にrecordとaggregate digestを再計算できます。v2 snapshotはv3として黙って受理されません。

## English

Roadmap v13 strengthens proposition-level README analysis and evidence ceilings while preserving `seiri.analysis.v2`, `seiri.patch-plan.v2`, `seiri.codex.v2`, and the exact ten Codex queries. The machine contract advances to `seiri.contract.v6`, expanding the closed semantic-revision set from 27 keys to 31.

The added keys are `readme_claim_atom`, `readme_translation_alignment`, `claim_draft`, and `claim_draft_reaudit`. Launchers validate the names, values, and exact count of all 31 keys; they do not accept a subset or fall back to an older contract.

`seiri.patch-plan.v2` may now carry source-bound, prose-free `claim_drafts` and an explicit `claim_draft_state` of `not_requested`, `ready`, or typed `held`. The planner semantic revision is `seiri.patch-planner.v8`. Drafts retain no README body and authorize no file write. Re-audit holds Unknown growth, an exceeded evidence ceiling, stale source, semantic drift, and scope escape.

Portable audit advances to `seiri.portable-audit.v3`. It retains typed preimages for document role, byte count, scan status, content digest, encoding, conflict relation, and obligation facet so record and aggregate digests can be recomputed after deserialization. A v2 snapshot is not silently accepted as v3.
