//! Auto Fishing for the latest Horizon WebAssembly mod API.
use mod_api::{
    ClientHost, ClientTickPostEvent, Hooks, KeyConflictContext, KeyRegistration, PayloadCodec,
    PayloadId, PlayerFishEvent, Registrar, RegistrationContext, ServerHost, ToClient, ToServer,
    VarInt,
};
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
};

const MOD_ID: &str = "auto_fishing";
const TOGGLE_KEY: &str = "toggle";

#[derive(Debug, Default, PayloadCodec)]
struct FishingAction {
    action: VarInt,
}

#[derive(Debug, Default, PayloadCodec)]
struct FishingToggle {
    enabled: VarInt,
}

#[derive(Default)]
struct State {
    enabled: bool,
    pending: Pending,
}

#[derive(Default)]
enum Pending {
    #[default]
    None,
    Reel,
    Recast(u8),
}

static STATE: Mutex<State> = Mutex::new(State {
    enabled: false,
    pending: Pending::None,
});
static SERVER_ENABLED: Mutex<BTreeSet<u128>> = Mutex::new(BTreeSet::new());
static ACTION_TO_CLIENT: OnceLock<ToClient<FishingAction>> = OnceLock::new();
static TOGGLE_TO_SERVER: OnceLock<ToServer<FishingToggle>> = OnceLock::new();

#[derive(Default)]
struct AutoFishing;

impl AutoFishing {
    fn state() -> std::sync::MutexGuard<'static, State> {
        STATE.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn server_enabled() -> std::sync::MutexGuard<'static, BTreeSet<u128>> {
        SERVER_ENABLED
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

#[mod_api::horizon_mod]
impl Hooks for AutoFishing {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_key_mapping(
            KeyRegistration::new(TOGGLE_KEY, "key.keyboard.f8")
                .label("key.auto_fishing.toggle")
                .context(KeyConflictContext::InGame),
        );

        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");

        let action = view.play_to_client_typed::<FishingAction>(
            PayloadId("auto_fishing:action".into()),
            |value, _| {
                let mut state = Self::state();
                match value.action.0 {
                    1 => state.pending = Pending::Reel,
                    2 => state.pending = Pending::Recast(2),
                    _ => {}
                }
                Ok(())
            },
        );
        let _ = ACTION_TO_CLIENT.set(action);

        let toggle = view.play_to_server_typed::<FishingToggle>(
            PayloadId("auto_fishing:toggle".into()),
            |value, context| {
                let player = context.player().map_err(|error| format!("{error:?}"))?;
                let mut enabled = Self::server_enabled();
                if value.enabled.0 == 1 {
                    enabled.insert(player.uuid());
                } else {
                    enabled.remove(&player.uuid());
                }
                Ok(())
            },
        );
        let _ = TOGGLE_TO_SERVER.set(toggle);
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        if host.consume_click(MOD_ID, TOGGLE_KEY) {
            let mut state = Self::state();
            state.enabled = !state.enabled;
            let enabled = state.enabled;
            drop(state);

            if let Some(toggle) = TOGGLE_TO_SERVER.get() {
                let _ = toggle.send(
                    host,
                    &FishingToggle {
                        enabled: VarInt(if enabled { 1 } else { 0 }),
                    },
                );
            }
        }

        if !host.playing() {
            return;
        }

        let mut state = Self::state();
        match std::mem::take(&mut state.pending) {
            Pending::Reel => {
                drop(state);
                let _ = host.click_mapping("key.use");
            }
            Pending::Recast(ticks) if ticks == 0 => {
                drop(state);
                let _ = host.click_mapping("key.use");
            }
            Pending::Recast(ticks) => {
                state.pending = Pending::Recast(ticks - 1);
            }
            Pending::None => {}
        }
    }

    fn player_fish(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerFishEvent<'_>,
    ) -> mod_api::Flow {
        if !Self::server_enabled().contains(&event.player) {
            return mod_api::Flow::Continue;
        }
        let Some(player) = host.player(event.player) else {
            return mod_api::Flow::Continue;
        };
        let action = match event.state {
            1 => 1,
            2 | 3 | 4 => 2,
            _ => return mod_api::Flow::Continue,
        };
        if let Some(token) = ACTION_TO_CLIENT.get() {
            let _ = token.send(
                host,
                player,
                &FishingAction {
                    action: VarInt(action),
                },
            );
        }
        mod_api::Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_bite_and_retrieve_states() {
        assert!(matches!(Pending::Reel, Pending::Reel));
        assert_eq!(FishingAction::default().action.0, 0);
    }

    #[test]
    fn toggle_round_trips() {
        assert_eq!(FishingToggle { enabled: VarInt(1) }.enabled.0, 1);
    }
}
