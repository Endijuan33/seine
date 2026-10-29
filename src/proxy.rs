//! Farm proxy: rigs on a local network share one upstream pool session.
//!
//! Each rig gets its own slice of the upstream job's nonce range, so the pool
//! sees one session while every rig hashes distinct nonces. Shares are checked
//! against the submitting rig's slice before they go upstream, and the pool's
//! answer is routed back to that rig. Rigs top up their slice with `refill`;
//! the proxy asks the pool for more range the same way, or reconnects for
//! fresh work when the pool does not offer `refill`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::miner::ui::{info, success, warn};
use crate::pool::{
    PoolClient, PoolEvent, PoolJob, PoolNotification, POOL_NOTIFICATION_MINER_BLOCK_FOUND,
    POOL_NOTIFICATION_POOL_BLOCK_SOLVED, STRATUM_CAPABILITY_NONCE_REFILL_V1,
};

const MAX_RIG_MESSAGE_BYTES: usize = 8 * 1024;
const CAP_SUBMIT_CLAIMED_HASH: &str = "submit_claimed_hash";
const CAP_SAME_TEMPLATE_REBIND_V1: &str = "same_template_rebind_v1";
const CAP_POW_BLAKE2B_V1: &str = "pow_blake2b_v1";
const FARADAY_UPGRADE_MESSAGE: &str =
    "this proxy is mining the Faraday upgrade; update Seine to a release that supports it";
/// A rig's first slice is this fraction of an upstream range.
const INITIAL_SLICE_DIVISOR: u64 = 128;
/// A rig's slice never grows past this fraction of an upstream range.
const MAX_SLICE_DIVISOR: u64 = 4;
/// A rig that finishes a slice faster than this gets a slice twice as large.
const QUICK_REFILL: Duration = Duration::from_secs(30);
/// A rig that takes longer than this gets a slice half as large.
const SLOW_REFILL: Duration = Duration::from_secs(10 * 60);
/// An unanswered upstream refill is asked for again after this long.
const UPSTREAM_REFILL_RETRY: Duration = Duration::from_secs(15);
const SUMMARY_INTERVAL: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_secs(1);

pub fn run(cfg: &Config, shutdown: Arc<AtomicBool>) -> Result<()> {
    let listen = cfg
        .proxy_listen
        .clone()
        .ok_or_else(|| anyhow!("--proxy-listen is required for the farm proxy"))?;
    let pool_url = cfg
        .pool_url
        .clone()
        .ok_or_else(|| anyhow!("the farm proxy needs --pool-url"))?;
    let destination = cfg
        .mining_address
        .clone()
        .ok_or_else(|| anyhow!("the farm proxy needs --address"))?;
    let address = crate::shortname::ShortnameResolver::default().resolve(&destination)?;
    if address != destination.trim() {
        info("ADDRESS", format!("resolved {destination} to {address}"));
    }
    let worker = cfg
        .pool_worker
        .clone()
        .unwrap_or_else(|| "proxy".to_string());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("seine-proxy")
        .build()
        .context("failed to start the proxy runtime")?;
    runtime.block_on(async {
        let listener = TcpListener::bind(&listen)
            .await
            .with_context(|| format!("failed to listen on {listen}"))?;
        serve(
            listener,
            pool_url,
            cfg.pool_proxy.clone(),
            address,
            worker,
            shutdown,
        )
        .await
    })
}

async fn serve(
    listener: TcpListener,
    pool_url: String,
    pool_proxy: Option<crate::pool::SocksProxy>,
    address: String,
    worker: String,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let local = listener
        .local_addr()
        .map_or_else(|_| "?".to_string(), |addr| addr.to_string());
    let upstream = Arc::new(PoolClient::connect_via(
        &pool_url,
        pool_proxy.as_ref(),
        address.clone(),
        worker.clone(),
        Arc::clone(&shutdown),
    )?);
    success(
        "PROXY",
        format!(
            "listening on {local} for rigs; mining to {} via {pool_url} as {worker}",
            compact_address(&address)
        ),
    );

    let (hub_tx, hub_rx) = mpsc::unbounded_channel::<HubMsg>();
    spawn_upstream_bridge(Arc::clone(&upstream), hub_tx.clone(), Arc::clone(&shutdown));
    let ticker_tx = hub_tx.clone();
    let ticker_shutdown = Arc::clone(&shutdown);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(TICK);
        while !ticker_shutdown.load(Ordering::Relaxed) {
            tick.tick().await;
            if ticker_tx.send(HubMsg::Tick).is_err() {
                break;
            }
        }
    });
    let accept_tx = hub_tx.clone();
    tokio::spawn(async move {
        let mut next_conn = 0u64;
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    next_conn += 1;
                    tokio::spawn(serve_rig(next_conn, stream, peer, accept_tx.clone()));
                }
                Err(err) => {
                    warn("PROXY", format!("accept failed: {err}"));
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    });
    drop(hub_tx);

    let mut hub = Hub::new(upstream, pool_url, address);
    hub.run(hub_rx, shutdown).await;
    Ok(())
}

/// Forwards upstream pool events into the hub.
fn spawn_upstream_bridge(
    upstream: Arc<PoolClient>,
    hub_tx: mpsc::UnboundedSender<HubMsg>,
    shutdown: Arc<AtomicBool>,
) {
    let _ = std::thread::Builder::new()
        .name("proxy-upstream".to_string())
        .spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                if let Some(event) = upstream.recv_event_timeout(Duration::from_millis(200)) {
                    if hub_tx.send(HubMsg::Upstream(event)).is_err() {
                        break;
                    }
                }
            }
        });
}

async fn serve_rig(
    conn: u64,
    stream: TcpStream,
    peer: SocketAddr,
    hub_tx: mpsc::UnboundedSender<HubMsg>,
) {
    let _ = stream.set_nodelay(true);
    let (read_half, mut write_half) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    if hub_tx.send(HubMsg::Connected { conn, peer, tx }).is_err() {
        return;
    }
    let writer = tokio::spawn(async move {
        while let Some(mut line) = rx.recv().await {
            line.push('\n');
            if write_half.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
        let _ = write_half.shutdown().await;
    });
    let mut reader = BufReader::new(read_half);
    while let Ok(Some(line)) =
        crate::pool::read_line_limited_async(&mut reader, MAX_RIG_MESSAGE_BYTES).await
    {
        if hub_tx.send(HubMsg::Line { conn, line }).is_err() {
            break;
        }
    }
    let _ = hub_tx.send(HubMsg::Closed { conn });
    let _ = writer.await;
}

enum HubMsg {
    Upstream(PoolEvent),
    Connected {
        conn: u64,
        peer: SocketAddr,
        tx: mpsc::UnboundedSender<String>,
    },
    Line {
        conn: u64,
        line: String,
    },
    Closed {
        conn: u64,
    },
    Tick,
}

/// One upstream nonce range: the job the pool assigned and how much of it
/// has been handed to rigs.
#[derive(Debug, Clone)]
struct Assignment {
    upstream_job_id: String,
    target: String,
    difficulty: Option<u64>,
    start: u64,
    end: u64,
    next: u64,
    exhausted: bool,
    /// The pool replaced this range; shares on it go stale after its grace.
    retired: bool,
}

impl Assignment {
    fn from_job(job: &PoolJob) -> Self {
        Self {
            upstream_job_id: job.job_id.clone(),
            target: job.target.clone(),
            difficulty: job.difficulty,
            start: job.nonce_start,
            end: job.nonce_end,
            next: job.nonce_start,
            exhausted: job.nonce_end < job.nonce_start,
            retired: false,
        }
    }

    fn remaining(&self) -> u64 {
        if self.exhausted || self.retired {
            0
        } else {
            self.end - self.next + 1
        }
    }

    /// Takes up to `size` nonces from the front of the unissued range.
    fn take(&mut self, size: u64) -> Option<(u64, u64)> {
        let available = self.remaining();
        if available == 0 || size == 0 {
            return None;
        }
        let start = self.next;
        let end = start + size.min(available) - 1;
        if end == self.end {
            self.exhausted = true;
        } else {
            self.next = end + 1;
        }
        Some((start, end))
    }
}

/// Upstream work for one block template.
#[derive(Debug)]
struct Template {
    generation: u64,
    height: u64,
    template_key: String,
    header_base: String,
    network_target: Option<String>,
    template_id: Option<String>,
    range_size: u64,
    assignments: Vec<Assignment>,
    refill_requested_at: Option<Instant>,
    /// The upstream session this work came from has ended.
    dead: bool,
}

impl Template {
    fn from_job(generation: u64, job: &PoolJob) -> Self {
        Self {
            generation,
            height: job.height,
            template_key: template_key(job),
            header_base: job.header_base.clone(),
            network_target: job.network_target.clone(),
            template_id: job.template_id.clone(),
            range_size: job.nonce_count().max(1),
            assignments: vec![Assignment::from_job(job)],
            refill_requested_at: None,
            dead: false,
        }
    }

    fn remaining(&self) -> u64 {
        self.assignments
            .iter()
            .map(Assignment::remaining)
            .fold(0u64, u64::saturating_add)
    }
}

fn template_key(job: &PoolJob) -> String {
    match job.template_id.as_deref() {
        Some(id) => format!("{}:{id}", job.height),
        None => format!("{}:{}", job.height, job.header_base),
    }
}

/// A slice of upstream range given to one rig under a proxy job id.
#[derive(Debug, Clone)]
struct LocalJob {
    conn: u64,
    generation: u64,
    assignment: usize,
    start: u64,
    end: u64,
}

#[derive(Debug)]
struct Rig {
    tx: mpsc::UnboundedSender<String>,
    peer: SocketAddr,
    worker: String,
    logged_in: bool,
    blake2b: bool,
    slice_size: u64,
    current_job: Option<String>,
    slice_issued_at: Instant,
    accepted: u64,
    rejected: u64,
}

impl Rig {
    fn send(&self, message: Value) {
        let _ = self.tx.send(message.to_string());
    }
}

#[derive(Debug)]
enum Waiter {
    /// Logged in with no work yet.
    Job(u64),
    /// Asked to refill; `Value` is its request id.
    Refill(u64, Value),
}

#[derive(Debug, Default)]
struct Window {
    accepted: u64,
    rejected: u64,
    difficulty: u128,
}

struct Hub {
    upstream: Arc<PoolClient>,
    upstream_url: String,
    address: String,
    upstream_logged_in: bool,
    upstream_refill: bool,
    /// The most recent upstream job id, which a refill request must name.
    last_upstream_job: Option<String>,
    generation: u64,
    current: Option<Template>,
    previous: Option<Template>,
    rigs: HashMap<u64, Rig>,
    local_jobs: HashMap<String, LocalJob>,
    next_local_job: u64,
    pending_acks: HashMap<(String, u64), VecDeque<(u64, Value)>>,
    waiting: VecDeque<Waiter>,
    window: Window,
    last_summary: Instant,
    warned_addresses: HashSet<String>,
}

impl Hub {
    fn new(upstream: Arc<PoolClient>, upstream_url: String, address: String) -> Self {
        Self {
            upstream,
            upstream_url,
            address,
            upstream_logged_in: false,
            upstream_refill: false,
            last_upstream_job: None,
            generation: 0,
            current: None,
            previous: None,
            rigs: HashMap::new(),
            local_jobs: HashMap::new(),
            next_local_job: 0,
            pending_acks: HashMap::new(),
            waiting: VecDeque::new(),
            window: Window::default(),
            last_summary: Instant::now(),
            warned_addresses: HashSet::new(),
        }
    }

    async fn run(&mut self, mut rx: mpsc::UnboundedReceiver<HubMsg>, shutdown: Arc<AtomicBool>) {
        while let Some(message) = rx.recv().await {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            match message {
                HubMsg::Upstream(event) => self.on_upstream(event),
                HubMsg::Connected { conn, peer, tx } => {
                    self.rigs.insert(
                        conn,
                        Rig {
                            tx,
                            peer,
                            worker: peer.to_string(),
                            logged_in: false,
                            blake2b: false,
                            slice_size: 0,
                            current_job: None,
                            slice_issued_at: Instant::now(),
                            accepted: 0,
                            rejected: 0,
                        },
                    );
                }
                HubMsg::Line { conn, line } => self.on_rig_line(conn, &line),
                HubMsg::Closed { conn } => self.on_rig_closed(conn),
                HubMsg::Tick => self.on_tick(),
            }
        }
    }

    // ---- upstream -------------------------------------------------------

    fn on_upstream(&mut self, event: PoolEvent) {
        match event {
            PoolEvent::Connected => {}
            PoolEvent::LoginAccepted(ack) => {
                self.upstream_logged_in = true;
                self.upstream_refill = ack.supports(STRATUM_CAPABILITY_NONCE_REFILL_V1);
                success("PROXY", "upstream pool login accepted");
            }
            PoolEvent::LoginRejected(message) => {
                self.upstream_logged_in = false;
                warn("PROXY", format!("upstream pool rejected login: {message}"));
            }
            PoolEvent::Disconnected(message) => {
                self.upstream_logged_in = false;
                warn("PROXY", format!("upstream: {message}"));
                // Work from the ended session cannot be submitted any more.
                if let Some(mut current) = self.current.take() {
                    current.dead = true;
                    self.previous = Some(current);
                }
            }
            PoolEvent::Job(job) => self.on_upstream_job(job),
            PoolEvent::SubmitAck(ack) => {
                let key = (ack.job_id.clone(), ack.nonce);
                let Some(queue) = self.pending_acks.get_mut(&key) else {
                    return;
                };
                let Some((conn, request_id)) = queue.pop_front() else {
                    return;
                };
                if queue.is_empty() {
                    self.pending_acks.remove(&key);
                }
                let Some(rig) = self.rigs.get_mut(&conn) else {
                    return;
                };
                if ack.accepted {
                    rig.accepted += 1;
                    self.window.accepted += 1;
                    self.window.difficulty += u128::from(ack.difficulty.unwrap_or(0));
                    rig.send(json!({
                        "id": request_id,
                        "status": "ok",
                        "result": {"accepted": true, "difficulty": ack.difficulty},
                    }));
                } else {
                    rig.rejected += 1;
                    self.window.rejected += 1;
                    let reason = ack.error.unwrap_or_else(|| "rejected".to_string());
                    rig.send(json!({"id": request_id, "error": reason}));
                }
            }
            PoolEvent::SubmitRetry(_) => {}
            PoolEvent::Notification(notification) => self.on_upstream_notification(notification),
        }
    }

    fn on_upstream_notification(&mut self, notification: PoolNotification) {
        let kind = notification.kind.as_str();
        if kind == POOL_NOTIFICATION_POOL_BLOCK_SOLVED
            || kind == POOL_NOTIFICATION_MINER_BLOCK_FOUND
        {
            success("POOL", &notification.message);
            let message = json!({
                "method": "notification",
                "params": {"kind": notification.kind, "message": notification.message},
            });
            for rig in self.rigs.values().filter(|rig| rig.logged_in) {
                rig.send(message.clone());
            }
        } else {
            info("POOL", &notification.message);
        }
    }

    fn on_upstream_job(&mut self, job: PoolJob) {
        enum Fit {
            /// The pool re-issued a range we hold under a new job id.
            Reissue {
                generation: u64,
                index: usize,
                target_changed: bool,
                retired_others: bool,
            },
            /// The extra range a refill asked for.
            Refill,
            New,
        }
        self.last_upstream_job = Some(job.job_id.clone());
        let key = template_key(&job);
        let fit = match self.current.as_mut().filter(|t| t.template_key == key) {
            Some(current) => {
                if let Some(index) = current.assignments.iter().position(|assignment| {
                    assignment.start == job.nonce_start && assignment.end == job.nonce_end
                }) {
                    let assignment = &mut current.assignments[index];
                    let target_changed =
                        assignment.target != job.target || assignment.difficulty != job.difficulty;
                    assignment.upstream_job_id = job.job_id.clone();
                    assignment.target = job.target.clone();
                    assignment.difficulty = job.difficulty;
                    // A re-issued job supersedes every other range this
                    // session holds for the template, refills included.
                    let mut retired_others = false;
                    for (other, assignment) in current.assignments.iter_mut().enumerate() {
                        if other != index && !assignment.retired {
                            assignment.retired = true;
                            retired_others = true;
                        }
                    }
                    current.refill_requested_at = None;
                    Fit::Reissue {
                        generation: current.generation,
                        index,
                        target_changed,
                        retired_others,
                    }
                } else if current.refill_requested_at.take().is_some() {
                    current.assignments.push(Assignment::from_job(&job));
                    Fit::Refill
                } else {
                    Fit::New
                }
            }
            None => Fit::New,
        };
        match fit {
            Fit::Reissue {
                generation,
                index,
                target_changed,
                retired_others,
            } => {
                if target_changed {
                    self.reissue_slices(generation, index);
                }
                if retired_others {
                    self.move_rigs_off_retired_ranges(generation);
                }
            }
            Fit::Refill => self.serve_waiting(),
            Fit::New => self.start_generation(&job),
        }
    }

    fn start_generation(&mut self, job: &PoolJob) {
        self.generation += 1;
        let generation = self.generation;
        self.previous = self.current.take();
        self.current = Some(Template::from_job(generation, job));
        // Shares on the template before last are stale everywhere.
        self.local_jobs
            .retain(|_, local| local.generation + 1 >= generation);
        self.waiting.clear();

        let fork = job.height >= blocknet_pow_spec::FARADAY_ACTIVATION_HEIGHT;
        let conns: Vec<u64> = self
            .rigs
            .iter()
            .filter(|(_, rig)| rig.logged_in)
            .map(|(conn, _)| *conn)
            .collect();
        let mut dropped = Vec::new();
        for conn in conns {
            if fork && !self.rigs[&conn].blake2b {
                let rig = &self.rigs[&conn];
                rig.send(json!({
                    "method": "notification",
                    "params": {"kind": "pool_notice", "message": FARADAY_UPGRADE_MESSAGE},
                }));
                dropped.push(conn);
                continue;
            }
            self.push_new_slice(conn);
        }
        for conn in dropped {
            // Dropping the sender closes the rig's connection.
            self.rigs.remove(&conn);
        }
    }

    /// Sends rigs mining slices of `assignment` the same slices under new
    /// job ids carrying the assignment's new share target.
    fn reissue_slices(&mut self, generation: u64, assignment: usize) {
        let updates: Vec<(u64, LocalJob)> = self
            .rigs
            .iter()
            .filter_map(|(conn, rig)| {
                let local = self.local_jobs.get(rig.current_job.as_deref()?)?;
                (local.generation == generation && local.assignment == assignment)
                    .then(|| (*conn, local.clone()))
            })
            .collect();
        for (conn, local) in updates {
            let job_id = self.register_local_job(local.clone());
            if let Some(params) = self.job_params(&job_id, &local) {
                if let Some(rig) = self.rigs.get_mut(&conn) {
                    rig.current_job = Some(job_id);
                    rig.send(json!({"method": "job", "params": params}));
                }
            }
        }
    }

    /// Gives rigs mining a retired range a fresh slice of live range.
    fn move_rigs_off_retired_ranges(&mut self, generation: u64) {
        let Some(template) = self.template(generation) else {
            return;
        };
        let stranded: Vec<u64> = self
            .rigs
            .iter()
            .filter(|(_, rig)| rig.logged_in)
            .filter_map(|(conn, rig)| {
                let local = self.local_jobs.get(rig.current_job.as_deref()?)?;
                let retired = local.generation == generation
                    && template
                        .assignments
                        .get(local.assignment)
                        .is_some_and(|assignment| assignment.retired);
                retired.then_some(*conn)
            })
            .collect();
        for conn in stranded {
            self.push_new_slice(conn);
        }
    }

    // ---- nonce slices ---------------------------------------------------

    /// Carves the next slice for `conn`, asking the pool for more range when
    /// it runs low. `None` when no range is left yet.
    fn allocate_slice(&mut self, conn: u64) -> Option<(String, Value)> {
        let current = self.current.as_mut().filter(|template| !template.dead)?;
        let rig = self.rigs.get_mut(&conn)?;
        let range = current.range_size;
        let min_slice = (range / INITIAL_SLICE_DIVISOR).max(1);
        let max_slice = (range / MAX_SLICE_DIVISOR).max(min_slice);
        if rig.slice_size == 0 {
            rig.slice_size = min_slice;
        }
        let size = rig.slice_size.clamp(min_slice, max_slice);
        let index = current
            .assignments
            .iter()
            .rposition(|assignment| assignment.remaining() > 0);
        let slice = index.and_then(|index| {
            current.assignments[index]
                .take(size)
                .map(|(start, end)| (index, start, end))
        });
        let generation = current.generation;
        self.maybe_request_refill();
        let (assignment, start, end) = slice?;
        let local = LocalJob {
            conn,
            generation,
            assignment,
            start,
            end,
        };
        let job_id = self.register_local_job(local.clone());
        let params = self.job_params(&job_id, &local)?;
        let rig = self.rigs.get_mut(&conn)?;
        rig.current_job = Some(job_id.clone());
        rig.slice_issued_at = Instant::now();
        Some((job_id, params))
    }

    fn maybe_request_refill(&mut self) {
        let Some(current) = self.current.as_mut().filter(|template| !template.dead) else {
            return;
        };
        let remaining = current.remaining();
        // Without `refill` the pool only gives new range on a new session,
        // which ends every slice, so wait until nothing is left.
        let low = if self.upstream_refill {
            remaining < current.range_size / 2
        } else {
            remaining == 0
        };
        let due = current
            .refill_requested_at
            .is_none_or(|at| at.elapsed() >= UPSTREAM_REFILL_RETRY);
        if !low || !due || !self.upstream_logged_in {
            return;
        }
        if let Some(job_id) = self.last_upstream_job.as_deref() {
            self.upstream.request_nonce_refill(job_id);
            current.refill_requested_at = Some(Instant::now());
        }
    }

    fn register_local_job(&mut self, local: LocalJob) -> String {
        self.next_local_job += 1;
        let job_id = format!("p{:x}", self.next_local_job);
        self.local_jobs.insert(job_id.clone(), local);
        job_id
    }

    fn job_params(&self, job_id: &str, local: &LocalJob) -> Option<Value> {
        let template = self.template(local.generation)?;
        let assignment = template.assignments.get(local.assignment)?;
        Some(json!({
            "job_id": job_id,
            "template_id": template.template_id,
            "header_base": template.header_base,
            "target": assignment.target,
            "difficulty": assignment.difficulty,
            "network_target": template.network_target,
            "height": template.height,
            "nonce_start": local.start,
            "nonce_end": local.end,
        }))
    }

    fn template(&self, generation: u64) -> Option<&Template> {
        [self.current.as_ref(), self.previous.as_ref()]
            .into_iter()
            .flatten()
            .find(|template| template.generation == generation)
    }

    fn push_new_slice(&mut self, conn: u64) {
        match self.allocate_slice(conn) {
            Some((_, params)) => {
                if let Some(rig) = self.rigs.get(&conn) {
                    rig.send(json!({"method": "job", "params": params}));
                }
            }
            None => self.waiting.push_back(Waiter::Job(conn)),
        }
    }

    fn serve_waiting(&mut self) {
        let waiting = std::mem::take(&mut self.waiting);
        for (position, waiter) in waiting.iter().enumerate() {
            let conn = match waiter {
                Waiter::Job(conn) | Waiter::Refill(conn, _) => *conn,
            };
            if !self.rigs.contains_key(&conn) {
                continue;
            }
            let Some((_, params)) = self.allocate_slice(conn) else {
                // Out of range again; the rest keep waiting in order.
                self.waiting.extend(waiting.into_iter().skip(position));
                return;
            };
            let rig = &self.rigs[&conn];
            match waiter {
                Waiter::Job(_) => rig.send(json!({"method": "job", "params": params})),
                Waiter::Refill(_, id) => {
                    rig.send(json!({"id": id, "status": "ok", "result": params}))
                }
            }
        }
    }

    // ---- rigs -----------------------------------------------------------

    fn on_rig_line(&mut self, conn: u64, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let logged_in = self.rigs.get(&conn).is_some_and(|rig| rig.logged_in);
        match message.get("method").and_then(Value::as_str) {
            Some("login") if logged_in => self.reply_error(conn, id, "already logged in"),
            Some("login") => self.on_rig_login(conn, id, &params),
            Some(_) if !logged_in => self.reply_error(conn, id, "login first"),
            Some("submit") => self.on_rig_submit(conn, id, &params),
            Some("refill") => self.on_rig_refill(conn, id, &params),
            Some(_) => self.reply_error(conn, id, "unknown method"),
            None => {}
        }
    }

    fn on_rig_login(&mut self, conn: u64, id: Value, params: &Value) {
        let capabilities: HashSet<String> = params
            .get("capabilities")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|value| value.trim().to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        if params.get("protocol_version").and_then(Value::as_u64) != Some(2) {
            return self.reply_error(conn, id, "pool requires protocol_version 2");
        }
        if !capabilities.contains(CAP_SUBMIT_CLAIMED_HASH) {
            return self.reply_error(conn, id, "pool requires submit_claimed_hash capability");
        }
        let blake2b = capabilities.contains(CAP_POW_BLAKE2B_V1);
        if !blake2b
            && self.current.as_ref().is_some_and(|template| {
                template.height >= blocknet_pow_spec::FARADAY_ACTIVATION_HEIGHT
            })
        {
            return self.reply_error(conn, id, FARADAY_UPGRADE_MESSAGE);
        }
        let worker = params
            .get("worker")
            .and_then(Value::as_str)
            .map(|worker| {
                worker
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(64)
                    .collect::<String>()
            })
            .filter(|worker| !worker.trim().is_empty());
        let rig_address = params.get("address").and_then(Value::as_str).unwrap_or("");
        if !rig_address.is_empty()
            && rig_address != self.address
            && self.warned_addresses.insert(rig_address.to_string())
        {
            info(
                "PROXY",
                format!(
                    "rig logged in with address {}; shares through this proxy pay {}",
                    compact_address(rig_address),
                    compact_address(&self.address)
                ),
            );
        }
        let Some(rig) = self.rigs.get_mut(&conn) else {
            return;
        };
        rig.logged_in = true;
        rig.blake2b = blake2b;
        if let Some(worker) = worker {
            rig.worker = worker;
        }
        info(
            "PROXY",
            format!("rig {} connected from {}", rig.worker, rig.peer),
        );
        rig.send(json!({
            "id": id,
            "status": "ok",
            "result": {
                "protocol_version": 2,
                "capabilities": [
                    CAP_SUBMIT_CLAIMED_HASH,
                    CAP_SAME_TEMPLATE_REBIND_V1,
                    STRATUM_CAPABILITY_NONCE_REFILL_V1,
                ],
                "required_capabilities": [CAP_SUBMIT_CLAIMED_HASH],
            },
        }));
        self.push_new_slice(conn);
    }

    fn on_rig_submit(&mut self, conn: u64, id: Value, params: &Value) {
        let job_id = params.get("job_id").and_then(Value::as_str).unwrap_or("");
        let Some(nonce) = params.get("nonce").and_then(Value::as_u64) else {
            return self.reply_error(conn, id, "invalid nonce");
        };
        let Some(local) = self
            .local_jobs
            .get(job_id)
            .filter(|local| local.conn == conn)
            .cloned()
        else {
            return self.reply_error(conn, id, "stale job");
        };
        if nonce < local.start || nonce > local.end {
            return self.reply_error(conn, id, "nonce out of range");
        }
        let Some(template) = self.template(local.generation).filter(|t| !t.dead) else {
            return self.reply_error(conn, id, "stale job");
        };
        let Some(upstream_job_id) = template
            .assignments
            .get(local.assignment)
            .map(|assignment| assignment.upstream_job_id.clone())
        else {
            return self.reply_error(conn, id, "stale job");
        };
        let claimed_hash = match params.get("claimed_hash").and_then(Value::as_str) {
            Some(hex_hash) => match hex::decode(hex_hash.trim())
                .ok()
                .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
            {
                Some(hash) => Some(hash),
                None => return self.reply_error(conn, id, "invalid claimed_hash"),
            },
            None => None,
        };
        let key = (upstream_job_id.clone(), nonce);
        if self.pending_acks.contains_key(&key) {
            return self.reply_error(conn, id, "duplicate share");
        }
        let backend = params.get("backend").and_then(Value::as_str);
        if self
            .upstream
            .submit_share_with_timing(upstream_job_id, nonce, claimed_hash, backend, None)
            .is_err()
        {
            return self.reply_error(conn, id, "pool unavailable");
        }
        self.pending_acks
            .entry(key)
            .or_default()
            .push_back((conn, id));
    }

    fn on_rig_refill(&mut self, conn: u64, id: Value, params: &Value) {
        let job_id = params.get("job_id").and_then(Value::as_str).unwrap_or("");
        let Some(rig) = self.rigs.get_mut(&conn) else {
            return;
        };
        if rig.current_job.as_deref() != Some(job_id) {
            return self.reply_error(conn, id, "stale job");
        }
        let elapsed = rig.slice_issued_at.elapsed();
        if elapsed < QUICK_REFILL {
            rig.slice_size = rig.slice_size.saturating_mul(2);
        } else if elapsed > SLOW_REFILL {
            rig.slice_size /= 2;
        }
        match self.allocate_slice(conn) {
            Some((_, params)) => {
                if let Some(rig) = self.rigs.get(&conn) {
                    rig.send(json!({"id": id, "status": "ok", "result": params}));
                }
            }
            None => self.waiting.push_back(Waiter::Refill(conn, id)),
        }
    }

    fn on_rig_closed(&mut self, conn: u64) {
        if let Some(rig) = self.rigs.remove(&conn) {
            if rig.logged_in {
                info(
                    "PROXY",
                    format!(
                        "rig {} disconnected ({} accepted, {} rejected)",
                        rig.worker, rig.accepted, rig.rejected
                    ),
                );
            }
        }
        self.waiting.retain(|waiter| match waiter {
            Waiter::Job(waiting) | Waiter::Refill(waiting, _) => *waiting != conn,
        });
    }

    fn reply_error(&self, conn: u64, id: Value, message: &str) {
        if let Some(rig) = self.rigs.get(&conn) {
            rig.send(json!({"id": id, "error": message}));
        }
    }

    fn on_tick(&mut self) {
        if self
            .current
            .as_ref()
            .is_some_and(|template| template.refill_requested_at.is_some())
        {
            self.maybe_request_refill();
        }
        if self.last_summary.elapsed() >= SUMMARY_INTERVAL {
            self.log_summary();
        }
    }

    fn log_summary(&mut self) {
        let elapsed = self.last_summary.elapsed().as_secs_f64().max(1.0);
        let rigs = self.rigs.values().filter(|rig| rig.logged_in).count();
        let window = std::mem::take(&mut self.window);
        let hashrate = window.difficulty as f64 / elapsed;
        info(
            "PROXY",
            format!(
                "{rigs} rigs, ~{} from shares, {} accepted, {} rejected in the last {:.0}s; upstream {} {}",
                format_hashrate(hashrate),
                window.accepted,
                window.rejected,
                elapsed,
                self.upstream_url,
                if self.upstream_logged_in { "connected" } else { "reconnecting" },
            ),
        );
        self.last_summary = Instant::now();
    }
}

fn compact_address(address: &str) -> String {
    if address.len() <= 16 {
        return address.to_string();
    }
    format!("{}…{}", &address[..8], &address[address.len() - 6..])
}

fn format_hashrate(hashes_per_second: f64) -> String {
    if hashes_per_second >= 1_000_000.0 {
        format!("{:.2} MH/s", hashes_per_second / 1_000_000.0)
    } else if hashes_per_second >= 1_000.0 {
        format!("{:.2} kH/s", hashes_per_second / 1_000.0)
    } else {
        format!("{hashes_per_second:.1} H/s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader as StdBufReader, Write};
    use std::net::{TcpListener as StdTcpListener, TcpStream as StdTcpStream};
    use std::sync::mpsc as std_mpsc;

    const RANGE: u64 = 1280;

    fn job(job_id: &str, template: &str, height: u64, start: u64) -> Value {
        json!({"method": "job", "params": {
            "job_id": job_id, "template_id": template, "header_base": "11".repeat(92),
            "target": "ff".repeat(32), "difficulty": 5, "height": height,
            "nonce_start": start, "nonce_end": start + RANGE - 1}})
    }

    /// A scripted upstream pool. Lines the proxy sends come out of the
    /// returned receiver; lines pushed into the sender go to the proxy.
    fn fake_pool() -> (u16, std_mpsc::Receiver<Value>, std_mpsc::Sender<Value>) {
        let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (seen_tx, seen_rx) = std_mpsc::channel();
        let (push_tx, push_rx) = std_mpsc::channel::<Value>();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let push_writer = Arc::new(std::sync::Mutex::new(stream.try_clone().unwrap()));
            let pusher = Arc::clone(&push_writer);
            std::thread::spawn(move || {
                for message in push_rx {
                    let mut out = pusher.lock().unwrap();
                    if writeln!(out, "{message}").is_err() {
                        break;
                    }
                }
            });
            let mut reader = StdBufReader::new(stream);
            let mut line = String::new();
            let mut refills = 0u64;
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let Ok(message) = serde_json::from_str::<Value>(line.trim()) else {
                    line.clear();
                    continue;
                };
                line.clear();
                let reply = match message["method"].as_str() {
                    Some("login") => Some(vec![
                        json!({"id": 1, "status": "ok", "result": {"protocol_version": 2,
                            "capabilities": ["submit_claimed_hash", "nonce_refill_v1"],
                            "required_capabilities": ["submit_claimed_hash"]}}),
                        job("u1", "t1", 10, 0),
                    ]),
                    Some("submit") => Some(vec![json!({"id": message["id"], "status": "ok",
                        "result": {"accepted": true, "difficulty": 5}})]),
                    Some("refill") => {
                        refills += 1;
                        let mut reply = job(&format!("r{refills}"), "t1", 10, 10_000 * refills)
                            ["params"]
                            .clone();
                        reply["job_id"] = json!(format!("r{refills}"));
                        Some(vec![
                            json!({"id": message["id"], "status": "ok", "result": reply}),
                        ])
                    }
                    _ => None,
                };
                let _ = seen_tx.send(message);
                if let Some(reply) = reply {
                    let mut out = push_writer.lock().unwrap();
                    for message in reply {
                        writeln!(out, "{message}").unwrap();
                    }
                }
            }
            drop(writer.flush());
        });
        (port, seen_rx, push_tx)
    }

    fn start_proxy(pool_port: u16) -> (u16, Arc<AtomicBool>) {
        let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let shutdown = Arc::new(AtomicBool::new(false));
        let proxy_shutdown = Arc::clone(&shutdown);
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let _ = runtime.block_on(async move {
                serve(
                    TcpListener::from_std(listener).unwrap(),
                    format!("127.0.0.1:{pool_port}"),
                    None,
                    "proxyaddr".to_string(),
                    "proxy".to_string(),
                    proxy_shutdown,
                )
                .await
            });
        });
        (port, shutdown)
    }

    struct TestRig {
        writer: StdTcpStream,
        reader: StdBufReader<StdTcpStream>,
    }

    impl TestRig {
        fn connect(port: u16, capabilities: Value) -> (Self, Value, Value) {
            let stream = StdTcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut rig = Self {
                writer: stream.try_clone().unwrap(),
                reader: StdBufReader::new(stream),
            };
            rig.send(json!({"id": 1, "method": "login", "params": {
                "address": "rigaddr", "worker": "rig", "protocol_version": 2,
                "capabilities": capabilities}}));
            let login = rig.recv();
            let job = if login.get("error").is_none() {
                rig.recv()
            } else {
                Value::Null
            };
            (rig, login, job)
        }

        fn send(&mut self, message: Value) {
            writeln!(self.writer, "{message}").unwrap();
        }

        fn recv(&mut self) -> Value {
            let mut line = String::new();
            self.reader.read_line(&mut line).expect("proxy reply");
            serde_json::from_str(line.trim()).expect("json from proxy")
        }
    }

    fn range(params: &Value) -> (u64, u64) {
        (
            params["nonce_start"].as_u64().unwrap(),
            params["nonce_end"].as_u64().unwrap(),
        )
    }

    fn next_upstream(seen: &std_mpsc::Receiver<Value>, method: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        while let Ok(message) = seen.recv_timeout(deadline - Instant::now()) {
            if message["method"] == method {
                return message;
            }
        }
        panic!("upstream never saw {method}");
    }

    #[test]
    fn rigs_share_one_upstream_session_with_disjoint_slices() {
        let (pool_port, seen, push) = fake_pool();
        let (proxy_port, shutdown) = start_proxy(pool_port);
        next_upstream(&seen, "login");
        std::thread::sleep(Duration::from_millis(300));

        let caps = json!(["submit_claimed_hash"]);
        let (mut a, login_a, job_a) = TestRig::connect(proxy_port, caps.clone());
        let (mut b, _, job_b) = TestRig::connect(proxy_port, caps);
        assert_eq!(login_a["status"], "ok");
        assert_eq!(job_a["method"], "job");
        let (a_start, a_end) = range(&job_a["params"]);
        let (b_start, b_end) = range(&job_b["params"]);
        assert_eq!(a_end - a_start + 1, RANGE / INITIAL_SLICE_DIVISOR);
        assert!(a_end < b_start || b_end < a_start, "slices overlap");
        assert!(a_end < RANGE && b_end < RANGE);

        // A share inside the rig's slice goes upstream under the pool's job id.
        a.send(json!({"id": 7, "method": "submit", "params": {
            "job_id": job_a["params"]["job_id"], "nonce": a_start,
            "claimed_hash": "00".repeat(32)}}));
        let submit = next_upstream(&seen, "submit");
        assert_eq!(submit["params"]["job_id"], "u1");
        assert_eq!(submit["params"]["nonce"], a_start);
        let ack = a.recv();
        assert_eq!(ack["id"], 7);
        assert_eq!(ack["result"]["accepted"], true);

        // A nonce from another rig's slice is refused without reaching the pool.
        a.send(json!({"id": 8, "method": "submit", "params": {
            "job_id": job_a["params"]["job_id"], "nonce": b_start,
            "claimed_hash": "00".repeat(32)}}));
        assert_eq!(a.recv()["error"], "nonce out of range");

        // Refills hand out fresh slices, growing for a fast rig, until the
        // proxy asks the pool for more range and starts using it.
        let mut job_id = job_a["params"]["job_id"].clone();
        let mut last = (a_start, a_end);
        let mut used_refill_range = false;
        for request in 0..40 {
            a.send(json!({"id": 100 + request, "method": "refill", "params": {"job_id": job_id}}));
            let reply = a.recv();
            assert_eq!(reply["status"], "ok", "{reply}");
            let next = range(&reply["result"]);
            assert!(next.0 > last.1 || next.0 >= 10_000, "refill reused nonces");
            job_id = reply["result"]["job_id"].clone();
            last = next;
            if next.0 >= 10_000 {
                used_refill_range = true;
                break;
            }
        }
        assert!(used_refill_range, "proxy never used upstream refill range");
        let refill = next_upstream(&seen, "refill");
        assert!(refill["params"]["job_id"].as_str().is_some());

        // Vardiff re-issues the original range: rig B keeps its slice at the
        // new difficulty, and rig A leaves the refill range the pool retired.
        let mut reissue = job("u1b", "t1", 10, 0);
        reissue["params"]["difficulty"] = json!(9);
        push.send(reissue).unwrap();
        let b_job = b.recv();
        assert_eq!(b_job["method"], "job");
        assert_eq!(range(&b_job["params"]), (b_start, b_end));
        assert_eq!(b_job["params"]["difficulty"], 9);
        let a_job = a.recv();
        assert_eq!(a_job["method"], "job");
        let a_next = range(&a_job["params"]);
        assert!(
            a_next.1 < RANGE || a_next.0 >= 20_000,
            "rig A still on the retired refill range: {a_next:?}"
        );
        b.send(json!({"id": 9, "method": "submit", "params": {
            "job_id": b_job["params"]["job_id"], "nonce": b_start,
            "claimed_hash": "00".repeat(32)}}));
        assert_eq!(next_upstream(&seen, "submit")["params"]["job_id"], "u1b");
        assert_eq!(b.recv()["id"], 9);

        // A new template reaches every rig with a fresh slice.
        push.send(job("u2", "t2", 11, 50_000)).unwrap();
        for rig in [&mut a, &mut b] {
            let job = rig.recv();
            assert_eq!(job["method"], "job");
            assert_eq!(job["params"]["height"], 11);
            assert!(range(&job["params"]).0 >= 50_000);
        }
        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn seine_pool_client_mines_through_the_proxy_and_refills_in_place() {
        let (pool_port, seen, _push) = fake_pool();
        let (proxy_port, shutdown) = start_proxy(pool_port);
        next_upstream(&seen, "login");
        let client_shutdown = Arc::new(AtomicBool::new(false));
        let client = PoolClient::connect_via(
            &format!("127.0.0.1:{proxy_port}"),
            None,
            "rigaddr".into(),
            "rig".into(),
            Arc::clone(&client_shutdown),
        )
        .unwrap();
        let mut first = None;
        let mut refilled = None;
        let mut disconnects = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline && refilled.is_none() {
            match client.recv_event_timeout(Duration::from_millis(100)) {
                Some(PoolEvent::LoginAccepted(ack)) => {
                    assert!(ack.supports(STRATUM_CAPABILITY_NONCE_REFILL_V1));
                }
                Some(PoolEvent::Job(job)) if first.is_none() => {
                    client.request_nonce_refill(&job.job_id);
                    first = Some(job);
                }
                Some(PoolEvent::Job(job)) => refilled = Some(job),
                Some(PoolEvent::Disconnected(_)) => disconnects += 1,
                _ => {}
            }
        }
        client_shutdown.store(true, Ordering::Relaxed);
        shutdown.store(true, Ordering::Relaxed);
        let (first, refilled) = (first.expect("job"), refilled.expect("refilled job"));
        assert!(refilled.nonce_start > first.nonce_end);
        assert_eq!(
            disconnects, 0,
            "refill through the proxy must not reconnect"
        );
    }

    #[test]
    fn rigs_without_claimed_hash_are_refused() {
        let (pool_port, seen, _push) = fake_pool();
        let (proxy_port, shutdown) = start_proxy(pool_port);
        next_upstream(&seen, "login");
        let (_rig, login, _) = TestRig::connect(proxy_port, json!([]));
        assert_eq!(
            login["error"],
            "pool requires submit_claimed_hash capability"
        );
        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn assignments_hand_out_whole_ranges_without_overlap() {
        let mut assignment = Assignment::from_job(
            &serde_json::from_value::<PoolJob>(job("u", "t", 1, 100)["params"].clone()).unwrap(),
        );
        let mut covered = 0;
        let mut previous_end = None;
        while let Some((start, end)) = assignment.take(500) {
            assert!(previous_end.is_none_or(|prev| start == prev + 1));
            covered += end - start + 1;
            previous_end = Some(end);
        }
        assert_eq!(covered, RANGE);
        assert_eq!(previous_end, Some(100 + RANGE - 1));
        assert_eq!(assignment.remaining(), 0);
    }
}
