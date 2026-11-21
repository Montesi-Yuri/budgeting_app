# Data Model & Sync Protocol (Tauri, QR pairing, no cloud)

Terms in **bold capitals** (Account, Transfer, Planned item, ...) are defined in [CONTEXT.md](../CONTEXT.md). The reasons for the architecture are in [ADR 0001](adr/0001-tauri-with-direct-device-sync.md).

## Goals

- Windows PC and Android phone each keep a full local copy of the data.
- Devices pair once by scanning a QR code, then sync directly over the local network.
- A sync exchanges only what each side is missing, in both directions.
- Edits made offline on both devices merge without losing data.
- Traffic is authenticated and encrypted, because this is financial data.

## Architecture

```
┌──────────────── Tauri app (same code on Windows + Android) ────────────────┐
│  UI (React)                                                                │
│        │  invoke("...")            ▲ events ("sync://progress")             │
│  ──────▼───────────────────────────┴─────────────────────────────────────  │
│  Rust core                                                                 │
│   ├─ db/        rusqlite, migrations, repository functions                 │
│   ├─ model/     entities + Op + HLC + VersionVector                        │
│   ├─ sync/                                                                 │
│   │   ├─ pairing.rs   QR payload, one-time token                           │
│   │   ├─ secure.rs    Noise handshake (crate `snow`)                       │
│   │   ├─ protocol.rs  framed messages (CBOR via `ciborium`)                │
│   │   ├─ server.rs    TCP listener (PC)                                    │
│   │   └─ client.rs    TCP connector (phone)                                │
│   └─ commands.rs  #[tauri::command] functions exposed to UI                │
└────────────────────────────────────────────────────────────────────────────┘
```

All data and sync logic runs in the Rust core, which runs on Android as well. Only the UI is web code.

The PC listens and the phone connects. The phone is the side that scans and has a camera, and Android restricts background servers.

Crates and plugins:

| Need | Choice |
|---|---|
| SQLite | `rusqlite` (bundled feature) |
| IDs | `uuid` (v7 for new rows, v5 for imported rows) |
| Encryption / auth | `snow` (Noise protocol framework) |
| Serialization | `serde` + `ciborium` (CBOR) |
| Async networking | `tokio` |
| QR render (PC) | `qrcode` → SVG string sent to UI |
| QR scan (phone) | `tauri-plugin-barcode-scanner` (official, mobile) |
| Optional discovery | `mdns-sd` |
| Backup encryption | `age` (passphrase mode) |
| CSV parsing | `csv` + `encoding_rs` |

---

## 1. Data model

### 1.1 Principles

- **IDs are UUIDs, generated on the device**, so the two devices never need to coordinate.
- **Money is an integer in minor units**, e.g. `amount_minor = -1250` means -12.50. Never use floats.
- **Deletes are soft deletes** (`deleted = 1`, called a tombstone), so that deletions sync too.
- **Every write goes through one function** that updates the entity row *and* appends an op to the log, inside the same DB transaction.
- **Conflicts resolve by last-writer-wins per row**, ordered by a Hybrid Logical Clock (HLC). This keeps ordering correct even if the phone's and PC's clocks disagree.
- **Links are rows with a status.** Every Suggestion (a Transfer link, a Planned item fulfilment, a Manual Transaction match) is a row in its own table with `status = 'suggested' | 'confirmed' | 'rejected'`. The numbers count `suggested` the same as `confirmed`. A `rejected` row is kept, so the same pair is never suggested again.
- **The currency is a single setting**, not a column on each Account.
- **Derived things are not stored:** balances, Spendable, Forecast, Set-asides and Planned item occurrences that are still expected are all calculated from the stored rows. Only facts and user decisions sync.

### 1.2 Hybrid Logical Clock (HLC)

An HLC value is `(wall_ms: u64, counter: u32, device_id)`. It is stored as a sortable string:

```
"0001731900000123-0000-<device_id>"   // zero-padded so string order == logical order
```

- **On a local write:** `wall = max(now_ms, last.wall)`. Increment `counter` if `wall` didn't change, otherwise reset it to 0.
- **On receiving an op:** `last = max(last, op.hlc)`, then advance as above. This keeps the clock moving forward even if this device's clock is behind.
- `device_id` breaks ties, so two HLC values are never equal.

### 1.3 Tables

```sql
-- This device and its paired peers
CREATE TABLE device (
  device_id     TEXT PRIMARY KEY,          -- UUIDv4, created on first launch
  name          TEXT NOT NULL,             -- "Yuri-PC", "Pixel 8"
  static_pubkey BLOB NOT NULL,             -- Noise X25519 public key (32 bytes)
  is_self       INTEGER NOT NULL DEFAULT 0,
  last_addr     TEXT,                      -- "192.168.1.20:47321" (peers only)
  paired_at     INTEGER,
  last_sync_at  INTEGER
);
-- This device's private key: Windows Credential Manager on the PC,
-- Android Keystore-backed storage on the phone. Keep it out of plain SQLite if possible.

-- Every synced table below also has these two columns (omitted for brevity):
--   updated_hlc TEXT NOT NULL,
--   deleted     INTEGER NOT NULL DEFAULT 0

-- Synced key/value settings: 'currency' = "EUR", 'savings_target_minor' = 30000,
-- 'backup_dir' is NOT here (it is a per-device setting, stored locally only)
CREATE TABLE setting (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL              -- JSON
);

-- ===== Money and where it lives =====
CREATE TABLE account (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL,             -- 'current' | 'savings' | 'credit_card'
  opening_minor INTEGER NOT NULL,          -- Opening balance (card debt is negative)
  opening_date  TEXT NOT NULL,             -- 'YYYY-MM-DD': balance is as of this day
  is_main       INTEGER NOT NULL DEFAULT 0 -- the current account shown in the Forecast
);

CREATE TABLE category (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL,             -- 'income' | 'expense' (decides Income vs Expense)
  parent_id     TEXT REFERENCES category(id), -- max 2 levels: a parent can't have a parent
  color         TEXT,
  position      REAL NOT NULL              -- display order
);

CREATE TABLE txn (
  id              TEXT PRIMARY KEY,        -- v7 if manual, v5 if imported (see 1.5)
  account_id      TEXT NOT NULL REFERENCES account(id),
  date            TEXT NOT NULL,           -- 'YYYY-MM-DD'; card purchases: purchase date
  amount_minor    INTEGER NOT NULL,        -- signed, as seen from the Account
  description     TEXT NOT NULL,
  note            TEXT,
  source          TEXT NOT NULL,           -- 'import' | 'manual'
  import_id       TEXT REFERENCES import_batch(id),
  fingerprint     TEXT,                    -- imported rows only (see 1.5)
  balance_after_minor INTEGER,             -- from the CSV, if present: used to check balances
  category_id     TEXT REFERENCES category(id), -- NULL = Uncategorised
  category_source TEXT                     -- 'user' | 'rule' | NULL; Rules never overwrite 'user'
);
CREATE INDEX txn_account_date ON txn(account_id, date) WHERE deleted = 0;

-- A Transfer links an outgoing and an incoming txn. One side may be missing for now.
CREATE TABLE transfer (
  id            TEXT PRIMARY KEY,          -- from a match: UUIDv5(out_txn_id | in_txn_id), so both
                                           -- Devices suggesting the same pair create one row
  out_txn_id    TEXT REFERENCES txn(id),   -- NULL until that Account is imported
  in_txn_id     TEXT REFERENCES txn(id),   -- NULL until that Account is imported
  status        TEXT NOT NULL,             -- 'suggested' | 'confirmed' | 'rejected'
  origin        TEXT NOT NULL              -- 'rule' | 'match' | 'user'
);
-- A txn in a non-rejected transfer is neither Expense nor Income.
-- Rows marked by a Rule are created with status = 'confirmed'.

-- ===== Getting data in =====
CREATE TABLE import_profile (
  id              TEXT PRIMARY KEY,
  account_id      TEXT NOT NULL REFERENCES account(id),
  name            TEXT NOT NULL,           -- "Intesa current account CSV"
  encoding        TEXT NOT NULL,           -- 'utf-8' | 'windows-1252' | ...
  delimiter       TEXT NOT NULL,           -- ';' | ',' | '\t'
  skip_rows       INTEGER NOT NULL DEFAULT 0, -- lines before the header
  date_column     TEXT NOT NULL,
  date_format     TEXT NOT NULL,           -- '%d/%m/%Y'
  amount_mode     TEXT NOT NULL,           -- 'signed' (one column) | 'split' (debit + credit columns)
  amount_column   TEXT,                    -- for 'signed'
  debit_column    TEXT,                    -- for 'split'
  credit_column   TEXT,                    -- for 'split'
  decimal_sep     TEXT NOT NULL,           -- ',' | '.'
  thousands_sep   TEXT,                    -- '.' | ',' | NULL
  description_columns TEXT NOT NULL,       -- JSON list, joined with ' '
  balance_column  TEXT                     -- optional
);

CREATE TABLE import_batch (
  id            TEXT PRIMARY KEY,
  account_id    TEXT NOT NULL REFERENCES account(id),
  profile_id    TEXT NOT NULL REFERENCES import_profile(id),
  source_name   TEXT NOT NULL,             -- "estratto_2026_10.csv"
  imported_at   INTEGER NOT NULL,
  first_date    TEXT, last_date TEXT,
  rows_new      INTEGER NOT NULL,
  rows_skipped  INTEGER NOT NULL           -- already present (duplicates)
);
-- The original file is NOT synced (it's only an input). Only the resulting rows sync.

CREATE TABLE rule (
  id            TEXT PRIMARY KEY,
  position      REAL NOT NULL,             -- first match wins; REAL so reordering on two
                                           -- Devices doesn't renumber every row
  match_text    TEXT NOT NULL,             -- case-insensitive "contains" on description
  account_id    TEXT REFERENCES account(id), -- optional filter
  direction     TEXT,                      -- 'in' | 'out' | NULL (either)
  action        TEXT NOT NULL,             -- 'categorise' | 'transfer'
  category_id   TEXT REFERENCES category(id) -- for 'categorise'
);

-- An imported txn that duplicates a Manual Transaction
CREATE TABLE manual_match (
  id              TEXT PRIMARY KEY,
  manual_txn_id   TEXT NOT NULL REFERENCES txn(id),
  imported_txn_id TEXT NOT NULL REFERENCES txn(id),
  status          TEXT NOT NULL            -- 'suggested' | 'confirmed' | 'rejected'
);
-- While suggested/confirmed, the manual txn is hidden from all numbers (see 1.6).
-- Logically unique on (manual_txn_id, imported_txn_id); id = UUIDv5 of that pair,
-- so both Devices create the same row.

-- ===== Planning =====
CREATE TABLE planned_item (
  id            TEXT PRIMARY KEY,
  account_id    TEXT NOT NULL REFERENCES account(id),
  category_id   TEXT NOT NULL REFERENCES category(id), -- its kind says Income or Expense
  description   TEXT NOT NULL,
  amount_minor  INTEGER NOT NULL,          -- signed, as it will appear on the Account
  rrule         TEXT NOT NULL,             -- 'FREQ=MONTHLY;BYMONTHDAY=27', 'FREQ=YEARLY;BYMONTH=6;BYMONTHDAY=15'
  start_date    TEXT NOT NULL,
  end_date      TEXT,
  set_aside     INTEGER NOT NULL           -- 1 = spread into monthly Set-asides
                                           -- (default 1 when less frequent than monthly)
);

-- Only occurrences that are no longer simply "expected" are stored.
CREATE TABLE planned_occurrence (
  id              TEXT PRIMARY KEY,        -- UUIDv5(planned_item_id | due_date): both Devices
                                           -- produce the same id for the same occurrence
  planned_item_id TEXT NOT NULL REFERENCES planned_item(id),
  due_date        TEXT NOT NULL,
  state           TEXT NOT NULL,           -- 'fulfilled' | 'skipped' | 'later'
  txn_id          TEXT REFERENCES txn(id), -- for 'fulfilled'
  link_status     TEXT,                    -- 'suggested' | 'confirmed' (for 'fulfilled')
  later_date      TEXT                     -- for 'later': new expected date
);

CREATE TABLE budget (
  id            TEXT PRIMARY KEY,
  category_id   TEXT NOT NULL REFERENCES category(id), -- a parent covers its children
  per           TEXT NOT NULL,             -- 'period' | 'week'
  amount_minor  INTEGER NOT NULL,          -- positive limit
  valid_from    TEXT NOT NULL,
  valid_to      TEXT
);

-- ===== Sync log =====
CREATE TABLE op (
  origin        TEXT NOT NULL,             -- device_id that created the op
  seq           INTEGER NOT NULL,          -- 1,2,3... per origin, no gaps
  hlc           TEXT NOT NULL,
  entity        TEXT NOT NULL,             -- 'account' | 'category' | 'txn' | ...
  entity_id     TEXT NOT NULL,
  kind          TEXT NOT NULL,             -- 'upsert' | 'delete'
  payload       BLOB,                      -- CBOR of the full row (upsert only)
  PRIMARY KEY (origin, seq)
);
```

The **version vector** is never stored separately. It is derived from the op log:

```sql
SELECT origin, MAX(seq) FROM op GROUP BY origin;
-- e.g. { PC: 120, PHONE: 45 }  ==  "I have every op up to PC#120 and PHONE#45"
```

Because `seq` has no gaps per origin, "max seq" exactly describes which ops a device holds.

### 1.4 Write path and apply path (Rust)

```rust
pub struct Op {
    pub origin: Uuid, pub seq: u64, pub hlc: Hlc,
    pub entity: Entity, pub entity_id: Uuid,
    pub kind: OpKind,               // Upsert | Delete
    pub payload: Option<Vec<u8>>,   // CBOR row
}

/// Local edit: one DB transaction covers the row and its op.
fn write_local(tx: &Transaction, ent: Entity, row: &impl Row) -> Result<()> {
    let hlc = clock.tick();
    let seq = next_seq(tx, self_id)?;             // MAX(seq)+1 WHERE origin = self
    upsert_row(tx, ent, row, &hlc)?;
    insert_op(tx, &Op { origin: self_id, seq, hlc, entity: ent,
                        entity_id: row.id(), kind: OpKind::Upsert,
                        payload: Some(cbor(row)) })?;
    Ok(())
}

/// Remote op: idempotent, order-independent.
fn apply_remote(tx: &Transaction, op: &Op) -> Result<()> {
    clock.observe(&op.hlc);
    if !insert_op_if_absent(tx, op)? { return Ok(()); }   // already had it
    match current_hlc(tx, op.entity, op.entity_id)? {
        Some(cur) if cur >= op.hlc => {}                  // local is newer: keep it
        _ => match op.kind {
            OpKind::Upsert => upsert_row_from_payload(tx, op)?,
            OpKind::Delete => mark_deleted(tx, op.entity, op.entity_id, &op.hlc)?,
        },
    }
    Ok(())
}
```

A delete is an op with a newer HLC, so a later edit on the other device can "undelete" the row. This is a reasonable rule for a personal app.

### 1.5 Imports on both devices: deterministic IDs

If the same bank CSV is imported on the PC *and* on the phone, random UUIDs would create duplicates. To prevent this, imported transactions get a **deterministic** ID:

```
fingerprint = sha256(account_id | date | amount_minor | normalize(description) | n)
id          = UUIDv5(APP_NAMESPACE, fingerprint)
```

`n` is the occurrence index for identical lines on the same day (e.g. two 2.00 coffees). Both devices compute the same ID, so sync merges the two imports into one row. Before inserting, the import function skips rows whose `id` already exists. This also handles CSVs with overlapping date ranges on a single Device.

The import pipeline, in order:

1. Parse the file with the Account's `import_profile`, then compute `fingerprint` and `id`, and skip rows already present.
2. Apply the Rules in `position` order, skipping any txn whose `category_source = 'user'`. A `transfer` Rule creates a `transfer` row with `status = 'confirmed'`.
3. Find matches and create `suggested` rows:
   - **Transfers:** same absolute amount, opposite signs, different Accounts, within 3 days, where one side has no Transfer yet. This also completes a Transfer that so far has only one side.
   - **Planned items:** an expected occurrence in the same Account with the same sign, with a similar amount (±20% or an exact match) and a date within ±5 days.
   - **Manual Transactions:** a Manual Transaction in the same Account with the same amount (or ±1 for rounding) and a date within ±4 days.
4. If the profile has a `balance_column`, compare it with the calculated balance and warn about the first mismatch.

Pairs that were already `rejected` are never suggested again.

### 1.6 Merging a Manual Transaction

When a `manual_match` exists and is not `rejected`, the numbers ignore the manual txn and use the imported one. When the match is **confirmed**:

1. The imported txn takes over `category_id` (with `category_source = 'user'`) and the `note` from the manual txn.
2. The manual txn is soft-deleted.

Each step is a normal row write, so the merge syncs like any other change.

### 1.7 Calculated values (never stored)

These are pure functions over the tables above, recalculated after every write and every Sync batch.

- **Balance of an Account on day D:** `opening_minor` plus the sum of `amount_minor` for txns with `opening_date < date <= D`, excluding manual txns that are hidden by a match.
- **Expected occurrences:** expanded from `planned_item.rrule` for the range needed, minus those with a stored `planned_occurrence` (fulfilled or skipped), with `later` ones moved to `later_date`.
- **Set-aside per Period:** for each `planned_item` with `set_aside = 1`, its yearly total divided by 12.
- **Spendable for Period P**, all values signed, with expenses negative:
  ```
    Income txns in P            +  expected Income occurrences in P
  + Expense txns in P           +  expected Expense occurrences in P
  − Set-asides for P
  − Savings target
  ```
  - Transfers are excluded throughout.
  - Refunds are included as positive Expense txns.
  - Items with `set_aside = 1` appear only as their Set-aside. Their expected occurrences and the txns that fulfil them are left out of the two Expense terms, so the 600 insurance in June doesn't hit June twice.
- **Weekly Spendable:** `Spendable_remaining / days_left_in_period × days_left_in_week`.
- **Savings progress:** the Savings target compared with the sum of `in` Transfers into Accounts of kind `savings` during P.
- **Forecast** to the end of the Period and the end of the year:
  - Start from today's balance.
  - Add the expected occurrences of Planned items.
  - Add estimated everyday spending per expense Category: the Budget if one is set, otherwise the average of the last 3 Periods (Categories with Planned items excluded).
  - Mark it low confidence if there are fewer than 3 Periods of history.
  - Shown for the Net position (all Accounts) and for the `is_main` Account.

### 1.8 Log size

Budgeting data is small: thousands of ops per year, each a few hundred bytes. **Keep the log forever** at first. If it ever matters, a peer can compact the ops it knows every peer has acknowledged (the minimum over the peers' version vectors) down to one op per entity.

---

## 2. Pairing with a QR code (one time)

### 2.1 The QR payload (shown on the PC)

```jsonc
{
  "v": 1,
  "app": "budget-sync",
  "id": "8f1c…",                       // PC device_id
  "name": "Yuri-PC",
  "addrs": ["192.168.1.20", "10.0.0.5"], // all private IPv4s on the PC
  "port": 47321,
  "pk": "base64url(32-byte X25519 static public key)",
  "tok": "base64url(32 random bytes)",  // one-time pairing token
  "exp": 1731900300                     // unix seconds, now + 5 min
}
```

It is encoded as a string `bsync:1:<base64url(CBOR)>` to keep the QR small.

**Why this is secure:** the QR code is a channel an attacker can't easily intercept, since it needs line of sight. Through it the phone learns the PC's real public key, so a man-in-the-middle on the Wi-Fi can't impersonate the PC. The token proves to the PC that this phone actually scanned its screen.

### 2.2 Pairing flow

```
 PC (responder)                                  Phone (initiator)
 ─────────────                                   ─────────────────
 UI: "Pair new device"
 generate tok, exp; start listener
 show QR  ───────────── (camera) ──────────────▶ scan → parse payload
                                                 for addr in addrs: try TCP connect
             ◀──────── Noise IK handshake ─────── (phone already knows PC pk from QR)
             (PC learns phone static pk)
             ◀──────── PairRequest{tok, id, name}
 check tok matches && now < exp && unused
 store device(phone id, name, pk)
 invalidate tok
             ──────── PairOk{id, name} ─────────▶ store device(PC id, name, pk, last_addr)
             ═══════ immediately run first sync (section 3) ═══════
```

Noise pattern: **`Noise_IK_25519_ChaChaPoly_BLAKE2s`**. "IK" means the initiator knows the responder's key in advance, which the QR provides, and sends its own key encrypted during the handshake.

### 2.3 Later connections

The same Noise IK handshake is used with no token. After the handshake, the PC checks that the phone's static public key belongs to a paired `device` row. If not, it closes the connection. The phone checks that it reached the PC key it expected. No passwords or certificates are involved.

**Unpairing** deletes the peer's `device` row, after which that key is rejected.

---

## 3. Sync protocol

### 3.1 Transport

- TCP to the PC's `port`. The default is 47321; it is configurable, and the app adds a Windows Firewall rule for *private* networks only.
- After the Noise handshake, all data is sent as Noise transport messages. Each frame is `u16 length (big-endian) + ciphertext`, and the plaintext is capped at about 64 KB, which is Noise's limit.
- The plaintext of each frame is one CBOR `Msg`.

```rust
#[derive(Serialize, Deserialize)]
enum Msg {
    PairRequest { token: [u8; 32], device_id: Uuid, name: String },
    PairOk      { device_id: Uuid, name: String },
    Hello       { proto: u16, device_id: Uuid, schema: u32,
                  vv: BTreeMap<Uuid, u64> },          // my version vector
    Ops         { ops: Vec<Op>, last: bool },        // batches of ~200
    Done        { vv: BTreeMap<Uuid, u64> },         // my vv after applying yours
    Error       { code: ErrCode, msg: String },
}
```

### 3.2 Session

Both sides run the same logic at the same time. It is symmetric, so the PC and the phone each send and receive.

```
 Phone                                             PC
   │──── Hello{vv_phone} ─────────────────────────▶│
   │◀──────────────────────── Hello{vv_pc} ────────│
   │                                               │
   │  missing_for_pc = ops where                   │  missing_for_phone = ops where
   │    seq > vv_pc[origin] (0 if absent)          │    seq > vv_phone[origin]
   │  ordered by (origin, seq)                     │
   │                                               │
   │──── Ops{batch1} ─▶   ◀─ Ops{batch1} ──────────│   (interleaved; each side
   │──── Ops{…, last}─▶   ◀─ Ops{…, last} ─────────│    applies each batch in
   │                                               │    ONE DB transaction)
   │──── Done{vv'} ───────────────────────────────▶│
   │◀─────────────────────────────── Done{vv'} ────│
   close                                          close
```

The ops sent are selected with this query:

```sql
SELECT * FROM op
WHERE seq > COALESCE(:peer_vv_for_origin, 0) AND origin = :origin
-- run once per origin in the local vv; UNION over origins, ORDER BY origin, seq
```

Properties:

- **Resumable:** each batch is committed as it arrives. If the Wi-Fi drops halfway, the next Hello reports the new version vector and the transfer continues from there.
- **Idempotent:** `(origin, seq)` is the primary key, so receiving the same op twice does nothing.
- **Works for more than 2 devices:** the version vector covers all origins, so a phone can relay a laptop's ops to the PC.
- **Checked at the end:** after `Done`, each side may verify that `peer_vv' == my_vv`, and log a warning if they differ.

### 3.3 Versioning

- `proto`: if the major versions differ, reply `Error{IncompatibleProtocol}`.
- `schema`: the DB migration number. If the peer's schema is newer than mine, refuse with `Error{UpdateApp}`, because I couldn't apply its payloads. If the peer's schema is older, I can still accept its ops; missing fields take their defaults.

### 3.4 When sync runs

- A "Sync now" button calls the command `sync_now(peer_id)`.
- The phone also syncs automatically when the app comes to the foreground, if the PC is reachable (connect timeout of about 1.5 s).
- The PC listener is enabled from a "Allow sync" toggle in settings, so the port isn't open all the time.
- If the stored `last_addr` fails, the phone tries mDNS (`_bsync._tcp.local`) if implemented. Otherwise the user can rescan the QR code. A rescan only refreshes the address; the keys stay the same.

---

## 4. Backup

A Backup is the **whole op log** (every op of every origin), serialised as CBOR and encrypted with `age` using a passphrase. It contains no `device` rows and no keys: not the Device's own private key, and not the paired peers either, since a restored Device has to be paired again anyway. The code is `core/src/backup.rs`.

- **Export:** the "Export backup" command writes `budget-YYYY-MM-DD.bsync.age` to a file the user chooses.
- **Automatic (PC only):** once a week, on app start or while the app is running, the PC writes the same file to the per-device `backup_dir`, which can be a OneDrive folder. It keeps the last 8 backups. `backup_dir` lives in the `local_setting` table (per-device, never synced or backed up), and "once a week" means the newest `budget-YYYY-MM-DD.bsync.age` in the folder is at least 7 days old. The passphrase is stored in Windows Credential Manager, so the automatic backup needs no prompt.
- **Restore:** decrypt the file, then run `apply_remote` for every op, in HLC order and in one DB transaction. A wrong passphrase changes nothing. Restore is generic over the schema: `op.entity` is the table name, the row is rebuilt from the field names of the CBOR payload (= column names) and the primary key is read from SQLite, so new synced tables need no backup code. Because ops are idempotent, a Backup can be restored into an empty DB (a new or reinstalled Device) or into an existing DB. Restoring into an existing DB only adds what's missing. The restoring Device keeps its own `device_id`, and its peers need to be paired again.

## 5. Tauri commands (UI ↔ Rust)

```rust
#[tauri::command] fn pairing_start() -> PairingQr;          // { svg, expires_at }  (PC)
#[tauri::command] fn pairing_cancel();
#[tauri::command] async fn pairing_join(payload: String) -> PeerInfo; // (phone, after scan)
#[tauri::command] fn peers_list() -> Vec<PeerInfo>;
#[tauri::command] fn peer_remove(device_id: Uuid);
#[tauri::command] async fn sync_now(device_id: Uuid) -> SyncReport; // {sent, received, ms}
#[tauri::command] fn sync_server_set(enabled: bool);         // PC
#[tauri::command] fn backup_export(path: String, passphrase: String);
#[tauri::command] fn backup_restore(path: String, passphrase: String) -> RestoreReport;
#[tauri::command] fn backup_schedule_set(dir: Option<String>); // PC; None = off
// The domain commands (import_preview, import_commit, suggestions_list,
// suggestion_decide, spendable_get, forecast_get, ...) are designed with the first slice.
// progress: app.emit("sync://progress", {sent, received, phase})
// after a remote batch is applied: app.emit("data://changed", {entities})  -> UI refetches
```

Android configuration:

- Add the `INTERNET` permission.
- Add the barcode-scanner plugin's camera permission.
- No cleartext-HTTP configuration is needed, because this is raw TCP with its own encryption.

---

## 6. Edge cases checklist

| Case | Handling |
|---|---|
| Same row edited on both devices offline | Higher HLC wins for the whole row. The losing version still exists in the op log, so a "conflicts" view could be added later. |
| Edit on A, delete on B | Whichever has the later HLC wins |
| Same CSV imported on both | Deterministic UUIDv5 IDs merge the rows (1.5) |
| Category deleted while B adds a txn using it | The txn keeps its `category_id`. The UI shows the deleted category as "(deleted) Groceries" or "Uncategorized". |
| Device clock wrong by hours | The HLC only moves forward, so ordering is still consistent. Only "who wins" may differ from what a human expects. |
| Phone reinstalled (new device_id, empty DB) | Pair again. The first sync then sends the full history. The old device row can be removed. |
| PC IP changes (DHCP) | Try mDNS, otherwise rescan the QR |
| Connection drops mid-sync | Batches are committed as they arrive; the next sync resumes (3.2) |
| Lost phone | Unpair it on the PC, and its key is rejected from then on |
| Suggestion confirmed on one Device, rejected on the other | It's one row, so the later decision (by HLC) wins |
| Same Planned item occurrence marked on both Devices | The occurrence id is deterministic (UUIDv5), so it's one row and the later write wins |
| Same Manual Transaction matched to the import on both Devices | The match id is a UUIDv5 of the pair, so both Devices write the same row and the later write wins |
| Rules reordered on both Devices | Each Rule has its own `position`, so only Rules moved on both sides conflict, and the later move wins |
| Category used by a Rule or Budget is deleted | The Rule or Budget stays but is shown as broken and is not applied |
| A Transfer still has only one side at the end of the Period | It still counts as a Transfer. It's listed under review as "other side not imported yet". |

## 7. Build order

The first slice is **PC only** (see CONTEXT.md). Sync is not built yet, but every write already goes through `write_local`, so the op log exists from the first day.

1. Schema, migrations, HLC, `write_local` / `apply_remote`. Unit test: apply the same ops in different orders and check that both DBs end up identical.
2. Accounts, Categories, Opening balances, Manual Transactions.
3. Import profiles, the import pipeline (1.5), Rules, Transfers and Suggestions.
4. Planned items, Set-asides, Savings target and Spendable / Weekly Spendable (1.7).
5. Forecast.
6. Backup export, restore and the weekly schedule.
7. Sync: an in-process test with two SQLite files exchanging `Msg`s through channels, then TCP and Noise between two desktop instances, with pairing done by pasting the QR payload as text.
8. Android build: QR rendering on the PC and the barcode-scanner plugin on the phone, Manual Transactions on the phone.
9. Auto-sync on app foreground, mDNS, and the firewall rule.
