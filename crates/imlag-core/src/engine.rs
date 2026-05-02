//! Top-level engine — wires GSI events to the corpus / sender / cfg manager
//! and forwards UI status to the GUI via a [`tokio::sync::broadcast`] channel.

use crate::cfg_manager::CfgManager;
use crate::chat::ChatMessageManager;
use crate::config::AppConfig;
use crate::events::{UiEvent, UiKind};
use crate::i18n;
use crate::sender::ChatMessageSender;

use cs2_gsi::cfg::GsiCfg;
use cs2_gsi::events::PlayerDied;
use cs2_gsi::GameStateListener;

use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Default port the listener binds to (matches the original Godot project).
pub const DEFAULT_PORT: u16 = 4000;

/// Internal name used for the auto-generated GSI cfg file.
pub const GSI_SERVICE_NAME: &str = "ImLag";

/// All long-lived state of the application.
#[derive(Clone)]
pub struct Engine {
    config: Arc<RwLock<AppConfig>>,
    corpus: ChatMessageManager,
    sender: ChatMessageSender,
    cfg_manager: CfgManager,
    listener: GameStateListener,
    ui_tx: broadcast::Sender<UiEvent>,
    data_dir: PathBuf,
}

impl Engine {
    /// Build a brand-new engine, loading config + corpus from `data_dir`.
    pub fn bootstrap(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();

        let cfg = AppConfig::load_from(&data_dir);
        i18n::set_language(&cfg.language);
        let config = Arc::new(RwLock::new(cfg));

        let corpus = ChatMessageManager::new();
        if let Err(e) = corpus.load_or_seed(&data_dir) {
            tracing::warn!("could not load corpus: {e}");
        }

        let sender = ChatMessageSender::new(config.clone());
        let cfg_manager = CfgManager::new(config.clone(), corpus.clone());
        let listener = GameStateListener::new(DEFAULT_PORT);
        let (ui_tx, _) = broadcast::channel::<UiEvent>(64);

        Self {
            config,
            corpus,
            sender,
            cfg_manager,
            listener,
            ui_tx,
            data_dir,
        }
    }

    /// Subscribe to UI status events. The receiver is independent of every
    /// other receiver — GUI tabs can each hold their own.
    pub fn subscribe_ui(&self) -> broadcast::Receiver<UiEvent> {
        self.ui_tx.subscribe()
    }

    /// Push a status event manually (used by GUI on imports / settings save).
    pub fn emit(&self, event: UiEvent) {
        let _ = self.ui_tx.send(event);
    }

    /// Cloneable handle to the config — write through it to mutate values
    /// then call [`save_config`](Self::save_config) to persist.
    pub fn config(&self) -> Arc<RwLock<AppConfig>> {
        self.config.clone()
    }

    /// Cloneable corpus handle.
    pub fn corpus(&self) -> ChatMessageManager {
        self.corpus.clone()
    }

    /// Cloneable cfg-mode controller.
    pub fn cfg_manager(&self) -> CfgManager {
        self.cfg_manager.clone()
    }

    /// Cloneable chat-mode sender.
    pub fn sender(&self) -> ChatMessageSender {
        self.sender.clone()
    }

    /// Snapshot of the current configuration.
    pub fn config_snapshot(&self) -> AppConfig {
        self.config.read().clone()
    }

    /// Persist the in-memory config to disk.
    pub fn save_config(&self) -> std::io::Result<PathBuf> {
        let cfg = self.config.read().clone();
        cfg.save_to(&self.data_dir)
    }

    /// Persist the in-memory corpus to disk.
    pub fn save_corpus(&self) -> std::io::Result<PathBuf> {
        self.corpus.save_to(&self.data_dir)
    }

    /// `true` once the GSI listener has been started.
    pub fn is_gsi_running(&self) -> bool {
        self.listener.is_running()
    }

    /// Start the GSI listener and write the integration cfg if needed.
    pub async fn start_gsi(&self) -> anyhow::Result<()> {
        // Best-effort: place the cfg file. Non-fatal if it fails — the user
        // may have configured it manually.
        match GsiCfg::for_localhost(GSI_SERVICE_NAME, DEFAULT_PORT).write_to_cs2() {
            Ok(p) => self.emit(UiEvent::info(
                UiKind::Cfg,
                format!("写入 GSI cfg: {}", p.display()),
            )),
            Err(e) => self.emit(UiEvent::warn(
                UiKind::Cfg,
                format!("GSI cfg 自动写入失败 (可手动放置): {e}"),
            )),
        }

        self.attach_handlers();
        self.listener.start().await?;
        self.emit(UiEvent::info(UiKind::Gsi, i18n::t("gsi.started")));
        Ok(())
    }

    /// Stop the GSI listener.
    pub async fn stop_gsi(&self) -> anyhow::Result<()> {
        if self.listener.is_running() {
            self.listener.stop().await?;
        }
        self.emit(UiEvent::info(UiKind::Gsi, i18n::t("gsi.stopped")));
        Ok(())
    }

    fn attach_handlers(&self) {
        let config = self.config.clone();
        let corpus = self.corpus.clone();
        let sender = self.sender.clone();
        let cfg_manager = self.cfg_manager.clone();
        let ui_tx = self.ui_tx.clone();

        self.listener.on(move |e: &PlayerDied| {
            let snapshot = config.read().clone();
            let dead_name = e.player.name.clone();

            let _ = ui_tx.send(UiEvent::info(
                UiKind::PlayerDeath,
                i18n::t_args("status.player_dead", [dead_name.as_str()].as_slice()),
            ));

            // Optional: only trigger when one of our watched names died.
            if snapshot.only_self_death {
                let watched = snapshot
                    .player_names
                    .iter()
                    .map(|s| s.trim().to_ascii_lowercase());
                let dead_low = dead_name.trim().to_ascii_lowercase();
                if !watched.into_iter().any(|w| w == dead_low) {
                    return;
                }
            }

            let message = match corpus.random() {
                Some(m) => m,
                None => return,
            };

            if snapshot.use_cfg_mode {
                let cm = cfg_manager.clone();
                let in_team = cm.select_in_team_chat();
                let ui_tx2 = ui_tx.clone();
                let msg2 = message.clone();
                tokio::task::spawn_blocking(move || match cm.dispatch(&msg2, in_team) {
                    Ok(()) => {
                        let _ = ui_tx2.send(UiEvent::info(
                            UiKind::ChatSent,
                            i18n::t_args("status.message_sent", [msg2.as_str()].as_slice()),
                        ));
                    }
                    Err(err) => {
                        let _ = ui_tx2.send(UiEvent::error(UiKind::Cfg, format!("{err}")));
                    }
                });
            } else {
                let sender = sender.clone();
                let ui_tx2 = ui_tx.clone();
                tokio::spawn(async move {
                    match sender.send_message(&message).await {
                        Ok(()) => {
                            let _ = ui_tx2.send(UiEvent::info(
                                UiKind::ChatSent,
                                i18n::t_args("status.message_sent", [message.as_str()].as_slice()),
                            ));
                        }
                        Err(err) => {
                            let _ = ui_tx2.send(UiEvent::error(UiKind::ChatSent, format!("{err}")));
                        }
                    }
                });
            }
        });
    }
}
