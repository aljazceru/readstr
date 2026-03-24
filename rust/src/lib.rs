//! SpeedReading Core — shared Rust library for all platform targets.
//! Compiled as cdylib (Android), staticlib (iOS xcframework), and rlib (iced desktop).

uniffi::setup_scaffolding!();

pub mod actions;
pub mod core;
pub mod state;
pub mod updates;

use std::sync::{Arc, RwLock};
use std::thread;

// Re-export public types so platform crates can use them without sub-module paths
pub use actions::AppAction;
pub use state::{AppState, HistoryEntry, Router, Screen, WordDisplay, WordSegment};
pub use updates::AppUpdate;

use core::actor::{ActorState, emit};
use updates::CoreMsg;

/// Callback interface implemented by each platform reconciler.
/// Called from the listen_for_updates loop on a dedicated thread.
#[uniffi::export(callback_interface)]
pub trait AppReconciler: Send + Sync + 'static {
    fn reconcile(&self, update: AppUpdate);
}

/// The FFI entry point. One instance per application lifetime.
/// Platforms call FfiApp::new(data_dir) at startup.
#[derive(uniffi::Object)]
pub struct FfiApp {
    core_tx: flume::Sender<CoreMsg>,
    update_rx: flume::Receiver<AppUpdate>,
    listening: std::sync::atomic::AtomicBool,
    shared_state: Arc<RwLock<AppState>>,
    shared_history: Arc<RwLock<Vec<HistoryEntry>>>,  // updated by actor after history mutations
}

#[uniffi::export]
impl FfiApp {
    /// Create FfiApp and start the actor thread.
    /// data_dir: platform-specific writable directory for speedreading.db
    ///   - iOS: applicationSupportDirectory
    ///   - Android: context.filesDir.absolutePath
    ///   - Desktop: dirs_next::data_dir() + "/speedreading"
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Self> {
        let (update_tx, update_rx) = flume::unbounded::<AppUpdate>();
        let (core_tx, core_rx) = flume::unbounded::<CoreMsg>();
        let shared_state = Arc::new(RwLock::new(AppState::initial()));
        let shared_history = Arc::new(RwLock::new(Vec::<HistoryEntry>::new()));

        let shared_for_actor = shared_state.clone();
        let shared_history_for_actor = shared_history.clone();
        let data_dir_clone = data_dir.clone();
        let core_tx_for_actor = core_tx.clone();

        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_time()
                .build()
                .expect("tokio runtime");

            let mut actor = ActorState::new(&data_dir_clone, shared_history_for_actor);

            // Emit initial state
            emit(&mut actor.state, &shared_for_actor, &update_tx);

            while let Ok(msg) = core_rx.recv() {
                match msg {
                    CoreMsg::Action(action) => {
                        actor.handle_action(action, &runtime, &core_tx_for_actor);
                        emit(&mut actor.state, &shared_for_actor, &update_tx);
                    }
                    CoreMsg::Internal(event) => {
                        // handle_internal calls emit internally for all internal events
                        // (ParseComplete, ParseError each call emit; WordAdvance sends PlaybackTick directly)
                        actor.handle_internal(
                            event,
                            &runtime,
                            &core_tx_for_actor,
                            &update_tx,
                            &shared_for_actor,
                        );
                    }
                }
            }
        });

        Arc::new(Self {
            core_tx,
            update_rx,
            listening: std::sync::atomic::AtomicBool::new(false),
            shared_state,
            shared_history,
        })
    }

    /// Fire-and-forget action dispatch from native UI.
    pub fn dispatch(&self, action: AppAction) {
        let _ = self.core_tx.send(CoreMsg::Action(action));
    }

    /// Start streaming updates to the reconciler on a background thread.
    /// Only one listener allowed (AtomicBool guard).
    pub fn listen_for_updates(&self, reconciler: Box<dyn AppReconciler>) {
        use std::sync::atomic::Ordering;
        if self
            .listening
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // Already listening
        }

        let rx = self.update_rx.clone();
        thread::spawn(move || {
            while let Ok(update) = rx.recv() {
                reconciler.reconcile(update);
            }
        });
    }

    /// Synchronous state snapshot for initial UI hydration.
    pub fn state(&self) -> AppState {
        match self.shared_state.read() {
            Ok(g) => g.clone(),
            Err(p) => p.into_inner().clone(),
        }
    }

    /// Pull the current reading history list. Native layers call this when
    /// history_revision increments in the AppState received via reconcile().
    /// Returns a snapshot — no DB access on this thread.
    pub fn get_history(&self) -> Vec<HistoryEntry> {
        match self.shared_history.read() {
            Ok(g) => g.clone(),
            Err(p) => p.into_inner().clone(),
        }
    }
}
