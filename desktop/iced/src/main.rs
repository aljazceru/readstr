use iced::Center;
use iced::widget::{button, column, container, text, text_input};
use iced::{Element, Fill, Subscription, Task, Theme};
use std::sync::Arc;
use speedreading_app_core::{AppAction, AppState, FfiApp};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Speedreading App")
        .subscription(App::subscription)
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
            .join("Speedreading App")
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

struct DesktopReconciler {
    tx: flume::Sender<()>,
}

impl speedreading_app_core::AppReconciler for DesktopReconciler {
    fn reconcile(&self, _update: speedreading_app_core::AppUpdate) {
        let _ = self.tx.send(());
    }
}

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
        name_input: String,
    },
}

#[derive(Debug, Clone)]
enum Message {
    CoreUpdated,
    NameChanged(String),
    Apply,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = match AppManager::new() {
            Ok(manager) => {
                let state = manager.state();
                Self::Loaded {
                    manager,
                    state,
                    name_input: String::new(),
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
            App::Loaded {
                manager,
                state,
                name_input,
            } => match message {
                Message::CoreUpdated => {
                    let latest = manager.state();
                    if latest.rev > state.rev {
                        *state = latest;
                    }
                }
                Message::NameChanged(name) => {
                    *name_input = name;
                }
                Message::Apply => {
                    manager.dispatch(AppAction::SetName {
                        name: name_input.clone(),
                    });
                }
            },
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::BootError { error } => container(
                column![
                    text("Speedreading App").size(24),
                    text(error).color([0.8, 0.2, 0.2]),
                ]
                .spacing(12),
            )
            .center_x(Fill)
            .center_y(Fill)
            .into(),
            App::Loaded {
                state, name_input, ..
            } => container(
                column![
                    text("Speedreading App").size(24),
                    text(&state.greeting).size(20),
                    text_input("Enter a name", name_input)
                        .on_input(Message::NameChanged)
                        .on_submit(Message::Apply),
                    button("Apply").on_press(Message::Apply),
                ]
                .padding(24)
                .spacing(12)
                .align_x(Center),
            )
            .center_x(Fill)
            .center_y(Fill)
            .into()
        }
    }
}
