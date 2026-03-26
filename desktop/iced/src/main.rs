use iced::widget::{column, container, text, text_editor};
use iced::{Element, Fill, Font, Subscription, Task};
use iced::theme::Palette;
use std::sync::Arc;
use speedreading_app_core::{AppAction, AppState, AppUpdate, FfiApp, Screen};

mod views;
mod widgets;

// ── Font constants ────────────────────────────────────────────────────────────

const SYNE_BOLD: &[u8]           = include_bytes!("fonts/Syne-Bold.ttf");
const JETBRAINS_MONO_REG: &[u8]  = include_bytes!("fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("fonts/JetBrainsMono-Bold.ttf");

pub const SYNE: Font           = Font::with_name("Syne");
pub const JETBRAINS_MONO: Font = Font::with_name("JetBrains Mono");

pub const ACCENT_ORANGE_DARK:  iced::Color = iced::Color { r: 1.0,   g: 0.420, b: 0.169, a: 1.0 };
pub const ACCENT_ORANGE_LIGHT: iced::Color = iced::Color { r: 0.898, g: 0.322, b: 0.039, a: 1.0 };

// ── Theme palette functions ───────────────────────────────────────────────────

fn dark_theme() -> iced::Theme {
    iced::Theme::custom("ReadstrDark".to_string(), Palette {
        background: iced::Color::from_rgb(0.055, 0.047, 0.071),
        text:       iced::Color::from_rgb(0.918, 0.902, 0.957),
        primary:    ACCENT_ORANGE_DARK,
        success:    iced::Color::from_rgb(0.071, 0.400, 0.314),
        warning:    iced::Color::from_rgb(1.0,   0.792, 0.310),
        danger:     iced::Color::from_rgb(1.0,   0.333, 0.333),
    })
}

fn light_theme() -> iced::Theme {
    iced::Theme::custom("ReadstrLight".to_string(), Palette {
        background: iced::Color::from_rgb(0.965, 0.953, 1.0),
        text:       iced::Color::from_rgb(0.102, 0.094, 0.145),
        primary:    ACCENT_ORANGE_LIGHT,
        success:    iced::Color::from_rgb(0.071, 0.400, 0.314),
        warning:    iced::Color::from_rgb(1.0,   0.792, 0.310),
        danger:     iced::Color::from_rgb(0.800, 0.133, 0.133),
    })
}

fn load_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("icon.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Readstr")
        .subscription(App::subscription)
        .theme(App::theme)
        .window(iced::window::Settings {
            icon: load_icon(),
            ..iced::window::Settings::default()
        })
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

// ── HistoryRow ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct HistoryRow {
    entry: speedreading_app_core::HistoryEntry,
    is_missing: bool,
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

// ── Theme helpers ────────────────────────────────────────────────────────────

fn theme_file_path() -> std::path::PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("speedreading")
        .join("theme")
}

fn load_theme_from_disk() -> bool {
    std::fs::read_to_string(theme_file_path())
        .ok()
        .map(|s| s.trim() == "dark")
        .unwrap_or(false)
}

fn save_theme_to_disk(dark: bool) {
    let path = theme_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, if dark { "dark" } else { "light" });
}

// ── File dialog ──────────────────────────────────────────────────────────────

fn open_file_task() -> Task<Message> {
    Task::future(
        rfd::AsyncFileDialog::new()
            .add_filter("Supported Files", &["txt", "epub", "pdf"])
            .pick_file(),
    )
    .then(|handle| match handle {
        Some(file_handle) => {
            let path = file_handle.path().to_string_lossy().to_string();
            Task::done(Message::FileChosen(path))
        }
        None => Task::done(Message::FileCancelled),
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
        history: Vec<HistoryRow>,
        last_history_revision: u64,
        pending_delete: Option<(String, String)>,  // (file_hash, file_name) awaiting confirm
        file_not_found_error: Option<String>,
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
    // History
    ResumeFile(String),                          // file_hash
    ResumeMissingFile(String),                   // file_hash (unused — picker handles it)
    ConfirmDeletePrompt(String, String),         // (file_hash, file_name)
    ConfirmDelete,
    CancelDelete,
    // Font loading
    FontLoaded,
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
                    dark_mode: load_theme_from_disk(),
                    history: vec![],
                    last_history_revision: 0,
                    pending_delete: None,
                    file_not_found_error: None,
                }
            }
            Err(error) => Self::BootError { error },
        };
        let font_tasks = Task::batch([
            iced::font::load(SYNE_BOLD).map(|_| Message::FontLoaded),
            iced::font::load(JETBRAINS_MONO_REG).map(|_| Message::FontLoaded),
            iced::font::load(JETBRAINS_MONO_BOLD).map(|_| Message::FontLoaded),
        ]);
        (app, font_tasks)
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
            App::BootError { .. } => Task::none(),
            App::Loaded {
                manager,
                state,
                paste_content,
                wpm_preview,
                group_preview,
                dark_mode,
                history,
                last_history_revision,
                pending_delete,
                file_not_found_error,
            } => match message {
                Message::CoreUpdated => {
                    let latest = manager.state();
                    if latest.rev > state.rev {
                        *wpm_preview = latest.wpm;
                        *group_preview = latest.words_per_group;
                        if latest.history_revision != *last_history_revision {
                            *last_history_revision = latest.history_revision;
                            let raw = manager.ffi.get_history();
                            *history = raw
                                .into_iter()
                                .map(|e| {
                                    let is_missing = !std::path::Path::new(&e.file_path).exists();
                                    HistoryRow { entry: e, is_missing }
                                })
                                .collect();
                        }
                        *state = latest;
                    }
                    Task::none()
                }
                Message::GoBack => {
                    manager.dispatch(AppAction::PopScreen);
                    Task::none()
                }
                Message::OpenFile => open_file_task(),
                Message::FileChosen(path) => {
                    *file_not_found_error = None;
                    manager.dispatch(AppAction::PushScreen {
                        screen: speedreading_app_core::Screen::Reading,
                    });
                    manager.dispatch(AppAction::FileSelected { path });
                    Task::none()
                }
                Message::FileCancelled => Task::none(),
                Message::PasteAction(action) => {
                    paste_content.perform(action);
                    Task::none()
                }
                Message::LoadPastedText => {
                    let text_str = paste_content.text();
                    if !text_str.trim().is_empty() {
                        manager.dispatch(AppAction::PushScreen {
                            screen: speedreading_app_core::Screen::Reading,
                        });
                        manager.dispatch(AppAction::LoadText { text: text_str });
                    }
                    Task::none()
                }
                Message::WpmDragged(v) => {
                    *wpm_preview = v;
                    Task::none()
                }
                Message::WpmCommitted => {
                    manager.dispatch(AppAction::SetWPM { wpm: *wpm_preview });
                    Task::none()
                }
                Message::GroupDragged(v) => {
                    *group_preview = v;
                    Task::none()
                }
                Message::GroupCommitted => {
                    manager.dispatch(AppAction::SetWordsPerGroup { n: *group_preview });
                    Task::none()
                }
                Message::ToggleTheme => {
                    *dark_mode = !*dark_mode;
                    save_theme_to_disk(*dark_mode);
                    Task::none()
                }
                Message::Dispatch(action) => {
                    manager.dispatch(action);
                    Task::none()
                }
                Message::ResumeFile(file_hash) => {
                    // Do NOT dispatch PushScreen — on_parse_complete in actor.rs pushes Screen::Reading
                    // Dispatching PushScreen here would cause a double-push
                    manager.dispatch(AppAction::ResumeFile { file_hash });
                    Task::none()
                }
                Message::ResumeMissingFile(_file_hash) => {
                    *file_not_found_error = Some("File not found — please re-locate it".to_string());
                    open_file_task()
                }
                Message::ConfirmDeletePrompt(file_hash, file_name) => {
                    *pending_delete = Some((file_hash, file_name));
                    Task::none()
                }
                Message::ConfirmDelete => {
                    if let Some((file_hash, _)) = pending_delete.take() {
                        manager.dispatch(AppAction::DeleteSession { file_hash });
                    }
                    Task::none()
                }
                Message::CancelDelete => {
                    *pending_delete = None;
                    Task::none()
                }
                Message::FontLoaded => Task::none(),
            },
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::BootError { error } => container(
                column![
                    text("SpeedReader").size(24),
                    text(error).color([0.8_f32, 0.2_f32, 0.2_f32]),
                ]
                .spacing(12),
            )
            .center_x(Fill)
            .center_y(Fill)
            .into(),

            App::Loaded {
                state,
                paste_content,
                wpm_preview,
                group_preview,
                dark_mode,
                history,
                pending_delete,
                file_not_found_error,
                ..
            } => {
                let screen: Element<'_, Message> = match state.router.current_screen() {
                    Screen::Landing => views::landing::view(state, paste_content, history, pending_delete.as_ref(), file_not_found_error.as_deref()),
                    Screen::Reading => {
                        views::reading::view(state, *wpm_preview, *group_preview, *dark_mode)
                    }
                };

                screen
            }
        }
    }

    fn theme(app: &App) -> iced::Theme {
        match app {
            App::Loaded { dark_mode: true, .. } => dark_theme(),
            _ => light_theme(),
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
