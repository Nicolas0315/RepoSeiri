# R13-SAIP-v2: Semantic Alignment Implementation Protocol

## 日本語

### 1. Triggerと権限

```text
R13-SAIP-v2でB0-B11を4レーン、順序通り実装してください。
```

このtriggerはB0-B11のsource、test、fixture、公開文書の変更と必要なlocal verificationを許可します。commit、push、merge、release、publication、visibility change、plugin installation、restartは許可しません。

### 2. 実行規則

1. B0からB11を依存順に実行し、同時に`in_progress`にするbatchは一つだけとします。
2. batch内部は最大4レーンで並列化できます。
   - Lane A: IR、型、digest
   - Lane B: parser、analyzer
   - Lane C: schema、query、launcher
   - Lane D: fixture、test、benchmark
3. 次batchへ進む前にtargeted test、semantic differential、Unknown regression、ceiling regression、query compatibilityを実行します。
4. B3、B6、B9、B11の後にworkspace group gateを実行します。
5. blocking checkをskip、passへ変換、または非blockingとして扱いません。
6. user-owned changeをrevert、stash、resetしません。
7. 通常の局所failureは同じbatchで修復します。固定不変条件を壊すfailureではaffected dependency coneだけを停止します。

### 3. 停止コーン

| Failure | Stop cone |
| --- | --- |
| claim semantic mismatch | B2、B4-B7、B11のclaim部分 |
| Unknown collapse | 全claim生成とREADME変更 |
| evidence ceiling regression | B4-B7、B11 |
| digest nondeterminism | portable、incremental、parallel、public example |
| ten-query breakage | B9、B11、plugin integration |
| `seiri.codex.v2` outer-wire breakage | B8以降の公開surface |
| scope escape | actions、portable compare、plugin surface |
| geometry semantic influence | geometry依存コーン全体 |
| performance regression | 該当最適化のみ |

### 4. Privacyとledger

実行ledgerを置く場合は`target/r13-saip/<execution-id>/`を使用します。source body、diff body、private analysis名・本文、private calibration値・digest、host absolute path、credentialを保存しません。

### 5. 終端状態

全required local gateが同じsourceに対して通過した場合だけ`ready_for_git`とします。それ以外は`incomplete`です。`ready_for_git`はGit操作権限ではありません。

---

## English

### 1. Trigger And Authority

```text
Implement B0-B11 with four lanes in dependency order under R13-SAIP-v2.
```

This trigger authorizes changes to B0-B11 source, tests, fixtures, public documentation, and required local verification. It does not authorize commit, push, merge, release, publication, visibility changes, plugin installation, or restart.

### 2. Execution Rules

1. Execute B0 through B11 in dependency order with at most one batch `in_progress`.
2. A batch may use up to four parallel lanes.
   - Lane A: IR, types, and digests
   - Lane B: parsers and analyzers
   - Lane C: schemas, queries, and launchers
   - Lane D: fixtures, tests, and benchmarks
3. Before advancing, run targeted tests, semantic differentials, Unknown regression, ceiling regression, and query compatibility checks.
4. Run workspace group gates after B3, B6, B9, and B11.
5. Never skip a blocking check, convert it to pass, or relabel it nonblocking.
6. Never revert, stash, or reset user-owned changes.
7. Repair ordinary local failures within the same batch. A fixed-invariant failure stops only the affected dependency cone.

### 3. Stop Cones

| Failure | Stop cone |
| --- | --- |
| claim semantic mismatch | claim-dependent work in B2, B4-B7, and B11 |
| Unknown collapse | all claim generation and README changes |
| evidence-ceiling regression | B4-B7 and B11 |
| digest nondeterminism | portable, incremental, parallel, and public-example work |
| ten-query breakage | B9, B11, and plugin integration |
| `seiri.codex.v2` outer-wire breakage | public surfaces from B8 onward |
| scope escape | actions, portable comparison, and plugin surfaces |
| geometry semantic influence | the entire geometry dependency cone |
| performance regression | the affected optimization only |

### 4. Privacy And Ledger

If an execution ledger is used, place it under `target/r13-saip/<execution-id>/`. Store no source body, diff body, private-analysis name or body, private-calibration value or digest, host absolute path, or credential.

### 5. Terminal States

Use `ready_for_git` only after every required local gate passes against the same source. Otherwise use `incomplete`. `ready_for_git` is not Git-operation authority.
