//! Auto Fishing for the latest Horizon native mod API.
use mod_api::{
    ClientHost, ClientTickPostEvent, Hand, Hooks, InteractionAttempt, InteractionKey,
    InteractionKeyMappingTriggeredEvent, KeyConflictContext, KeyRegistration, LoopContext,
    PlaySoundEvent, Registrar, RegistrationContext, Text,
};
use std::sync::Mutex;

const MOD_ID: &str = "auto_fishing";
const TOGGLE_KEY: &str = "toggle";
const FISHING_ROD: &str = "minecraft:fishing_rod";
const BITE_SOUND: &str = "minecraft:entity.fishing_bobber.splash";
const MAX_BITE_SOUND_DISTANCE: f64 = 32.0;

#[derive(Default)]
struct State {
    enabled: bool,
    armed: bool,
    player_position: Option<[f64; 3]>,
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
    armed: false,
    player_position: None,
    pending: Pending::None,
});

#[derive(Default)]
struct AutoFishing;

impl AutoFishing {
    fn state() -> std::sync::MutexGuard<'static, State> {
        STATE.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn handle_bite_sound(state: &mut State, sound: &str, position: [f64; 3]) {
    if !state.enabled
        || !state.armed
        || sound != BITE_SOUND
        || !matches!(state.pending, Pending::None)
    {
        return;
    }

    let Some(player_position) = state.player_position else {
        return;
    };
    let distance_squared = (position[0] - player_position[0]).powi(2)
        + (position[1] - player_position[1]).powi(2)
        + (position[2] - player_position[2]).powi(2);
    if distance_squared <= MAX_BITE_SOUND_DISTANCE.powi(2) {
        state.pending = Pending::Reel;
    }
}

fn toggle_status(state: &mut State) -> &'static str {
    state.enabled = !state.enabled;
    if !state.enabled {
        state.armed = false;
        state.player_position = None;
        state.pending = Pending::None;
        "Auto Fishing disabled"
    } else {
        "Auto Fishing enabled"
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
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        if host.consume_click(MOD_ID, TOGGLE_KEY) {
            let status = {
                let mut state = Self::state();
                toggle_status(&mut state)
            };
            if host.playing() {
                if let Err(error) = host.chat_line(Text::literal(status)) {
                    eprintln!("[auto_fishing] failed to show toggle status: {error:?}");
                }
            }
        }

        if !host.playing() {
            let mut state = Self::state();
            state.armed = false;
            state.player_position = None;
            state.pending = Pending::None;
            return;
        }

        let local_player = host.local_player().and_then(|player| {
            let entity = player.entity();
            let position = entity.position(host)?;
            let living = host.living(entity)?;
            let holding_rod = Hand::BOTH.into_iter().any(|hand| {
                living
                    .hand_item(host, hand)
                    .is_some_and(|stack| stack.item().id(host) == FISHING_ROD)
            });
            Some((position, holding_rod))
        });

        let mut state = Self::state();
        let Some((position, holding_rod)) = local_player else {
            state.armed = false;
            state.player_position = None;
            state.pending = Pending::None;
            return;
        };
        state.player_position = Some(position);
        state.armed &= holding_rod;
        if !state.armed {
            state.pending = Pending::None;
            return;
        }

        match std::mem::take(&mut state.pending) {
            Pending::Reel => {
                state.pending = Pending::Recast(2);
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

    fn interaction_key_mapping_triggered(
        &self,
        host: &mut dyn ClientHost,
        event: &mut InteractionKeyMappingTriggeredEvent,
    ) -> mod_api::Flow {
        if event.key != InteractionKey::Use || event.attempt != InteractionAttempt::Press {
            return mod_api::Flow::Continue;
        }

        let Some(player) = host.local_player() else {
            return mod_api::Flow::Continue;
        };
        let Some(living) = host.living(player.entity()) else {
            return mod_api::Flow::Continue;
        };
        let holding_rod = living
            .hand_item(host, event.hand)
            .is_some_and(|stack| stack.item().id(host) == FISHING_ROD);
        if holding_rod {
            let mut state = Self::state();
            state.armed = state.enabled;
        }

        mod_api::Flow::Continue
    }

    fn play_sound(&self, _: &dyn LoopContext, event: &mut PlaySoundEvent<'_>) {
        if !event.starting || event.original.relative {
            return;
        }

        let mut state = Self::state();
        handle_bite_sound(
            &mut state,
            &event.original.identifier,
            event.original.position,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_disabled_without_a_pending_action() {
        let state = State::default();
        assert!(!state.enabled);
        assert!(matches!(state.pending, Pending::None));
    }

    #[test]
    fn toggle_reports_each_enabled_state() {
        let mut state = State::default();

        assert_eq!(toggle_status(&mut state), "Auto Fishing enabled");
        assert!(state.enabled);
        assert_eq!(toggle_status(&mut state), "Auto Fishing disabled");
        assert!(!state.enabled);
    }

    #[test]
    fn bite_sound_does_not_queue_another_reel_while_recasting() {
        let mut state = State {
            enabled: true,
            armed: true,
            player_position: Some([0.0, 64.0, 0.0]),
            pending: Pending::Recast(1),
        };

        handle_bite_sound(&mut state, BITE_SOUND, [12.0, 64.0, 0.0]);

        assert!(matches!(state.pending, Pending::Recast(1)));
    }

    #[test]
    fn disabled_or_unarmed_state_ignores_a_bobber_splash() {
        for (enabled, armed) in [(false, true), (true, false)] {
            let mut state = State {
                enabled,
                armed,
                player_position: Some([0.0, 64.0, 0.0]),
                pending: Pending::None,
            };

            handle_bite_sound(&mut state, BITE_SOUND, [12.0, 64.0, 0.0]);

            assert!(matches!(state.pending, Pending::None));
        }
    }

    #[test]
    fn a_nearby_bobber_splash_queues_a_reel_only_while_armed() {
        let mut state = State {
            enabled: true,
            armed: true,
            player_position: Some([0.0, 64.0, 0.0]),
            pending: Pending::None,
        };

        handle_bite_sound(
            &mut state,
            "minecraft:entity.fishing_bobber.splash",
            [12.0, 64.0, 0.0],
        );

        assert!(matches!(state.pending, Pending::Reel));
    }

    #[test]
    fn unrelated_or_distant_sounds_do_not_queue_a_reel() {
        for (sound, position) in [
            ("minecraft:entity.fishing_bobber.throw", [12.0, 64.0, 0.0]),
            ("minecraft:entity.fishing_bobber.splash", [100.0, 64.0, 0.0]),
        ] {
            let mut state = State {
                enabled: true,
                armed: true,
                player_position: Some([0.0, 64.0, 0.0]),
                pending: Pending::None,
            };

            handle_bite_sound(&mut state, sound, position);

            assert!(matches!(state.pending, Pending::None));
        }
    }
}
