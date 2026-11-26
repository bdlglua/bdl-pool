//! Private coordinator API and durable append-only ledger for collective mining.
//!
//! The public statistics API remains read-only. This service is bound to
//! loopback and is reached through the HTTPS reverse proxy. Only a trusted
//! Stratum/backend token may record accepted shares or mature block rewards.
//! Miner bearer tokens are read-only and scoped to one participant.

use crate::{
    address,
    collective::{plan_pplns_reward, AcceptedShare, PayoutMode, SUPPORTED_COINS},
    http::{Request, Response},
};
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::{
    fd::AsRawFd,
    unix::fs::{OpenOptionsExt, PermissionsExt},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::Mutex,
};

const MAX_LEDGER_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SHARES: usize = 1_000_000;
const MAX_PARTICIPANTS: usize = 10_000;
const MAX_BLOCKS: usize = 100_000;
const MAX_EVENT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub struct CoordinatorError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}
impl CoordinatorError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    fn response(self) -> Response {
        Response::error(self.status, self.code, &self.message)
    }
}
type Result<T> = std::result::Result<T, CoordinatorError>;

fn io_error(message: &str) -> CoordinatorError {
    CoordinatorError::new(503, "ledger_unavailable", message)
}
fn invalid(message: &str) -> CoordinatorError {
    CoordinatorError::new(400, "invalid_request", message)
}
fn valid_id(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}
fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}
fn random_hex(bytes: usize) -> io::Result<String> {
    let mut random = vec![0_u8; bytes];
    getrandom::getrandom(&mut random).map_err(io::Error::other)?;
    Ok(hex(&random))
}
fn sha256_hex(value: &str) -> String {
    hex(&Sha256::digest(value.as_bytes()))
}
fn constant_time_equal(left: &str, right: &str) -> bool {
    let a = left.as_bytes();
    let b = right.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |diff, (x, y)| diff | (*x ^ *y)) == 0
}
fn bearer(request: &Request) -> Option<&str> {
    request.header("authorization")?.strip_prefix("Bearer ")
}
fn authorized(request: &Request, expected: &str) -> bool {
    bearer(request)
        .map(|given| constant_time_equal(given, expected))
        .unwrap_or(false)
}
fn parse_body(request: &Request) -> Result<Value> {
    if !request
        .header("content-type")
        .map(|v| v.eq_ignore_ascii_case("application/json"))
        .unwrap_or(false)
    {
        return Err(invalid("Content-Type must be application/json."));
    }
    if request.body.is_empty() || request.body.len() > 32_768 {
        return Err(invalid("A JSON request body is required."));
    }
    let value: Value = serde_json::from_slice(&request.body)
        .map_err(|_| invalid("The request body is not valid JSON."))?;
    if !value.is_object() {
        return Err(invalid("A JSON object is required."));
    }
    Ok(value)
}
fn required_str<'a>(value: &'a Value, key: &str, max: usize) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= max)
        .ok_or_else(|| invalid("A required string field is missing or invalid."))
}
fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("A required positive integer field is missing or invalid."))
}
fn conflict(message: &str) -> CoordinatorError {
    CoordinatorError::new(409, "conflict", message)
}
fn unauthorized() -> CoordinatorError {
    CoordinatorError::new(401, "unauthorized", "A valid bearer token is required.")
}

struct LedgerState {
    next_sequence: u64,
    payout_mode: PayoutMode,
    participants: BTreeMap<String, Value>,
    shares: BTreeMap<String, Vec<AcceptedShare>>,
    share_ids: BTreeSet<String>,
    candidates: BTreeMap<String, Value>,
    blocks: BTreeMap<String, Value>,
    share_count: usize,
}
impl LedgerState {
    fn new(mode: PayoutMode) -> Self {
        Self {
            next_sequence: 1,
            payout_mode: mode,
            participants: BTreeMap::new(),
            shares: BTreeMap::new(),
            share_ids: BTreeSet::new(),
            candidates: BTreeMap::new(),
            blocks: BTreeMap::new(),
            share_count: 0,
        }
    }
    fn block_key(coin: &str, block_id: &str) -> String {
        format!("{coin}:{block_id}")
    }
    fn apply_event(&mut self, event: &Value) -> io::Result<()> {
        match event["type"].as_str() {
            Some("participant_created") => {
                let p = event
                    .get("participant")
                    .ok_or_else(|| io::Error::other("missing participant"))?;
                let id = p["id"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("missing participant ID"))?;
                if self.participants.contains_key(id) || self.participants.len() >= MAX_PARTICIPANTS
                {
                    return Err(io::Error::other(
                        "duplicate or excessive participant records",
                    ));
                }
                self.participants.insert(id.to_string(), p.clone());
            }
            Some("share_accepted") => {
                let s = event
                    .get("share")
                    .ok_or_else(|| io::Error::other("missing share"))?;
                let sequence = s["sequence"]
                    .as_u64()
                    .ok_or_else(|| io::Error::other("invalid share sequence"))?;
                let share_id = s["shareId"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid share ID"))?;
                let participant_id = s["participantId"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid participant ID"))?;
                let coin = s["coin"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid coin"))?;
                let work_units = s["workUnits"]
                    .as_u64()
                    .ok_or_else(|| io::Error::other("invalid work units"))?;
                if sequence != self.next_sequence
                    || !self
                        .participants
                        .get(participant_id)
                        .map(|p| p["coin"] == coin)
                        .unwrap_or(false)
                    || self.share_ids.contains(&format!("{coin}:{share_id}"))
                    || work_units == 0
                    || self.share_count >= MAX_SHARES
                {
                    return Err(io::Error::other("inconsistent accepted-share event"));
                }
                let share = AcceptedShare {
                    sequence,
                    share_id: share_id.to_string(),
                    participant_id: participant_id.to_string(),
                    work_units,
                };
                self.shares.entry(coin.to_string()).or_default().push(share);
                self.share_ids.insert(format!("{coin}:{share_id}"));
                self.share_count += 1;
                self.next_sequence = self
                    .next_sequence
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("share sequence exhausted"))?;
            }
            Some("block_candidate_recorded") => {
                let candidate = event
                    .get("candidate")
                    .ok_or_else(|| io::Error::other("missing block candidate"))?;
                let coin = candidate["coin"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid candidate coin"))?;
                let block_id = candidate["blockId"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid candidate block ID"))?;
                let cutoff = candidate["shareSequenceCutoff"]
                    .as_u64()
                    .ok_or_else(|| io::Error::other("invalid candidate share cutoff"))?;
                if !SUPPORTED_COINS.contains(&coin)
                    || !valid_id(block_id, 128)
                    || candidate["height"].as_u64().is_none()
                    || candidate["rewardAtoms"]
                        .as_u64()
                        .filter(|n| *n > 0)
                        .is_none()
                    || cutoff == 0
                    || cutoff >= self.next_sequence
                {
                    return Err(io::Error::other("inconsistent block candidate event"));
                }
                let key = Self::block_key(coin, block_id);
                if self.candidates.contains_key(&key)
                    || self.blocks.contains_key(&key)
                    || self.candidates.len() >= MAX_BLOCKS
                {
                    return Err(io::Error::other("duplicate or excessive block candidates"));
                }
                let has_work = self
                    .shares
                    .get(coin)
                    .map(|shares| shares.iter().any(|share| share.sequence <= cutoff))
                    .unwrap_or(false);
                if !has_work {
                    return Err(io::Error::other(
                        "block candidate has no eligible accepted work",
                    ));
                }
                self.candidates.insert(key, candidate.clone());
            }
            Some("block_candidate_orphaned") => {
                let coin = event["coin"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid orphan coin"))?;
                let block_id = event["blockId"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid orphan block ID"))?;
                let key = Self::block_key(coin, block_id);
                let candidate = self
                    .candidates
                    .get_mut(&key)
                    .ok_or_else(|| io::Error::other("orphaned candidate does not exist"))?;
                if candidate["status"] != "pending" || self.blocks.contains_key(&key) {
                    return Err(io::Error::other("candidate cannot be orphaned"));
                }
                candidate["status"] = Value::String("orphaned".into());
                candidate["orphanedAt"] = event["orphanedAt"].clone();
            }
            Some("block_recorded") => {
                let b = event
                    .get("block")
                    .ok_or_else(|| io::Error::other("missing block"))?;
                let coin = b["coin"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid block coin"))?;
                let block_id = b["blockId"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid block ID"))?;
                let key = Self::block_key(coin, block_id);
                if self.blocks.contains_key(&key) || self.blocks.len() >= MAX_BLOCKS {
                    return Err(io::Error::other("duplicate or excessive block records"));
                }
                if let Some(candidate) = self.candidates.get_mut(&key) {
                    if candidate["status"] != "pending"
                        || candidate["rewardAtoms"] != b["rewardAtoms"]
                        || candidate["shareSequenceCutoff"] != b["shareSequenceCutoff"]
                    {
                        return Err(io::Error::other("block does not match its saved candidate"));
                    }
                    candidate["status"] = Value::String("matured".into());
                }
                self.blocks.insert(key, b.clone());
            }
            Some("payout_mode_changed") => {
                let mode = event["mode"]
                    .as_str()
                    .and_then(PayoutMode::parse)
                    .ok_or_else(|| io::Error::other("invalid payout mode"))?;
                self.payout_mode = mode;
            }
            _ => return Err(io::Error::other("unknown ledger event")),
        }
        Ok(())
    }
}

struct Journal {
    file: File,
    state: LedgerState,
}
impl Journal {
    fn open(path: &Path, initial_mode: PayoutMode) -> io::Result<Self> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("ledger path has no parent"))?;
        fs::create_dir_all(parent)?;
        let parent_meta = fs::symlink_metadata(parent)?;
        if !parent_meta.is_dir() || parent_meta.file_type().is_symlink() {
            return Err(io::Error::other(
                "ledger directory must be a real directory",
            ));
        }
        #[cfg(unix)]
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let mut options = OpenOptions::new();
        options.read(true).append(true).create(true);
        #[cfg(unix)]
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("ledger path is not a regular file"));
        }
        #[cfg(unix)]
        {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
            // Keep the exclusive lock for the lifetime of this service process.
            let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if rc != 0 {
                return Err(io::Error::other(
                    "another coordinator process owns the ledger",
                ));
            }
        }
        let size = file.metadata()?.len();
        if size > MAX_LEDGER_BYTES {
            return Err(io::Error::other("ledger reached its configured size limit"));
        }
        let mut state = LedgerState::new(initial_mode);
        let input = File::open(path)?;
        let mut reader = BufReader::new(input);
        let mut line = Vec::new();
        loop {
            line.clear();
            let bytes = reader.read_until(b'\n', &mut line)?;
            if bytes == 0 {
                break;
            }
            if line.len() > MAX_EVENT_BYTES {
                return Err(io::Error::other("ledger contains an oversized event"));
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let event: Value = serde_json::from_slice(&line)
                .map_err(|_| io::Error::other("ledger contains invalid JSON"))?;
            state.apply_event(&event)?;
        }
        Ok(Self { file, state })
    }

    fn append(&mut self, event: &Value) -> io::Result<()> {
        let mut encoded = serde_json::to_vec(event)?;
        if encoded.len() > MAX_EVENT_BYTES {
            return Err(io::Error::other("ledger event exceeds size limit"));
        }
        encoded.push(b'\n');
        let current = self.file.metadata()?.len();
        if current.saturating_add(encoded.len() as u64) > MAX_LEDGER_BYTES {
            return Err(io::Error::other(
                "ledger storage limit reached; archive/compact before accepting more work",
            ));
        }
        self.file.write_all(&encoded)?;
        self.file.sync_data()?;
        self.state.apply_event(event)
    }
}

pub struct Coordinator {
    admin_token: String,
    backend_token: String,
    journal: Mutex<Journal>,
}
impl Coordinator {
    pub fn open(config: Value) -> io::Result<Self> {
        let path = config["ledgerPath"]
            .as_str()
            .filter(|s| !s.is_empty() && !s.contains(['\n', '\r', '\0']))
            .ok_or_else(|| io::Error::other("missing ledgerPath"))?;
        let admin_token = config["adminToken"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let backend_token = config["backendToken"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if !valid_secret(&admin_token)
            || !valid_secret(&backend_token)
            || constant_time_equal(&admin_token, &backend_token)
        {
            return Err(io::Error::other(
                "adminToken and backendToken must be distinct 32-byte lowercase hex tokens",
            ));
        }
        let mode = config["payoutMode"]
            .as_str()
            .and_then(PayoutMode::parse)
            .unwrap_or(PayoutMode::Manual);
        let journal = Journal::open(Path::new(path), mode)?;
        Ok(Self {
            admin_token,
            backend_token,
            journal: Mutex::new(journal),
        })
    }

    pub fn dispatch(&self, request: &Request) -> Response {
        match self.route(request) {
            Ok(value) => Response::json(200, value),
            Err(error) => error.response(),
        }
    }

    fn route(&self, request: &Request) -> Result<Value> {
        if request.target.contains('?') || request.target.contains('#') {
            return Err(invalid(
                "Query strings and fragments are not accepted on coordinator routes.",
            ));
        }
        let path = request.target.as_str();
        match (request.method.as_str(), path) {
            ("GET", "/api/v1/collective/status") => self.public_status(),
            ("GET", "/api/v1/collective/me") => self.participant_status(request),
            ("GET", "/api/v1/collective/admin/participants") => self.admin_participants(request),
            ("GET", "/api/v1/collective/admin/blocks") => self.admin_blocks(request),
            ("POST", "/api/v1/collective/admin/participants") => self.create_participant(request),
            ("POST", "/api/v1/collective/admin/payout-mode") => self.change_payout_mode(request),
            ("POST", "/api/v1/collective/internal/authorize") => self.authorize_miner(request),
            ("POST", "/api/v1/collective/internal/shares") => self.record_share(request),
            ("POST", "/api/v1/collective/internal/candidates") => self.record_candidate(request),
            ("GET", "/api/v1/collective/internal/candidates") => self.pending_candidates(request),
            ("POST", "/api/v1/collective/internal/candidates/orphan") => {
                self.orphan_candidate(request)
            }
            ("POST", "/api/v1/collective/internal/blocks") => self.record_block(request),
            _ => Err(CoordinatorError::new(
                404,
                "not_found",
                "Coordinator endpoint not found.",
            )),
        }
    }

    fn public_status(&self) -> Result<Value> {
        let ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let coins: Vec<Value> = SUPPORTED_COINS.iter().map(|coin| {
            let participants = ledger.state.participants.values()
                .filter(|p| p["coin"] == *coin && p["active"] == true).count();
            let shares = ledger.state.shares.get(*coin).map(Vec::len).unwrap_or(0);
            let work_units: u128 = ledger.state.shares.get(*coin)
                .into_iter().flatten().map(|s| s.work_units as u128).sum();
            json!({"coin":coin,"participants":participants,"acceptedShares":shares,"acceptedWorkUnits":work_units})
        }).collect();
        Ok(json!({
            "status":"ok",
            "schemaVersion":1,
            "payoutMode":ledger.state.payout_mode.as_str(),
            "collectiveMiningReady":false,
            "shareIngress":"trusted-backend-only",
            "coins":coins
        }))
    }

    fn create_participant(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.admin_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let coin = required_str(&body, "coin", 8)?;
        if !SUPPORTED_COINS.contains(&coin) {
            return Err(invalid("Unsupported coin profile."));
        }
        let raw_address = required_str(&body, "payoutAddress", 128)?;
        let address = address::canonical_coin(raw_address, coin).ok_or_else(|| {
            invalid("The payout address is invalid for the selected mainnet coin.")
        })?;
        let mut ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        if ledger.state.participants.len() >= MAX_PARTICIPANTS {
            return Err(CoordinatorError::new(
                507,
                "participant_limit",
                "Participant capacity reached.",
            ));
        }
        let id =
            random_hex(16).map_err(|_| io_error("Unable to generate a participant identifier."))?;
        let token =
            random_hex(32).map_err(|_| io_error("Unable to generate a participant token."))?;
        let stratum_password =
            random_hex(32).map_err(|_| io_error("Unable to generate a Stratum password."))?;
        let participant = json!({
            "id":id,
            "coin":coin,
            "payoutAddress":address,
            "tokenHash":sha256_hex(&token),
            "stratumPasswordHash":sha256_hex(&stratum_password),
            "active":true,
            "createdAt":Utc::now().to_rfc3339()
        });
        let event = json!({"type":"participant_created","participant":participant});
        ledger.append(&event).map_err(|_| {
            io_error("The participant could not be committed to the durable ledger.")
        })?;
        Ok(json!({
            "participant":{"id":id,"coin":coin,"payoutAddress":address,"active":true},
            "participantToken":token,
            "stratumPassword":stratum_password,
            "warning":"Both secrets are shown once. Use participantToken only for the private dashboard API. Use stratumPassword as the Stratum V1 password; it is separate and grants mining access only."
        }))
    }

    fn change_payout_mode(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.admin_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let mode_name = required_str(&body, "payoutMode", 16)?;
        let mode = PayoutMode::parse(mode_name)
            .ok_or_else(|| invalid("payoutMode must be automatic or manual."))?;
        let mut ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        if ledger.state.payout_mode != mode {
            let event = json!({"type":"payout_mode_changed","mode":mode.as_str(),"changedAt":Utc::now().to_rfc3339()});
            ledger
                .append(&event)
                .map_err(|_| io_error("Payout mode could not be committed."))?;
        }
        Ok(json!({"payoutMode":mode.as_str(),"paymentsEnabled":false}))
    }

    fn authorize_miner(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.backend_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let participant_id = required_str(&body, "participantId", 64)?;
        let password = required_str(&body, "stratumPassword", 64)?;
        if !valid_id(participant_id, 64) || !valid_secret(password) {
            return Ok(json!({"authorized":false}));
        }
        let password_hash = sha256_hex(password);
        let ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let Some(participant) = ledger.state.participants.get(participant_id) else {
            return Ok(json!({"authorized":false}));
        };
        let accepted = participant["active"] == true
            && participant["coin"] == "btc"
            && constant_time_equal(
                participant["stratumPasswordHash"].as_str().unwrap_or(""),
                &password_hash,
            );
        Ok(json!({
            "authorized":accepted,
            "participantId":if accepted { participant_id } else { "" },
            "coin":if accepted { "btc" } else { "" }
        }))
    }

    fn record_share(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.backend_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let coin = required_str(&body, "coin", 8)?;
        let share_id = required_str(&body, "shareId", 128)?;
        let participant_id = required_str(&body, "participantId", 64)?;
        let work_units = required_u64(&body, "workUnits")?;
        if !SUPPORTED_COINS.contains(&coin)
            || !valid_id(share_id, 128)
            || !valid_id(participant_id, 64)
        {
            return Err(invalid("Share identity or coin is invalid."));
        }
        let mut ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        if !ledger
            .state
            .participants
            .get(participant_id)
            .map(|p| p["coin"] == coin && p["active"] == true)
            .unwrap_or(false)
        {
            return Err(conflict("No active participant exists for this coin."));
        }
        if ledger
            .state
            .share_ids
            .contains(&format!("{coin}:{share_id}"))
        {
            return Err(conflict(
                "This accepted share identifier has already been recorded.",
            ));
        }
        if ledger.state.share_count >= MAX_SHARES {
            return Err(CoordinatorError::new(
                507,
                "share_limit",
                "Share ledger capacity reached.",
            ));
        }
        let sequence = ledger.state.next_sequence;
        let share = json!({
            "sequence":sequence,
            "shareId":share_id,
            "participantId":participant_id,
            "coin":coin,
            "workUnits":work_units,
            "acceptedAt":Utc::now().to_rfc3339()
        });
        let event = json!({"type":"share_accepted","share":share});
        ledger.append(&event).map_err(|_| {
            io_error("Accepted share could not be committed to the durable ledger.")
        })?;
        Ok(json!({"accepted":true,"sequence":sequence,"coin":coin,"workUnits":work_units}))
    }

    fn record_candidate(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.backend_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let coin = required_str(&body, "coin", 8)?;
        let block_id = required_str(&body, "blockId", 128)?;
        let height = required_u64(&body, "height")?;
        let reward_atoms = required_u64(&body, "rewardAtoms")?;
        if !SUPPORTED_COINS.contains(&coin) || !valid_id(block_id, 128) {
            return Err(invalid("Block candidate identity or coin is invalid."));
        }
        let mut ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let key = LedgerState::block_key(coin, block_id);
        if let Some(existing) = ledger.state.candidates.get(&key) {
            if existing["height"] == height
                && existing["rewardAtoms"] == reward_atoms
                && existing["status"] == "pending"
            {
                return Ok(existing.clone());
            }
            return Err(conflict(
                "This block candidate already exists with different data or status.",
            ));
        }
        if ledger.state.blocks.contains_key(&key) {
            return Err(conflict("This block already has a mature allocation."));
        }
        let share_sequence_cutoff = ledger.state.next_sequence.saturating_sub(1);
        let has_work = ledger
            .state
            .shares
            .get(coin)
            .map(|shares| {
                shares
                    .iter()
                    .any(|share| share.sequence <= share_sequence_cutoff)
            })
            .unwrap_or(false);
        if !has_work {
            return Err(conflict(
                "No accepted work exists for this coin at candidate discovery.",
            ));
        }
        if ledger.state.candidates.len() >= MAX_BLOCKS {
            return Err(CoordinatorError::new(
                507,
                "candidate_limit",
                "Block candidate capacity reached.",
            ));
        }
        let candidate = json!({
            "coin":coin,
            "blockId":block_id,
            "height":height,
            "rewardAtoms":reward_atoms,
            "shareSequenceCutoff":share_sequence_cutoff,
            "status":"pending",
            "discoveredAt":Utc::now().to_rfc3339()
        });
        let event = json!({"type":"block_candidate_recorded","candidate":candidate});
        ledger.append(&event).map_err(|_| {
            io_error("Block candidate could not be committed to the durable ledger.")
        })?;
        Ok(candidate)
    }

    fn pending_candidates(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.backend_token) {
            return Err(unauthorized());
        }
        let ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let candidates: Vec<Value> = ledger
            .state
            .candidates
            .values()
            .filter(|candidate| candidate["status"] == "pending")
            .cloned()
            .collect();
        Ok(json!({"candidates":candidates,"total":candidates.len()}))
    }

    fn orphan_candidate(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.backend_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let coin = required_str(&body, "coin", 8)?;
        let block_id = required_str(&body, "blockId", 128)?;
        if !SUPPORTED_COINS.contains(&coin) || !valid_id(block_id, 128) {
            return Err(invalid("Block candidate identity or coin is invalid."));
        }
        let mut ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let key = LedgerState::block_key(coin, block_id);
        let Some(candidate) = ledger.state.candidates.get(&key) else {
            return Err(CoordinatorError::new(
                404,
                "candidate_not_found",
                "Block candidate not found.",
            ));
        };
        if candidate["status"] == "orphaned" {
            return Ok(candidate.clone());
        }
        if candidate["status"] != "pending" || ledger.state.blocks.contains_key(&key) {
            return Err(conflict(
                "A mature or already allocated candidate cannot be orphaned.",
            ));
        }
        let event = json!({
            "type":"block_candidate_orphaned",
            "coin":coin,
            "blockId":block_id,
            "orphanedAt":Utc::now().to_rfc3339()
        });
        ledger
            .append(&event)
            .map_err(|_| io_error("Orphaned block candidate could not be committed."))?;
        Ok(ledger
            .state
            .candidates
            .get(&key)
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn record_block(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.backend_token) {
            return Err(unauthorized());
        }
        let body = parse_body(request)?;
        let coin = required_str(&body, "coin", 8)?;
        let block_id = required_str(&body, "blockId", 128)?;
        let reward_atoms = required_u64(&body, "rewardAtoms")?;
        let window = required_u64(&body, "windowWorkUnits")?;
        if !SUPPORTED_COINS.contains(&coin) || !valid_id(block_id, 128) {
            return Err(invalid("Block identifier or coin is invalid."));
        }
        if body["mature"] != true {
            return Err(conflict(
                "A block can only be recorded after the coin backend confirms maturity.",
            ));
        }
        let mut ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let key = LedgerState::block_key(coin, block_id);
        if let Some(existing) = ledger.state.blocks.get(&key) {
            if existing["rewardAtoms"] == reward_atoms
                && existing["windowWorkUnits"] == window
                && existing["mature"] == true
            {
                return Ok(existing.clone());
            }
            return Err(conflict(
                "This coin/block identifier already exists with different reward data.",
            ));
        }
        if ledger.state.blocks.len() >= MAX_BLOCKS {
            return Err(CoordinatorError::new(
                507,
                "block_limit",
                "Block ledger capacity reached.",
            ));
        }
        let candidate = ledger.state.candidates.get(&key).cloned().ok_or_else(|| {
            conflict("Block has no saved candidate snapshot; PPLNS cutoff cannot be verified.")
        })?;
        if candidate["status"] != "pending" || candidate["rewardAtoms"] != reward_atoms {
            return Err(conflict(
                "Mature block does not match its saved candidate snapshot.",
            ));
        }
        let share_sequence_cutoff = candidate["shareSequenceCutoff"]
            .as_u64()
            .ok_or_else(|| io_error("Saved block candidate has an invalid share cutoff."))?;
        let shares = ledger
            .state
            .shares
            .get(coin)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|share| share.sequence <= share_sequence_cutoff)
            .collect::<Vec<_>>();
        let plan = plan_pplns_reward(
            coin,
            block_id,
            reward_atoms,
            ledger.state.payout_mode,
            window as u128,
            &shares,
        )
        .map_err(|error| conflict(&error.to_string()))?;
        let allocations: Vec<Value> = plan.payouts.iter().map(|p| {
            json!({"participantId":p.participant_id,"workUnits":p.work_units,"amountAtoms":p.amount_atoms})
        }).collect();
        let block = json!({
            "coin":coin,
            "blockId":block_id,
            "rewardAtoms":reward_atoms,
            "windowWorkUnits":window,
            "shareSequenceCutoff":share_sequence_cutoff,
            "accountedWorkUnits":plan.accounted_work_units,
            "payoutMode":plan.payout_mode.as_str(),
            "mature":true,
            "status":"awaiting_settlement",
            "allocations":allocations,
            "recordedAt":Utc::now().to_rfc3339(),
            "paymentsEnabled":false
        });
        let event = json!({"type":"block_recorded","block":block});
        ledger.append(&event).map_err(|_| {
            io_error("Mature block reward could not be committed to the durable ledger.")
        })?;
        Ok(block)
    }

    fn participant_status(&self, request: &Request) -> Result<Value> {
        let token = bearer(request)
            .filter(|v| valid_secret(v))
            .ok_or_else(unauthorized)?;
        let token_hash = sha256_hex(token);
        let ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let participant = ledger
            .state
            .participants
            .values()
            .find(|p| constant_time_equal(p["tokenHash"].as_str().unwrap_or(""), &token_hash))
            .ok_or_else(unauthorized)?;
        let id = participant["id"].as_str().unwrap_or_default();
        let coin = participant["coin"].as_str().unwrap_or_default();
        let shares = ledger
            .state
            .shares
            .get(coin)
            .into_iter()
            .flatten()
            .filter(|s| s.participant_id == id)
            .collect::<Vec<_>>();
        let accepted_work_units: u128 = shares.iter().map(|s| s.work_units as u128).sum();
        let pending_atoms: u128 = ledger
            .state
            .blocks
            .values()
            .filter(|b| b["coin"] == coin && b["status"] == "awaiting_settlement")
            .flat_map(|b| b["allocations"].as_array().into_iter().flatten())
            .filter(|p| p["participantId"] == id)
            .map(|p| p["amountAtoms"].as_u64().unwrap_or(0) as u128)
            .sum();
        Ok(json!({
            "participant":{"id":id,"coin":coin,"payoutAddress":participant["payoutAddress"],"active":participant["active"]},
            "acceptedShares":shares.len(),
            "acceptedWorkUnits":accepted_work_units,
            "pendingRewardAtoms":pending_atoms,
            "payoutMode":ledger.state.payout_mode.as_str()
        }))
    }

    fn admin_participants(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.admin_token) {
            return Err(unauthorized());
        }
        let ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let participants: Vec<Value> = ledger.state.participants.values().map(|p| {
            json!({"id":p["id"],"coin":p["coin"],"payoutAddress":p["payoutAddress"],"active":p["active"],"createdAt":p["createdAt"]})
        }).collect();
        Ok(json!({"participants":participants,"total":participants.len()}))
    }

    fn admin_blocks(&self, request: &Request) -> Result<Value> {
        if !authorized(request, &self.admin_token) {
            return Err(unauthorized());
        }
        let ledger = self
            .journal
            .lock()
            .map_err(|_| io_error("Ledger lock is unavailable."))?;
        let blocks: Vec<Value> = ledger.state.blocks.values().cloned().collect();
        Ok(json!({"blocks":blocks,"total":blocks.len()}))
    }
}

fn valid_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Create the first coordinator config exactly once. Both secrets are 256-bit
/// random tokens, written mode 0600; they are printed once for the operator.
pub fn init_config(config_path: &Path, ledger_path: &Path) -> io::Result<(String, String)> {
    if config_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "coordinator config already exists; refusing to replace secrets",
        ));
    }
    let parent = config_path
        .parent()
        .ok_or_else(|| io::Error::other("config path has no parent"))?;
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let admin = random_hex(32)?;
    let backend = random_hex(32)?;
    let config = json!({
        "ledgerPath":ledger_path,
        "adminToken":admin,
        "backendToken":backend,
        "payoutMode":"manual"
    });
    let encoded = serde_json::to_vec_pretty(&config)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options.open(config_path)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    Ok((admin, backend))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn config(root: &Path) -> Value {
        json!({
            "ledgerPath":root.join("state/ledger.jsonl"),
            "adminToken":"a".repeat(64),
            "backendToken":"b".repeat(64),
            "payoutMode":"manual"
        })
    }
    fn request(method: &str, target: &str, token: &str, body: Value) -> Request {
        Request {
            method: method.into(),
            target: target.into(),
            headers: vec![
                ("authorization".into(), format!("Bearer {token}")),
                ("content-type".into(), "application/json".into()),
            ],
            body: serde_json::to_vec(&body).unwrap(),
        }
    }
    #[test]
    fn ledger_persists_participants_and_accepted_shares_across_restart() {
        let root = TempDir::new().unwrap();
        let cfg = config(root.path());
        let coordinator = Coordinator::open(cfg.clone()).unwrap();
        let created = coordinator.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/participants",
            &"a".repeat(64),
            json!({"coin":"btc","payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"}),
        ));
        assert_eq!(created.status, 200);
        let response = created.data;
        let id = response["participant"]["id"].as_str().unwrap().to_string();
        let token = response["participantToken"].as_str().unwrap().to_string();
        let accepted = coordinator.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/shares",
            &"b".repeat(64),
            json!({"coin":"btc","shareId":"share-001","participantId":id,"workUnits":250}),
        ));
        assert_eq!(accepted.data["accepted"], true);
        drop(coordinator);
        let reopened = Coordinator::open(cfg).unwrap();
        let me = reopened.dispatch(&request("GET", "/api/v1/collective/me", &token, json!({})));
        assert_eq!(me.data["acceptedShares"], 1);
        assert_eq!(me.data["acceptedWorkUnits"], 250);
    }

    #[test]
    fn stratum_password_is_separate_from_private_dashboard_token() {
        let root = TempDir::new().unwrap();
        let c = Coordinator::open(config(root.path())).unwrap();
        let created = c.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/participants",
            &"a".repeat(64),
            json!({"coin":"btc","payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"}),
        ));
        assert_eq!(created.status, 200);
        let id = created.data["participant"]["id"].as_str().unwrap();
        let participant_token = created.data["participantToken"].as_str().unwrap();
        let stratum_password = created.data["stratumPassword"].as_str().unwrap();
        assert_ne!(participant_token, stratum_password);
        let authorize = |password: &str| {
            c.dispatch(&request(
                "POST",
                "/api/v1/collective/internal/authorize",
                &"b".repeat(64),
                json!({"participantId":id,"stratumPassword":password}),
            ))
        };
        assert_eq!(authorize(stratum_password).data["authorized"], true);
        assert_eq!(authorize(participant_token).data["authorized"], false);
        assert_eq!(authorize(&"c".repeat(64)).data["authorized"], false);
        let wrong_backend = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/authorize",
            &"d".repeat(64),
            json!({"participantId":id,"stratumPassword":stratum_password}),
        ));
        assert_eq!(wrong_backend.status, 401);
    }

    #[test]
    fn only_backend_can_record_shares_and_duplicate_shares_are_rejected() {
        let root = TempDir::new().unwrap();
        let c = Coordinator::open(config(root.path())).unwrap();
        let created = c.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/participants",
            &"a".repeat(64),
            json!({"coin":"btc","payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"}),
        ));
        let id = created.data["participant"]["id"].as_str().unwrap();
        let denied = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/shares",
            &"c".repeat(64),
            json!({"coin":"btc","shareId":"s1","participantId":id,"workUnits":1}),
        ));
        assert_eq!(denied.status, 401);
        let good = request(
            "POST",
            "/api/v1/collective/internal/shares",
            &"b".repeat(64),
            json!({"coin":"btc","shareId":"s1","participantId":id,"workUnits":1}),
        );
        assert_eq!(c.dispatch(&good).status, 200);
        assert_eq!(c.dispatch(&good).status, 409);
    }

    #[test]
    fn payouts_modes_are_persisted_and_block_records_require_maturity() {
        let root = TempDir::new().unwrap();
        let c = Coordinator::open(config(root.path())).unwrap();
        let created = c.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/participants",
            &"a".repeat(64),
            json!({"coin":"ppc","payoutAddress":"P8o5K6h8v6cK2LqF2VY1HfR8tTjPZqf8gB"}),
        ));
        // This address is a format fixture only; a real checksum is required.
        assert_eq!(created.status, 400);
        let mode = c.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/payout-mode",
            &"a".repeat(64),
            json!({"payoutMode":"automatic"}),
        ));
        assert_eq!(mode.data["payoutMode"], "automatic");
        let block = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/blocks",
            &"b".repeat(64),
            json!({"coin":"btc","blockId":"block-1","rewardAtoms":100,"windowWorkUnits":100,"mature":false}),
        ));
        assert_eq!(block.status, 409);
    }

    #[test]
    fn block_reward_uses_the_share_cutoff_saved_when_candidate_was_discovered() {
        let root = TempDir::new().unwrap();
        let cfg = config(root.path());
        let c = Coordinator::open(cfg.clone()).unwrap();
        let create = || {
            c.dispatch(&request(
                "POST",
                "/api/v1/collective/admin/participants",
                &"a".repeat(64),
                json!({"coin":"btc","payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"}),
            ))
        };
        let alice = create().data["participant"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let bob = create().data["participant"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let accepted_alice = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/shares",
            &"b".repeat(64),
            json!({"coin":"btc","shareId":"before-block","participantId":alice,"workUnits":40}),
        ));
        assert_eq!(accepted_alice.data["sequence"], 1);

        let candidate = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/candidates",
            &"b".repeat(64),
            json!({"coin":"btc","blockId":"block-2","height":100,"rewardAtoms":100}),
        ));
        assert_eq!(candidate.status, 200);
        assert_eq!(candidate.data["shareSequenceCutoff"], 1);

        let accepted_bob = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/shares",
            &"b".repeat(64),
            json!({"coin":"btc","shareId":"after-block","participantId":bob,"workUnits":60}),
        ));
        assert_eq!(accepted_bob.data["sequence"], 2);

        let block = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/blocks",
            &"b".repeat(64),
            json!({
                "coin":"btc","blockId":"block-2","rewardAtoms":100,
                "windowWorkUnits":100,"mature":true
            }),
        ));
        assert_eq!(block.status, 200);
        assert_eq!(block.data["shareSequenceCutoff"], 1);
        assert_eq!(block.data["allocations"].as_array().unwrap().len(), 1);
        assert_eq!(block.data["allocations"][0]["participantId"], alice);
        assert_eq!(block.data["allocations"][0]["amountAtoms"], 100);
        assert_eq!(block.data["status"], "awaiting_settlement");
        drop(c);

        let reopened = Coordinator::open(cfg).unwrap();
        let candidates = reopened.dispatch(&request(
            "GET",
            "/api/v1/collective/internal/candidates",
            &"b".repeat(64),
            json!({}),
        ));
        assert_eq!(candidates.data["total"], 0);
    }

    #[test]
    fn orphaned_candidate_is_removed_from_pending_queue_and_survives_restart() {
        let root = TempDir::new().unwrap();
        let cfg = config(root.path());
        let c = Coordinator::open(cfg.clone()).unwrap();
        let created = c.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/participants",
            &"a".repeat(64),
            json!({"coin":"btc","payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"}),
        ));
        let id = created.data["participant"]["id"].as_str().unwrap();
        let share = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/shares",
            &"b".repeat(64),
            json!({"coin":"btc","shareId":"orphan-share","participantId":id,"workUnits":1}),
        ));
        assert_eq!(share.status, 200);
        let candidate = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/candidates",
            &"b".repeat(64),
            json!({"coin":"btc","blockId":"orphan-block","height":101,"rewardAtoms":50}),
        ));
        assert_eq!(candidate.status, 200);
        let orphan = c.dispatch(&request(
            "POST",
            "/api/v1/collective/internal/candidates/orphan",
            &"b".repeat(64),
            json!({"coin":"btc","blockId":"orphan-block"}),
        ));
        assert_eq!(orphan.status, 200);
        assert_eq!(orphan.data["status"], "orphaned");
        drop(c);

        let reopened = Coordinator::open(cfg).unwrap();
        let pending = reopened.dispatch(&request(
            "GET",
            "/api/v1/collective/internal/candidates",
            &"b".repeat(64),
            json!({}),
        ));
        assert_eq!(pending.data["total"], 0);
    }

    #[test]
    fn public_status_does_not_expose_addresses_or_tokens() {
        let root = TempDir::new().unwrap();
        let c = Coordinator::open(config(root.path())).unwrap();
        let _ = c.dispatch(&request(
            "POST",
            "/api/v1/collective/admin/participants",
            &"a".repeat(64),
            json!({"coin":"btc","payoutAddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT"}),
        ));
        let response = c.dispatch(&request("GET", "/api/v1/collective/status", "", json!({})));
        assert_eq!(response.data["status"], "ok");
        assert!(!response.data.to_string().contains("1Boat"));
        assert!(!response.data.to_string().contains(&"a".repeat(64)));
    }
}
