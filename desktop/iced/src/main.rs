use iced::widget::{column, container, text, text_editor};
use iced::{Element, Fill, Subscription, Task};
use std::sync::Arc;
use speedreading_app_core::{AppAction, AppState, AppUpdate, FfiApp, Screen};

mod views;
mod widgets;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("SpeedReader")
        .subscription(App::subscription)
        .theme(App::theme)
        .run()
}

// ── AppManager ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppManager {
    ffi: Arc<FfiApp>,
    update_rx: flume::Receiver<()>,
}

impl AppManager {
    fn new() -> Result<Self, String> {
        let data_dir = dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("speedreading")
            .to_string_lossy()
            .to_string();
        let _ = std::fs::create_dir_all(&data_dir);

        let ffi = FfiApp::new(data_dir);
        let (notify_tx, update_rx) = flume::unbounded();
        ffi.listen_for_updates(Box::new(DesktopReconciler { tx: notify_tx }));

        Ok(Self { ffi, update_rx })
    }

    fn state(&self) -> AppState {
        self.ffi.state()
    }

    fn dispatch(&self, action: AppAction) {
        self.ffi.dispatch(action);
    }

    fn subscribe_updates(&self) -> flume::Receiver<()> {
        self.update_rx.clone()
    }
}

// ── DesktopReconciler ────────────────────────────────────────────────────────

struct DesktopReconciler {
    tx: flume::Sender<()>,
}

impl speedreading_app_core::AppReconciler for DesktopReconciler {
    fn reconcile(&self, _update: AppUpdate) {
        let _ = self.tx.send(());
    }
}

// ── Subscription stream ──────────────────────────────────────────────────────

fn manager_update_stream(manager: &AppManager) -> impl iced::futures::Stream<Item = ()> {
    let rx = manager.subscribe_updates();
    iced::futures::stream::unfold(rx, |rx| async move {
        match rx.recv_async().await {
            Ok(()) => Some(((), rx)),
            Err(_) => None,
        }
    })
}

// ── App ─────────────────────────────────────────────────────────────────────

enum App {
    BootError { error: String },
    Loaded {
        manager: AppManager,
        state: AppState,
        paste_content: text_editor::Content,
        wpm_preview: u32,
        group_preview: u32,
        dark_mode: bool,
    },
}

#[derive(Debug, Clone)]
enum Message {
    // Existing — preserve
    CoreUpdated,
    // Navigation
    GoBack,
    // File operations
    OpenFile,
    FileChosen(String),
    FileCancelled,
    // Text paste
    PasteAction(text_editor::Action),
    LoadPastedText,
    // Playback
    WpmDragged(u32),
    WpmCommitted,
    GroupDragged(u32),
    GroupCommitted,
    // Theme
    ToggleTheme,
    // Generic dispatch wrapper
    Dispatch(AppAction),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = match AppManager::new() {
            Ok(manager) => {
                let state = manager.state();
                let wpm = state.wpm;
                let wpg = state.words_per_group;
                Self::Loaded {
                    manager,
                    state,
                    paste_content: text_editor::Content::new(),
                    wpm_preview: wpm,
                    group_preview: wpg,
                    dark_mode: false,
                }
            }
            Err(error) => Self::BootError { error },
        };
        (app, Task::none())
    }

    fn subscription(&self) -> Subscription<Message> {
        match self {
            App::BootError { .. } => Subscription::none(),
            App::Loaded { manager, .. } => {
                Subscription::run_with(manager.clone(), manager_update_stream)
                    .map(|_| Message::CoreUpdated)
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match self {
            App::BootError { .. } => {}
            App::Loaded { manager, state, paste_content, wpm_preview, group_preview, dark_mode } => {
                match message {
                    Message::CoreUpdated => {
                        let latest = manager.state();
                        if latest.rev > state.rev {
                            *state = latest;
                        }
                    }
                    Message::GoBack => {}
                    Message::OpenFile => {}
                    Message::FileChosen(_) | Message::FileCancelled => {}
                    Message::PasteAction(action) => {
                        paste_content.perform(action);
                    }
                    Message::LoadPastedText => {}
                    Message::WpmDragged(v) => {
                        *wpm_preview = v;
                    }
                    Message::WpmCommitted => {}
                    Message::GroupDragged(v) => {
                        *group_preview = v;
                    }
                    Message::GroupCommitted => {}
                    Message::ToggleTheme => {
                        *dark_mode = !*dark_mode;
                    }
                    Message::Dispatch(_) => {}
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::BootError { error } => container(
                column![
                    text("SpeedReader").size(24),
                    text(error).color([0.8, 0.2, 0.2]),
                ]
                .spacing(12),
            )
            .center_x(Fill)
            .center_y(Fill)
            .into(),
            App::Loaded { state, paste_content, wpm_preview, group_preview, .. } => {
                match state.router.current_screen() {
                    Screen::Landing => views::landing::view(state, paste_content),
                    Screen::Reading => views::reading::view(state, *wpm_preview, *group_preview),
                }
            }
        }
    }

    fn theme(app: &App) -> iced::Theme {
        match app {
            App::Loaded { dark_mode: true, .. } => iced::Theme::Dark,
            _ => iced::Theme::Light,
        }
    }
}

// ── Hash stability for Subscription ─────────────────────────────────────────

impl std::hash::Hash for AppManager {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash only the Arc pointer identity — not contents.
        // Required: if we hash mutable state, subscription tears down on every render tick.
        Arc::as_ptr(&self.ffi).hash(state);
    }
}

impl PartialEq for AppManager {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ffi, &other.ffi)
    }
}

impl Eq for AppManager {}
