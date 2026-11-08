use horizon_client::mod_api::{
    ClientPacketEvent, ClientTickPreEvent, ClientboundPlayPacket, EntityDataValue, Hooks, Host,
    KeyEvent, NativeScreenKind, ServerboundPlayPacket, Text,
};
use horizon_protocol::packets::play::UseItem;
use std::{collections::BTreeMap, sync::Mutex};

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(AutoFishing::default())
}

const MOD_ID: &str = "auto_fishing";
const TOGGLE_KEY: &str = "key.keyboard.f8";
const FISHING_BOBBER_KIND: &str = "minecraft:fishing_bobber";
const BITE_DATA_INDEX: u8 = 9;
const MAX_CANDIDATE_AGE: u8 = 20;

#[derive(Default)]
struct AutoFishing {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    enabled: bool,
    player_id: Option<i32>,
    bobber_id: Option<u32>,
    candidates: BTreeMap<u32, Candidate>,
    next_sequence: i32,
    bite_pending: bool,
    action: Action,
}

#[derive(Clone, Copy, Default)]
struct Candidate {
    age: u8,
    biting: bool,
}

#[derive(Default)]
enum Action {
    #[default]
    Idle,
    Fishing,
    Reeling {
        ticks: u8,
    },
    RecastIn(u8),
    WaitingForBobber {
        ticks: u8,
    },
}

impl Hooks for AutoFishing {
    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        let toggle_key = host.mod_key(MOD_ID, "toggle", TOGGLE_KEY);
        if !event.pressed || event.key != toggle_key {
            return;
        }
        let Some(client) = host.client_state() else {
            return;
        };
        if client.screen != NativeScreenKind::Playing {
            return;
        }

        let held_rod = host
            .held()
            .is_some_and(|(item, _, _)| item == "minecraft:fishing_rod");
        let mut state = self.state.lock().expect("auto-fishing state poisoned");
        state.enabled = !state.enabled;
        if state.enabled {
            state.action = if state.bobber_id.is_some() && held_rod {
                Action::Fishing
            } else {
                Action::Idle
            };
        } else {
            state.bite_pending = false;
            state.action = Action::Idle;
        }
        let message = if !state.enabled {
            "Auto fishing OFF"
        } else if !held_rod {
            "Auto fishing ON - equip a fishing rod"
        } else if state.bobber_id.is_some() {
            "Auto fishing ON"
        } else {
            "Auto fishing ON - cast once with a fishing rod"
        };
        host.client_action_bar(Text::literal(message), true);
    }

    fn client_packet(&self, _host: &mut dyn Host, event: &mut ClientPacketEvent<'_>) {
        let mut state = self.state.lock().expect("auto-fishing state poisoned");
        match event.packet {
            ClientboundPlayPacket::Login(packet) => {
                state.player_id = Some(packet.player_id);
                state.bobber_id = None;
                state.candidates.clear();
                state.bite_pending = false;
                state.next_sequence = 1;
                state.action = Action::Idle;
            }
            ClientboundPlayPacket::BlockChangedAck(packet) => {
                let acknowledged_next = packet.sequence.wrapping_add(1);
                if acknowledged_next.wrapping_sub(state.next_sequence) > 0 {
                    state.next_sequence = acknowledged_next;
                }
            }
            ClientboundPlayPacket::AddEntity(packet) if state.player_id == Some(packet.data) => {
                if let Ok(id) = u32::try_from(packet.id) {
                    state.candidates.insert(id, Candidate::default());
                }
            }
            ClientboundPlayPacket::SetEntityData(packet) => {
                let id = u32::try_from(packet.id).ok();
                let biting = packet.values.iter().any(|(index, _, value)| {
                    *index == BITE_DATA_INDEX && matches!(value, EntityDataValue::Bool(true))
                });
                if biting && let Some(id) = id {
                    if let Some(candidate) = state.candidates.get_mut(&id) {
                        candidate.biting = true;
                    }
                    if state.bobber_id == Some(id) {
                        state.bite_pending = true;
                    }
                }
            }
            ClientboundPlayPacket::RemoveEntities(packet) => {
                for id in &packet.ids {
                    if let Ok(id) = u32::try_from(*id) {
                        state.candidates.remove(&id);
                        if state.bobber_id == Some(id) {
                            state.bobber_id = None;
                            state.bite_pending = false;
                            state.action = if state.enabled {
                                Action::RecastIn(2)
                            } else {
                                Action::Idle
                            };
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn client_tick_pre(&self, host: &mut dyn Host, _event: &mut ClientTickPreEvent) {
        let Some(client) = host.client_state() else {
            return;
        };

        let candidate_ids = {
            let state = self.state.lock().expect("auto-fishing state poisoned");
            if !client.logged_in || client.world.is_none() {
                return;
            }
            state.candidates.keys().copied().collect::<Vec<_>>()
        };

        let fishing_bobbers = candidate_ids
            .into_iter()
            .filter(|id| {
                host.entity(*id)
                    .and_then(|entity| host.entity_kind(entity))
                    .is_some_and(|kind| kind == FISHING_BOBBER_KIND)
            })
            .collect::<Vec<_>>();

        let mut state = self.state.lock().expect("auto-fishing state poisoned");
        for id in fishing_bobbers {
            if let Some(candidate) = state.candidates.remove(&id) {
                state.bobber_id = Some(id);
                state.bite_pending |= candidate.biting;
                if state.enabled
                    && matches!(state.action, Action::Idle | Action::WaitingForBobber { .. })
                {
                    state.action = Action::Fishing;
                }
            }
        }
        state.candidates.retain(|_, candidate| {
            candidate.age = candidate.age.saturating_add(1);
            candidate.age <= MAX_CANDIDATE_AGE
        });

        let held_rod = host
            .held()
            .is_some_and(|(item, _, _)| item == "minecraft:fishing_rod");
        if !state.enabled || client.screen != NativeScreenKind::Playing || !held_rod {
            return;
        }

        let mut send_use = false;
        match std::mem::take(&mut state.action) {
            Action::Fishing if state.bite_pending => {
                state.bite_pending = false;
                state.action = Action::Reeling { ticks: 0 };
                send_use = true;
            }
            Action::RecastIn(0) => {
                state.action = Action::WaitingForBobber { ticks: 0 };
                send_use = true;
            }
            Action::RecastIn(ticks) => {
                state.action = Action::RecastIn(ticks - 1);
            }
            Action::Reeling { ticks } => {
                if ticks >= 20 {
                    state.enabled = false;
                    state.action = Action::Idle;
                    host.client_action_bar(
                        Text::literal("Auto fishing paused - the bobber did not reel in"),
                        true,
                    );
                } else {
                    state.action = Action::Reeling {
                        ticks: ticks.saturating_add(1),
                    };
                }
            }
            Action::WaitingForBobber { ticks } => {
                if ticks >= 20 {
                    state.enabled = false;
                    state.action = Action::Idle;
                    host.client_action_bar(
                        Text::literal("Auto fishing paused - no bobber appeared"),
                        true,
                    );
                } else {
                    state.action = Action::WaitingForBobber {
                        ticks: ticks.saturating_add(1),
                    };
                }
            }
            action => {
                state.action = action;
            }
        }

        if !send_use {
            return;
        }
        let packet = ServerboundPlayPacket::UseItem(UseItem {
            hand: 0,
            sequence: state.next_sequence,
            yaw: client.yaw,
            pitch: client.pitch,
        });
        state.next_sequence = state.next_sequence.wrapping_add(1);
        if let Err(error) = host.send_packet(packet) {
            state.enabled = false;
            state.action = Action::Idle;
            host.client_action_bar(
                Text::literal(format!("Auto fishing stopped: {error}")),
                true,
            );
        }
    }
}
