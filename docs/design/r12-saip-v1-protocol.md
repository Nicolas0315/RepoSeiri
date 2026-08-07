# R12-SAIP-v1: Semantic Appeal Integrity Protocol

## 日本語

### 1. Triggerと権限

```text
R12-SAIP-v1でB0-B11を一括実装してください。
```

このtriggerはB0-B11のrepository mutationとlocal verificationを許可します。commit、push、merge、release、publication、visibility change、plugin install、restartは許可しません。

### 2. 順次実行

1. HEAD、worktree、contract、query count、wire identifier、既知failureをbaseline化します。
2. B0からB11をdependency順に実行し、同時に`in_progress`にするbatchは一つだけです。
3. 各batchでcoherent diff、focused test、semantic differential、privacy scan、Labyrinth critiqueを実行します。
4. compile、test、lint failureは同じbatch内で修復します。blocking checkをskipまたはpassへ変換しません。
5. B3、B6、B9、B11の後にworkspace group gateを実行します。
6. user-owned changeをrevert、stash、resetしません。
7. geometry on/off、full/incrementalでclaim意味が変わる場合は該当する依存coneを停止します。
8. 最後にbatch状態、source digest、verification receipt、calibrationとhostの境界を報告します。

同じsourceに対する全required local checkが通過した終端状態を`ready_for_git`と呼びます。これはGit操作権限ではありません。

### 3. 停止条件

通常のfailureでは全体を停止しません。private data leak、権限不足、破壊的曖昧性、解消不能なuser change衝突、Unknownのcollapse、証拠上限違反、10 queryまたは`seiri.codex.v2`の破壊、同じhard blockerが3回続く場合だけ、affected dependency coneを停止します。

### 4. Privacyとclaim境界

ledgerは`target/r12-saip/<execution-id>/`へ置けますが、source body、diff body、private analysis名・本文、private calibration値・digest、host absolute path、credentialを保存しません。local test、host receipt、calibration、manual policy、performance evidenceは別claimです。

---

## English

### 1. Trigger And Authority

```text
Implement B0-B11 as one batch under R12-SAIP-v1.
```

The trigger authorizes repository mutation and local verification for B0-B11. It does not authorize commit, push, merge, release, publication, visibility changes, plugin installation, or restart.

### 2. Sequential Execution

1. Baseline HEAD, the worktree, contracts, query count, wire identifier, and known failures.
2. Execute B0 through B11 in dependency order with at most one batch `in_progress`.
3. For each batch, run a coherent diff, focused tests, semantic differentials, privacy scans, and Labyrinth critique.
4. Repair compile, test, and lint failures inside the same batch. Never skip a blocking check or convert failure into pass.
5. Run workspace group gates after B3, B6, B9, and B11.
6. Never revert, stash, or reset user-owned changes.
7. Stop the affected dependency cone if geometry on/off or full/incremental computation changes claim meaning.
8. Report batch states, source digests, verification receipts, and calibration and host boundaries.

The terminal state for all required local checks passing against the same source is `ready_for_git`. It is not Git-operation authority.

### 3. Stop Conditions

Ordinary failures do not stop the whole run. Stop the affected dependency cone only for a private-data leak, missing authority, destructive ambiguity, an irreconcilable user-change conflict, Unknown collapse, an evidence-ceiling violation, breakage of the ten queries or `seiri.codex.v2`, or the same hard blocker repeated three times.

### 4. Privacy And Claim Boundary

The ledger may live under `target/r12-saip/<execution-id>/`, but it stores no source body, diff body, private-analysis name or body, private-calibration value or digest, host absolute path, or credential. Local tests, host receipts, calibration, manual policy, and performance evidence remain separate claims.
