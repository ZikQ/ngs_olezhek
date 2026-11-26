use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use axum::extract::ws::{Message, WebSocket};
use ngs_olezhek_bundle::{Bundle, Entry, lua_name};
use notify::{EventKind, RecursiveMode, Watcher};
use omnilua::Lua;
use tokio::sync::broadcast::{self, error::RecvError};

pub type Updates = broadcast::Sender<Arc<Vec<u8>>>;

/// Watches `dir` on a background thread and broadcasts every changed `.lua` file as a
/// one-entry bundle (same format as `/_ngs/bundle`, so the client reuses its decoder).
pub fn watch(dir: PathBuf) -> Updates {
    let (tx, _) = broadcast::channel(64);
    let sender = tx.clone();
    thread::spawn(move || {
        let (events_tx, events) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(events_tx) {
            Ok(w) => w,
            Err(e) => return eprintln!("ngs_olezhek: live reload disabled: {e}"),
        };
        if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
            return eprintln!("ngs_olezhek: live reload disabled, cannot watch {}: {e}", dir.display());
        }
        let lua = Lua::new();
        while let Ok(first) = events.recv() {
            let mut changed = BTreeMap::new();
            collect(&dir, first, &mut changed);
            // Editors save in several steps (truncate, write, rename); wait until it settles.
            while let Ok(more) = events.recv_timeout(Duration::from_millis(60)) {
                collect(&dir, more, &mut changed);
            }
            let entries: Vec<Entry> = changed
                .into_iter()
                .filter_map(|(name, path)| Some(Entry::compile(&lua, &name, &fs::read(path).ok()?, false)))
                .collect();
            if !entries.is_empty() {
                let _ = sender.send(Arc::new(Bundle { dev: true, entries }.encode()));
            }
        }
    });
    tx
}

fn collect(dir: &Path, event: notify::Result<notify::Event>, out: &mut BTreeMap<String, PathBuf>) {
    let Ok(event) = event else { return };
    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
        return;
    }
    for path in event.paths {
        if let Some(name) = lua_name(dir, &path)
            && path.is_file()
        {
            out.insert(name, path);
        }
    }
}

/// Without a watcher the socket still matters: when it drops and reconnects the client
/// knows the server restarted and reloads the page.
pub async fn serve(mut socket: WebSocket, updates: Option<broadcast::Receiver<Arc<Vec<u8>>>>) {
    let Some(mut rx) = updates else {
        while let Some(Ok(_)) = socket.recv().await {}
        return;
    };
    loop {
        tokio::select! {
            update = rx.recv() => match update {
                Ok(bytes) => {
                    if socket.send(Message::Binary(bytes.as_ref().clone().into())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
}
