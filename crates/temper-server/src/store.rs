//! SQLite persistence: a small pool, WAL, prepared statements, transactions.
//!
//! Concurrency model: one connection per in-flight request, capped at
//! `max(2, cores/2).min(8)`. Writers serialise through SQLite's WAL writer
//! lock; readers never block writers. No ORM, no query builder, no per-request
//! schema work.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::error::AppError;

pub type DbResult<T> = Result<T, AppError>;

const SCHEMA_VERSION: i64 = 1;

pub struct Db {
    path: PathBuf,
    pool: Arc<Pool>,
}

struct Pool {
    idle: Mutex<Vec<Connection>>,
    available: Condvar,
    open: AtomicUsize,
    max: usize,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct PoolGuard<'a> {
    pool: &'a Pool,
    conn: Option<Connection>,
}

impl Deref for PoolGuard<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn.as_ref().expect("connection checked out")
    }
}

impl DerefMut for PoolGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        self.conn.as_mut().expect("connection checked out")
    }
}

impl Drop for PoolGuard<'_> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            lock(&self.pool.idle).push(conn);
            self.pool.available.notify_one();
        }
    }
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    AppError::internal(format!("无法创建数据目录: {err}"))
                })?;
            }
        }
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        let max = (cores / 2).clamp(2, 8);
        let db = Self {
            path,
            pool: Arc::new(Pool {
                idle: Mutex::new(Vec::with_capacity(max)),
                available: Condvar::new(),
                open: AtomicUsize::new(0),
                max,
            }),
        };
        {
            let conn = db.conn()?;
            migrate(&conn)?;
        }
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> DbResult<PoolGuard<'_>> {
        loop {
            {
                let mut idle = lock(&self.pool.idle);
                if let Some(conn) = idle.pop() {
                    return Ok(PoolGuard { pool: &self.pool, conn: Some(conn) });
                }
            }
            let open = self.pool.open.load(Ordering::Acquire);
            if open < self.pool.max {
                if self
                    .pool
                    .open
                    .compare_exchange(open, open + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    match open_connection(&self.path) {
                        Ok(conn) => {
                            return Ok(PoolGuard { pool: &self.pool, conn: Some(conn) });
                        }
                        Err(err) => {
                            self.pool.open.fetch_sub(1, Ordering::AcqRel);
                            return Err(err);
                        }
                    }
                }
                continue;
            }
            let idle = lock(&self.pool.idle);
            let (idle, timeout) = self
                .pool
                .available
                .wait_timeout(idle, Duration::from_secs(5))
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            drop(idle);
            if timeout.timed_out() {
                return Err(AppError::unavailable("数据库连接池繁忙，请重试"));
            }
        }
    }

    pub fn health(&self) -> bool {
        self.conn()
            .ok()
            .and_then(|conn| conn.query_row("SELECT 1", [], |r| r.get::<_, i64>(0)).ok())
            .is_some_and(|v| v == 1)
    }
}

fn open_connection(path: &Path) -> DbResult<Connection> {
    let conn = Connection::open(path)
        .map_err(|err| AppError::internal(format!("数据库连接失败: {err}")))?;
    conn.busy_timeout(Duration::from_millis(5_000))
        .map_err(AppError::from_db)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA temp_store=MEMORY;
         PRAGMA cache_size=-8000;",
    )
    .map_err(AppError::from_db)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> DbResult<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(AppError::from_db)?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        r#"
BEGIN;
CREATE TABLE IF NOT EXISTS users (
  id          TEXT PRIMARY KEY,
  created_at  TEXT NOT NULL,
  last_seen   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS device_state (
  user_id     TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  instrument  TEXT NOT NULL DEFAULT 'piano',
  key_pc      INTEGER NOT NULL DEFAULT 0,
  tempo       INTEGER NOT NULL DEFAULT 96,
  preset_id   TEXT,
  degrees     TEXT NOT NULL DEFAULT '[0,4,5,3]',
  sound       INTEGER NOT NULL DEFAULT 1,
  updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS practice_sessions (
  id          TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  mode        TEXT NOT NULL,
  difficulty  INTEGER NOT NULL,
  total       INTEGER NOT NULL,
  answered    INTEGER NOT NULL DEFAULT 0,
  correct     INTEGER NOT NULL DEFAULT 0,
  score       INTEGER NOT NULL DEFAULT 0,
  started_at  TEXT NOT NULL,
  finished_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON practice_sessions(user_id, started_at DESC);

CREATE TABLE IF NOT EXISTS practice_questions (
  id          TEXT PRIMARY KEY,
  session_id  TEXT NOT NULL REFERENCES practice_sessions(id) ON DELETE CASCADE,
  idx         INTEGER NOT NULL,
  kind        TEXT NOT NULL,
  prompt      TEXT NOT NULL,
  answer      TEXT NOT NULL,
  tag         TEXT NOT NULL,
  answered_at TEXT,
  correct     INTEGER,
  elapsed_ms  INTEGER,
  response    TEXT,
  UNIQUE(session_id, idx)
);
CREATE INDEX IF NOT EXISTS idx_questions_session ON practice_questions(session_id, idx);
CREATE INDEX IF NOT EXISTS idx_questions_tag ON practice_questions(tag);

CREATE TABLE IF NOT EXISTS progress_items (
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  item_id     TEXT NOT NULL,
  done        INTEGER NOT NULL,
  updated_at  TEXT NOT NULL,
  PRIMARY KEY (user_id, item_id)
);

CREATE TABLE IF NOT EXISTS shares (
  code        TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title       TEXT NOT NULL,
  payload     TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  views       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS custom_presets (
  id          TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  degrees     TEXT NOT NULL,
  key_pc      INTEGER NOT NULL,
  tempo       INTEGER NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_presets_user ON custom_presets(user_id, created_at DESC);

PRAGMA user_version = 1;
COMMIT;
"#,
    )
    .map_err(AppError::from_db)?;
    Ok(())
}

// ---------------------------------------------------------------- users

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub created_at: String,
}

pub fn touch_user(db: &Db, id: &str) -> DbResult<UserRow> {
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO users(id, created_at, last_seen)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(id) DO UPDATE SET last_seen = excluded.last_seen",
        params![id],
    )
    .map_err(AppError::from_db)?;
    conn.query_row("SELECT id, created_at FROM users WHERE id = ?1", params![id], |r| {
        Ok(UserRow { id: r.get(0)?, created_at: r.get(1)? })
    })
    .map_err(AppError::from_db)
}

// ---------------------------------------------------------------- device state

#[derive(Debug, Clone)]
pub struct StateRow {
    pub instrument: String,
    pub key_pc: u8,
    pub tempo: u16,
    pub preset_id: Option<String>,
    pub degrees: Vec<u8>,
    pub sound: bool,
    pub updated_at: Option<String>,
}

impl Default for StateRow {
    fn default() -> Self {
        Self {
            instrument: "piano".into(),
            key_pc: 0,
            tempo: 96,
            preset_id: Some("p1564".into()),
            degrees: vec![0, 4, 5, 3],
            sound: true,
            updated_at: None,
        }
    }
}

pub fn get_state(db: &Db, user: &str) -> DbResult<StateRow> {
    let conn = db.conn()?;
    let row = conn
        .query_row(
            "SELECT instrument, key_pc, tempo, preset_id, degrees, sound, updated_at
             FROM device_state WHERE user_id = ?1",
            params![user],
            |r| {
                let degrees: String = r.get(4)?;
                Ok(StateRow {
                    instrument: r.get(0)?,
                    key_pc: r.get::<_, i64>(1)? as u8,
                    tempo: r.get::<_, i64>(2)? as u16,
                    preset_id: r.get(3)?,
                    degrees: serde_json::from_str(&degrees).unwrap_or_default(),
                    sound: r.get::<_, i64>(5)? != 0,
                    updated_at: r.get(6)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from_db)?;
    Ok(row.unwrap_or_default())
}

pub fn put_state(db: &Db, user: &str, state: &StateRow) -> DbResult<()> {
    let conn = db.conn()?;
    let degrees = serde_json::to_string(&state.degrees).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO device_state(user_id, instrument, key_pc, tempo, preset_id, degrees, sound, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(user_id) DO UPDATE SET
           instrument=excluded.instrument, key_pc=excluded.key_pc, tempo=excluded.tempo,
           preset_id=excluded.preset_id, degrees=excluded.degrees, sound=excluded.sound,
           updated_at=excluded.updated_at",
        params![
            user,
            state.instrument,
            state.key_pc as i64,
            state.tempo as i64,
            state.preset_id,
            degrees,
            state.sound as i64
        ],
    )
    .map_err(AppError::from_db)?;
    Ok(())
}

// ---------------------------------------------------------------- practice

pub struct NewSession<'a> {
    pub id: &'a str,
    pub user: &'a str,
    pub mode: &'a str,
    pub difficulty: u8,
    pub total: u16,
}

pub struct NewQuestion {
    pub id: String,
    pub index: u16,
    pub kind: String,
    pub prompt: Value,
    pub answer: Value,
    pub tag: String,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub user_id: String,
    pub mode: String,
    pub difficulty: u8,
    pub total: u16,
    pub answered: u16,
    pub correct: u16,
    pub score: i32,
    pub started_at: String,
    pub finished_at: Option<String>,
}

impl SessionRow {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            user_id: r.get(1)?,
            mode: r.get(2)?,
            difficulty: r.get::<_, i64>(3)? as u8,
            total: r.get::<_, i64>(4)? as u16,
            answered: r.get::<_, i64>(5)? as u16,
            correct: r.get::<_, i64>(6)? as u16,
            score: r.get::<_, i64>(7)? as i32,
            started_at: r.get(8)?,
            finished_at: r.get(9)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct QuestionRow {
    pub id: String,
    pub session_id: String,
    pub index: u16,
    pub kind: String,
    pub prompt: Value,
    pub answer: Value,
    pub tag: String,
    pub answered: bool,
    pub correct: Option<bool>,
}

pub fn create_session(db: &Db, s: &NewSession<'_>, questions: &[NewQuestion]) -> DbResult<()> {
    let mut conn = db.conn()?;
    let tx = conn.transaction().map_err(AppError::from_db)?;
    tx.execute(
        "INSERT INTO practice_sessions(id, user_id, mode, difficulty, total, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![s.id, s.user, s.mode, s.difficulty as i64, s.total as i64],
    )
    .map_err(AppError::from_db)?;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO practice_questions(id, session_id, idx, kind, prompt, answer, tag)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(AppError::from_db)?;
        for q in questions {
            stmt.execute(params![
                q.id,
                s.id,
                q.index as i64,
                q.kind,
                q.prompt.to_string(),
                q.answer.to_string(),
                q.tag
            ])
            .map_err(AppError::from_db)?;
        }
    }
    tx.commit().map_err(AppError::from_db)?;
    Ok(())
}

pub fn session(db: &Db, id: &str) -> DbResult<Option<SessionRow>> {
    let conn = db.conn()?;
    conn.query_row(
        "SELECT id, user_id, mode, difficulty, total, answered, correct, score, started_at, finished_at
         FROM practice_sessions WHERE id = ?1",
        params![id],
        SessionRow::from_row,
    )
    .optional()
    .map_err(AppError::from_db)
}

pub fn questions(db: &Db, session_id: &str) -> DbResult<Vec<QuestionRow>> {
    let conn = db.conn()?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, session_id, idx, kind, prompt, answer, tag, answered_at, correct
             FROM practice_questions WHERE session_id = ?1 ORDER BY idx",
        )
        .map_err(AppError::from_db)?;
    let rows = stmt
        .query_map(params![session_id], |r| {
            let prompt: String = r.get(4)?;
            let answer: String = r.get(5)?;
            Ok(QuestionRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                index: r.get::<_, i64>(2)? as u16,
                kind: r.get(3)?,
                prompt: serde_json::from_str(&prompt).unwrap_or(Value::Null),
                answer: serde_json::from_str(&answer).unwrap_or(Value::Null),
                tag: r.get(6)?,
                answered: r.get::<_, Option<String>>(7)?.is_some(),
                correct: r.get::<_, Option<i64>>(8)?.map(|v| v != 0),
            })
        })
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;
    Ok(rows)
}

pub fn question(db: &Db, session_id: &str, question_id: &str) -> DbResult<Option<QuestionRow>> {
    Ok(questions(db, session_id)?
        .into_iter()
        .find(|q| q.id == question_id))
}

/// Returns false when the question was already answered (idempotency guard).
pub fn record_answer(
    db: &Db,
    question_id: &str,
    correct: bool,
    elapsed_ms: i64,
    response: &Value,
) -> DbResult<bool> {
    let conn = db.conn()?;
    let changed = conn
        .execute(
            "UPDATE practice_questions
             SET answered_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 correct = ?2, elapsed_ms = ?3, response = ?4
             WHERE id = ?1 AND answered_at IS NULL",
            params![question_id, correct as i64, elapsed_ms, response.to_string()],
        )
        .map_err(AppError::from_db)?;
    Ok(changed == 1)
}

pub fn bump_session(db: &Db, session_id: &str, correct: bool, score: i32) -> DbResult<()> {
    let conn = db.conn()?;
    conn.execute(
        "UPDATE practice_sessions
         SET answered = answered + 1, correct = correct + ?2, score = score + ?3
         WHERE id = ?1",
        params![session_id, correct as i64, score],
    )
    .map_err(AppError::from_db)?;
    Ok(())
}

pub fn finish_session(db: &Db, session_id: &str, score: i32) -> DbResult<()> {
    let conn = db.conn()?;
    conn.execute(
        "UPDATE practice_sessions
         SET finished_at = COALESCE(finished_at, strftime('%Y-%m-%dT%H:%M:%fZ','now')), score = ?2
         WHERE id = ?1",
        params![session_id, score],
    )
    .map_err(AppError::from_db)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub id: String,
    pub mode: String,
    pub difficulty: u8,
    pub total: u16,
    pub answered: u16,
    pub correct: u16,
    pub score: i32,
    pub started_at: String,
    pub finished_at: Option<String>,
}

pub fn history(db: &Db, user: &str, limit: u32) -> DbResult<Vec<HistoryRow>> {
    let conn = db.conn()?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, mode, difficulty, total, answered, correct, score, started_at, finished_at
             FROM practice_sessions WHERE user_id = ?1
             ORDER BY started_at DESC LIMIT ?2",
        )
        .map_err(AppError::from_db)?;
    let rows = stmt
        .query_map(params![user, limit as i64], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                mode: r.get(1)?,
                difficulty: r.get::<_, i64>(2)? as u8,
                total: r.get::<_, i64>(3)? as u16,
                answered: r.get::<_, i64>(4)? as u16,
                correct: r.get::<_, i64>(5)? as u16,
                score: r.get::<_, i64>(6)? as i32,
                started_at: r.get(7)?,
                finished_at: r.get(8)?,
            })
        })
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;
    Ok(rows)
}

#[derive(Debug, Clone, Default)]
pub struct ModeStat {
    pub mode: String,
    pub sessions: u32,
    pub answered: u32,
    pub correct: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub sessions: u32,
    pub answered: u32,
    pub correct: u32,
    pub score: i32,
    pub minutes: f64,
    pub streak_days: u32,
    pub by_mode: Vec<ModeStat>,
}

pub fn stats(db: &Db, user: &str) -> DbResult<Stats> {
    let conn = db.conn()?;
    let mut stats = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(answered),0), COALESCE(SUM(correct),0), COALESCE(SUM(score),0)
             FROM practice_sessions WHERE user_id = ?1 AND finished_at IS NOT NULL",
            params![user],
            |r| {
                Ok(Stats {
                    sessions: r.get::<_, i64>(0)? as u32,
                    answered: r.get::<_, i64>(1)? as u32,
                    correct: r.get::<_, i64>(2)? as u32,
                    score: r.get::<_, i64>(3)? as i32,
                    ..Default::default()
                })
            },
        )
        .map_err(AppError::from_db)?;

    let seconds: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(q.elapsed_ms),0) / 1000.0
             FROM practice_questions q
             JOIN practice_sessions s ON s.id = q.session_id
             WHERE s.user_id = ?1 AND q.answered_at IS NOT NULL",
            params![user],
            |r| r.get(0),
        )
        .map_err(AppError::from_db)?;
    stats.minutes = (seconds / 60.0 * 10.0).round() / 10.0;

    let mut stmt = conn
        .prepare_cached(
            "SELECT mode, COUNT(*), COALESCE(SUM(answered),0), COALESCE(SUM(correct),0)
             FROM practice_sessions WHERE user_id = ?1 AND finished_at IS NOT NULL
             GROUP BY mode ORDER BY COUNT(*) DESC",
        )
        .map_err(AppError::from_db)?;
    stats.by_mode = stmt
        .query_map(params![user], |r| {
            Ok(ModeStat {
                mode: r.get(0)?,
                sessions: r.get::<_, i64>(1)? as u32,
                answered: r.get::<_, i64>(2)? as u32,
                correct: r.get::<_, i64>(3)? as u32,
            })
        })
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;

    let mut dates = conn
        .prepare_cached(
            "SELECT DISTINCT date(started_at) AS d FROM practice_sessions
             WHERE user_id = ?1 AND finished_at IS NOT NULL ORDER BY d DESC LIMIT 60",
        )
        .map_err(AppError::from_db)?;
    let days: Vec<String> = dates
        .query_map(params![user], |r| r.get(0))
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;
    stats.streak_days = streak_from_days(&days);

    Ok(stats)
}

fn streak_from_days(days: &[String]) -> u32 {
    if days.is_empty() {
        return 0;
    }
    let today = chrono_like_today();
    // The streak counts from today, or from yesterday when today is empty.
    let mut expected = today.clone();
    let mut streak = 0;
    for day in days {
        if *day == expected {
            streak += 1;
            expected = previous_day(&expected);
        } else if streak == 0 && previous_day(&today) == *day {
            streak = 1;
            expected = previous_day(day);
        } else {
            break;
        }
    }
    streak
}

/// UTC date without pulling in a date-time crate.
fn chrono_like_today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn previous_day(day: &str) -> String {
    let parts: Vec<i64> = day.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return String::new();
    }
    let days = days_from_civil(parts[0], parts[1] as u32, parts[2] as u32) - 1;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's civil-date algorithms.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[derive(Debug, Clone)]
pub struct WeakTag {
    pub tag: String,
    pub attempts: u32,
    pub correct: u32,
}

pub fn weak_tags(db: &Db, user: &str, limit: u32) -> DbResult<Vec<WeakTag>> {
    let conn = db.conn()?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT q.tag, COUNT(*), SUM(q.correct)
             FROM practice_questions q
             JOIN practice_sessions s ON s.id = q.session_id
             WHERE s.user_id = ?1 AND q.answered_at IS NOT NULL AND q.tag <> ''
             GROUP BY q.tag HAVING COUNT(*) >= 2
             ORDER BY (SUM(q.correct) * 1.0 / COUNT(*)) ASC, COUNT(*) DESC
             LIMIT ?2",
        )
        .map_err(AppError::from_db)?;
    let rows = stmt
        .query_map(params![user, limit as i64], |r| {
            Ok(WeakTag {
                tag: r.get(0)?,
                attempts: r.get::<_, i64>(1)? as u32,
                correct: r.get::<_, i64>(2)? as u32,
            })
        })
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;
    Ok(rows)
}

// ---------------------------------------------------------------- progress

pub fn get_progress(db: &Db, user: &str) -> DbResult<Vec<(String, bool)>> {
    let conn = db.conn()?;
    let mut stmt = conn
        .prepare_cached("SELECT item_id, done FROM progress_items WHERE user_id = ?1 ORDER BY item_id")
        .map_err(AppError::from_db)?;
    let rows = stmt
        .query_map(params![user], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)))
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;
    Ok(rows)
}

pub fn put_progress(db: &Db, user: &str, items: &[(String, bool)]) -> DbResult<()> {
    let mut conn = db.conn()?;
    let tx = conn.transaction().map_err(AppError::from_db)?;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO progress_items(user_id, item_id, done, updated_at)
                 VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                 ON CONFLICT(user_id, item_id) DO UPDATE SET
                   done=excluded.done, updated_at=excluded.updated_at",
            )
            .map_err(AppError::from_db)?;
        for (id, done) in items {
            stmt.execute(params![user, id, *done as i64])
                .map_err(AppError::from_db)?;
        }
    }
    tx.commit().map_err(AppError::from_db)?;
    Ok(())
}

// ---------------------------------------------------------------- shares

pub struct ShareRow {
    pub code: String,
    pub title: String,
    pub payload: Value,
    pub created_at: String,
    pub views: u32,
}

pub fn create_share(db: &Db, user: &str, title: &str, payload: &Value) -> DbResult<String> {
    let conn = db.conn()?;
    for _ in 0..8 {
        let code = random_code(6);
        let result = conn.execute(
            "INSERT INTO shares(code, user_id, title, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![code, user, title, payload.to_string()],
        );
        match result {
            Ok(_) => return Ok(code),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation => continue,
            Err(err) => return Err(AppError::from_db(err)),
        }
    }
    Err(AppError::internal("无法生成分享码，请重试"))
}

pub fn get_share(db: &Db, code: &str) -> DbResult<Option<ShareRow>> {
    let conn = db.conn()?;
    let row = conn
        .query_row(
            "SELECT code, title, payload, created_at, views FROM shares WHERE code = ?1",
            params![code],
            |r| {
                let payload: String = r.get(2)?;
                Ok(ShareRow {
                    code: r.get(0)?,
                    title: r.get(1)?,
                    payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
                    created_at: r.get(3)?,
                    views: r.get::<_, i64>(4)? as u32,
                })
            },
        )
        .optional()
        .map_err(AppError::from_db)?;
    if row.is_some() {
        conn.execute("UPDATE shares SET views = views + 1 WHERE code = ?1", params![code])
            .map_err(AppError::from_db)?;
    }
    Ok(row)
}

fn random_code(len: usize) -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

// ---------------------------------------------------------------- presets

#[derive(Debug, Clone)]
pub struct PresetRow {
    pub id: String,
    pub name: String,
    pub degrees: Vec<u8>,
    pub key_pc: u8,
    pub tempo: u16,
    pub created_at: String,
}

pub fn list_presets(db: &Db, user: &str) -> DbResult<Vec<PresetRow>> {
    let conn = db.conn()?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, name, degrees, key_pc, tempo, created_at FROM custom_presets
             WHERE user_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(AppError::from_db)?;
    let rows = stmt
        .query_map(params![user], |r| {
            let degrees: String = r.get(2)?;
            Ok(PresetRow {
                id: r.get(0)?,
                name: r.get(1)?,
                degrees: serde_json::from_str(&degrees).unwrap_or_default(),
                key_pc: r.get::<_, i64>(3)? as u8,
                tempo: r.get::<_, i64>(4)? as u16,
                created_at: r.get(5)?,
            })
        })
        .map_err(AppError::from_db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from_db)?;
    Ok(rows)
}

pub fn create_preset(
    db: &Db,
    user: &str,
    name: &str,
    degrees: &[u8],
    key_pc: u8,
    tempo: u16,
) -> DbResult<PresetRow> {
    let conn = db.conn()?;
    let id = uuid::Uuid::new_v4().to_string();
    let degrees_json = serde_json::to_string(degrees).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO custom_presets(id, user_id, name, degrees, key_pc, tempo, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![id, user, name, degrees_json, key_pc as i64, tempo as i64],
    )
    .map_err(AppError::from_db)?;
    let created_at = conn
        .query_row(
            "SELECT created_at FROM custom_presets WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(AppError::from_db)?;
    Ok(PresetRow {
        id,
        name: name.to_string(),
        degrees: degrees.to_vec(),
        key_pc,
        tempo,
        created_at,
    })
}

pub fn delete_preset(db: &Db, user: &str, id: &str) -> DbResult<bool> {
    let conn = db.conn()?;
    let changed = conn
        .execute(
            "DELETE FROM custom_presets WHERE id = ?1 AND user_id = ?2",
            params![id, user],
        )
        .map_err(AppError::from_db)?;
    Ok(changed == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn migrations_are_idempotent() {
        let (_dir, db) = temp_db();
        {
            let conn = db.conn().unwrap();
            migrate(&conn).unwrap();
            migrate(&conn).unwrap();
            let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
            assert_eq!(version, SCHEMA_VERSION);
        }
    }

    #[test]
    fn state_roundtrip() {
        let (_dir, db) = temp_db();
        touch_user(&db, "device-abc").unwrap();
        let default = get_state(&db, "device-abc").unwrap();
        assert_eq!(default.instrument, "piano");
        let mut next = default;
        next.instrument = "guitar".into();
        next.key_pc = 5;
        next.degrees = vec![0, 4, 5, 3, 0, 2];
        put_state(&db, "device-abc", &next).unwrap();
        let loaded = get_state(&db, "device-abc").unwrap();
        assert_eq!(loaded.instrument, "guitar");
        assert_eq!(loaded.key_pc, 5);
        assert_eq!(loaded.degrees.len(), 6);
    }

    #[test]
    fn progress_and_presets_are_scoped_to_user() {
        let (_dir, db) = temp_db();
        touch_user(&db, "user-one").unwrap();
        touch_user(&db, "user-two").unwrap();
        put_progress(&db, "user-one", &[("scale.major".into(), true)]).unwrap();
        assert_eq!(get_progress(&db, "user-one").unwrap().len(), 1);
        assert!(get_progress(&db, "user-two").unwrap().is_empty());

        let preset = create_preset(&db, "user-one", "我的进行", &[0, 4, 5, 3], 2, 92).unwrap();
        assert_eq!(list_presets(&db, "user-one").unwrap().len(), 1);
        assert!(!delete_preset(&db, "user-two", &preset.id).unwrap());
        assert!(delete_preset(&db, "user-one", &preset.id).unwrap());
    }

    #[test]
    fn streak_counts_consecutive_days() {
        let today = chrono_like_today();
        let yesterday = previous_day(&today);
        assert_eq!(streak_from_days(&[today.clone(), yesterday.clone()]), 2);
        assert_eq!(streak_from_days(&[yesterday.clone()]), 1);
        assert_eq!(streak_from_days(&[today]), 1);
        assert_eq!(streak_from_days(&[]), 0);
    }
}
