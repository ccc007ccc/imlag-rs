//! Top-level engine — wires GSI events to the corpus / sender / cfg manager
//! and forwards UI status to the GUI via a [`tokio::sync::broadcast`] channel.

use crate::cfg_manager::CfgManager;
use crate::chat::ChatMessageManager;
use crate::config::AppConfig;
use crate::events::{UiEvent, UiKind};
use crate::i18n;
use crate::sender::ChatMessageSender;

use cs2_gsi::cfg::GsiCfg;
use cs2_gsi::events::{PlayerDied, PlayerGotKill};
use cs2_gsi::GameStateListener;

use parking_lot::RwLock;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Default port the listener binds to.
///
/// Tried 47474 first (IANA registered range, no known squatters); but
/// the user reports it's persistently held on at least one machine,
/// almost certainly by a security tool / driver that opens it
/// transparently to scan. 57534 is in the OS ephemeral range (so a
/// stray outbound connection *could* get assigned to it), but the
/// fallback walk in [`start_gsi`] makes that recoverable, and on the
/// affected machine 57534 has been observed to bind cleanly.
pub const DEFAULT_PORT: u16 = 57534;

/// How many ports immediately above [`DEFAULT_PORT`] to try before
/// asking the OS to assign one. Five gives generous head-room without
/// fanning out into territory we haven't reasoned about.
const PORT_FALLBACK_COUNT: u16 = 5;

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
    ///
    /// Tries [`DEFAULT_PORT`] first, falling back through
    /// `DEFAULT_PORT+1..=DEFAULT_PORT+PORT_FALLBACK_COUNT` and finally
    /// to an OS-assigned ephemeral port. The cfg is written *after* the
    /// listener is bound, with the actually-bound port baked in — so
    /// CS2 always points at the right place even if a security tool /
    /// driver is squatting on [`DEFAULT_PORT`].
    pub async fn start_gsi(&self) -> anyhow::Result<()> {
        self.attach_handlers();

        let fallbacks: Vec<SocketAddr> = (1..=PORT_FALLBACK_COUNT)
            .map(|offset| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_PORT + offset))
            // Final fallback: port 0 → OS picks any free ephemeral. Bind
            // here cannot really fail.
            .chain(std::iter::once(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                0,
            )))
            .collect();

        self.listener.start_with_fallbacks(fallbacks).await?;
        let bound_port = self
            .listener
            .actual_addr()
            .map(|a| a.port())
            .unwrap_or(DEFAULT_PORT);

        if bound_port != DEFAULT_PORT {
            self.emit(UiEvent::warn(
                UiKind::Gsi,
                format!(
                    "首选端口 {DEFAULT_PORT} 被占用，已切换到 {bound_port}（CS2 cfg 同步更新）"
                ),
            ));
        }

        // Best-effort: place the cfg file with the *actual* bound port.
        // Non-fatal if it fails — the user may have configured it manually.
        match GsiCfg::for_localhost(GSI_SERVICE_NAME, bound_port).write_to_cs2() {
            Ok(p) => self.emit(UiEvent::info(
                UiKind::Cfg,
                format!("写入 GSI cfg: {} (端口 {bound_port})", p.display()),
            )),
            Err(e) => self.emit(UiEvent::warn(
                UiKind::Cfg,
                format!("GSI cfg 自动写入失败 (可手动放置): {e}"),
            )),
        }

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
        // PlayerDied — fires when the GSI-visible player dies. With
        // only_self_death=true we additionally gate on player_names so
        // teammates / spectated bots don't fire chat replies.
        {
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

                // "Only fire on watched-list deaths" — when the toggle
                // is on we filter; when it's off, every PlayerDied is
                // a candidate (and PlayerGotKill below adds kills too).
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

                dispatch_chat_reply(
                    &snapshot,
                    &corpus,
                    &cfg_manager,
                    &sender,
                    &ui_tx,
                    DispatchTrigger::Death,
                );
            });
        }

        // PlayerGotKill — only meaningful when only_self_death is OFF
        // (the toggle's intent is "any combat event triggers a reply").
        // GSI never emits PlayerDied for unspectated bots, so without
        // this branch killing a bot would silently produce nothing.
        {
            let config = self.config.clone();
            let corpus = self.corpus.clone();
            let sender = self.sender.clone();
            let cfg_manager = self.cfg_manager.clone();
            let ui_tx = self.ui_tx.clone();

            self.listener.on(move |e: &PlayerGotKill| {
                let snapshot = config.read().clone();
                if snapshot.only_self_death {
                    // Strict mode: chat replies are gated by death-only.
                    return;
                }

                let killer = e.player.name.clone();
                let _ = ui_tx.send(UiEvent::info(
                    UiKind::PlayerDeath,
                    i18n::t_args("status.player_kill", [killer.as_str()].as_slice()),
                ));

                dispatch_chat_reply(
                    &snapshot,
                    &corpus,
                    &cfg_manager,
                    &sender,
                    &ui_tx,
                    DispatchTrigger::Kill,
                );
            });
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum DispatchTrigger {
    Death,
    Kill,
}

impl DispatchTrigger {
    /// i18n key for the human-readable label of this trigger
    /// (`"死亡"` / `"擊殺"` / `"death"` / …).
    fn label_key(self) -> &'static str {
        match self {
            DispatchTrigger::Death => "trigger.death",
            DispatchTrigger::Kill => "trigger.kill",
        }
    }
}

/// Pick a corpus line and route it through the configured delivery path
/// (cfg-mode press → in-game `say`, or auto-typing into the chat box).
/// Errors are forwarded to the UI so the user can see *why* a message
/// didn't go out.
fn dispatch_chat_reply(
    snapshot: &AppConfig,
    corpus: &ChatMessageManager,
    cfg_manager: &CfgManager,
    sender: &ChatMessageSender,
    ui_tx: &broadcast::Sender<UiEvent>,
    trigger: DispatchTrigger,
) {
    let Some(message) = corpus.random() else {
        let _ = ui_tx.send(UiEvent::warn(
            UiKind::ChatSent,
            i18n::t("status.corpus_empty"),
        ));
        return;
    };

    if snapshot.use_cfg_mode {
        let cm = cfg_manager.clone();
        let in_team = cm.select_in_team_chat();
        let ui_tx = ui_tx.clone();
        let msg = message.clone();
        tokio::task::spawn_blocking(move || match cm.dispatch(&msg, in_team) {
            Ok(()) => {
                let _ = ui_tx.send(UiEvent::info(
                    UiKind::ChatSent,
                    i18n::t_args("status.message_sent", [msg.as_str()].as_slice()),
                ));
            }
            Err(err) => {
                let label = i18n::t(trigger.label_key());
                let _ = ui_tx.send(UiEvent::error(
                    UiKind::Cfg,
                    i18n::t_args(
                        "dispatch.cfg_failed",
                        [label.as_str(), err.to_string().as_str()].as_slice(),
                    ),
                ));
            }
        });
    } else {
        let sender = sender.clone();
        let ui_tx = ui_tx.clone();
        tokio::spawn(async move {
            match sender.send_message(&message).await {
                Ok(()) => {
                    let _ = ui_tx.send(UiEvent::info(
                        UiKind::ChatSent,
                        i18n::t_args("status.message_sent", [message.as_str()].as_slice()),
                    ));
                }
                Err(err) => {
                    let label = i18n::t(trigger.label_key());
                    let _ = ui_tx.send(UiEvent::error(
                        UiKind::ChatSent,
                        i18n::t_args(
                            "dispatch.chat_failed",
                            [label.as_str(), err.to_string().as_str()].as_slice(),
                        ),
                    ));
                }
            }
        });
    }
}
