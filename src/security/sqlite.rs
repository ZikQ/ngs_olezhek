use std::{
    io,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use super::{BoxFuture, session::{Record, SessionStore}};

/// Expired rows are deleted at most this often.
const SWEEP_EVERY: u64 = 600;

/// Sessions in a SQLite file: they survive restarts and deploys. One server process per file
/// is the intended setup; several processes work too (WAL), at the cost of lock waits.
#[derive(Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    table: Arc<str>,
    last_sweep: Arc<AtomicU64>,
}

impl SqliteStore {
    /// Opens or creates the database and the `ngs_sessions` table.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let conn = Connection::open(path).map_err(other)?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(other)?;
        conn.pragma_update(None, "synchronous", "NORMAL").map_err(other)?;
        Self::from_connection(conn)
    }

    /// A database that lives as long as the store; handy in tests.
    pub fn in_memory() -> io::Result<Self> {
        Self::from_connection(Connection::open_in_memory().map_err(other)?)
    }

    /// Uses a connection the app opened itself, e.g. to keep sessions in the app's own database.
    pub fn from_connection(conn: Connection) -> io::Result<Self> {
        Self::with_table(conn, "ngs_sessions")
    }

    /// Like `from_connection` with another table name (letters, digits and `_`).
    pub fn with_table(conn: Connection, table: &str) -> io::Result<Self> {
        if table.is_empty() || !table.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("bad session table name {table:?}")));
        }
        conn.busy_timeout(Duration::from_secs(5)).map_err(other)?;
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {table} (
                key TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                created INTEGER NOT NULL,
                seen INTEGER NOT NULL,
                expires INTEGER NOT NULL
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS {table}_expires ON {table}(expires);"
        ))
        .map_err(other)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)), table: table.into(), last_sweep: Arc::default() })
    }

    /// Deletes expired sessions now. Saves already do it every ten minutes.
    pub async fn sweep(&self) -> io::Result<usize> {
        let table = self.table.clone();
        self.run(move |c| c.execute(&format!("DELETE FROM {table} WHERE expires <= ?1"), [sql_time(unix_now())])).await
    }

    /// rusqlite blocks, so every query goes to tokio's blocking pool.
    async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> io::Result<T> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
            f(&conn).map_err(other)
        })
        .await
        .map_err(other)?
    }
}

impl SessionStore for SqliteStore {
    fn load<'a>(&'a self, key: &'a str) -> BoxFuture<'a, io::Result<Option<Record>>> {
        let (key, table) = (key.to_owned(), self.table.clone());
        Box::pin(async move {
            let row = self
                .run(move |c| {
                    c.query_row(
                        &format!("SELECT data, created, seen FROM {table} WHERE key = ?1 AND expires > ?2"),
                        params![key, sql_time(unix_now())],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
                    )
                    .optional()
                })
                .await?;
            let Some((data, created, seen)) = row else { return Ok(None) };
            // A row we cannot read is treated as no session: the user logs in again instead of
            // getting 503 forever.
            let Ok(data) = serde_json::from_str::<Map<String, Value>>(&data) else { return Ok(None) };
            Ok(Some(Record { data, created: created.max(0) as u64, seen: seen.max(0) as u64 }))
        })
    }

    fn save<'a>(&'a self, key: &'a str, record: &'a Record, ttl: Duration) -> BoxFuture<'a, io::Result<()>> {
        let data = serde_json::to_string(&record.data);
        let (key, table, created, seen) = (key.to_owned(), self.table.clone(), record.created, record.seen);
        Box::pin(async move {
            let data = data.map_err(other)?;
            let now = unix_now();
            let sweep = now.saturating_sub(self.last_sweep.load(Ordering::Relaxed)) >= SWEEP_EVERY
                && self.last_sweep.swap(now, Ordering::Relaxed) != now;
            self.run(move |c| {
                c.execute(
                    &format!(
                        "INSERT INTO {table} (key, data, created, seen, expires) VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT(key) DO UPDATE SET data = excluded.data, created = excluded.created,
                         seen = excluded.seen, expires = excluded.expires"
                    ),
                    params![key, data, sql_time(created), sql_time(seen), sql_time(now.saturating_add(ttl.as_secs()))],
                )?;
                if sweep {
                    c.execute(&format!("DELETE FROM {table} WHERE expires <= ?1"), [sql_time(now)])?;
                }
                Ok(())
            })
            .await
        })
    }

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, io::Result<()>> {
        let (key, table) = (key.to_owned(), self.table.clone());
        Box::pin(async move {
            self.run(move |c| c.execute(&format!("DELETE FROM {table} WHERE key = ?1"), [key]).map(drop)).await
        })
    }
}

fn other(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}

/// SQLite integers are signed.
fn sql_time(secs: u64) -> i64 {
    secs.min(i64::MAX as u64) as i64
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
