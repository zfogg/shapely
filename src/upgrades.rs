//! Permanent rewards offered after a room is cleared.
//!
//! This module owns the upgrade catalog, deterministic room reward rolls, the
//! animated reward-chest UI, and the input gate that requires one selection.

use bevy::prelude::*;

use crate::{BASE_HEALTH, BASE_MANA};

/// The permanent upgrades that can appear in a cleared-room chest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpgradeKind {
    Speed,
    AttackSpeed,
    Power,
    ExtraAttack,
    ExtraHealth,
    ExtraMana,
    ExtraHealthMana,
}

/// Marker for the cleared-room reward panel.
#[derive(Component)]
pub(crate) struct UpgradePanel;

/// Marker for the text populated once the chest finishes opening.
#[derive(Component)]
pub(crate) struct UpgradeChoicesText;

/// Marker for the animated chest lid.
#[derive(Component)]
pub(crate) struct ChestLid;

/// Returns the display name and rarity/effect description for an upgrade.
pub(crate) fn upgrade_label(kind: UpgradeKind) -> (&'static str, &'static str) {
    match kind {
        UpgradeKind::Speed => ("SPEED", "COMMON  //  move faster"),
        UpgradeKind::AttackSpeed => ("ATTACK SPEED", "COMMON  //  faster cooldowns"),
        UpgradeKind::Power => ("POWER", "COMMON  //  1.18x damage"),
        UpgradeKind::ExtraAttack => ("EXTRA ATTACK", "RARE  //  fire twice"),
        UpgradeKind::ExtraHealth => ("EXTRA HEALTH", "UNCOMMON  //  +20 max HP, full heal"),
        UpgradeKind::ExtraMana => ("EXTRA MANA", "UNCOMMON  //  +20 max mana, full refill"),
        UpgradeKind::ExtraHealthMana => (
            "EXTRA HEALTH + MANA",
            "RARE  //  +20 max HP and mana, refill both",
        ),
    }
}

/// Rolls three deterministic, distinct upgrades for a room.
pub(crate) fn upgrade_choices(room: IVec2) -> [UpgradeKind; 3] {
    const POOL: [UpgradeKind; 12] = [
        UpgradeKind::Speed,
        UpgradeKind::AttackSpeed,
        UpgradeKind::Power,
        UpgradeKind::Speed,
        UpgradeKind::AttackSpeed,
        UpgradeKind::Power,
        UpgradeKind::ExtraHealth,
        UpgradeKind::ExtraMana,
        UpgradeKind::ExtraHealth,
        UpgradeKind::ExtraMana,
        UpgradeKind::ExtraAttack,
        UpgradeKind::ExtraHealthMana,
    ];
    let seed = crate::room::seed(room) as usize;
    let mut choices = [UpgradeKind::Speed; 3];
    let mut count = 0;
    let mut offset = 0;
    while count < choices.len() {
        let candidate = POOL[(seed + offset * 5) % POOL.len()];
        if !choices[..count].contains(&candidate) {
            choices[count] = candidate;
            count += 1;
        }
        offset += 1;
    }
    choices
}

/// Applies one permanent reward to the run state.
pub(crate) fn apply_upgrade(state: &mut crate::GameState, kind: UpgradeKind) {
    match kind {
        UpgradeKind::Speed => state.move_speed_multiplier *= 1.12,
        UpgradeKind::AttackSpeed => state.cooldown_multiplier *= 0.85,
        UpgradeKind::Power => state.power_multiplier *= 1.18,
        UpgradeKind::ExtraAttack => state.extra_attack = true,
        UpgradeKind::ExtraHealth => {
            state.max_health += (BASE_HEALTH as f32 * 0.20) as i32;
            state.health = state.max_health;
        }
        UpgradeKind::ExtraMana => {
            state.max_mana += (BASE_MANA as f32 * 0.20) as i32;
            state.mana = state.max_mana;
        }
        UpgradeKind::ExtraHealthMana => {
            state.max_health += (BASE_HEALTH as f32 * 0.20) as i32;
            state.max_mana += (BASE_MANA as f32 * 0.20) as i32;
            state.health = state.max_health;
            state.mana = state.max_mana;
        }
    }
}

/// Builds the centered reward chest overlay.
pub(crate) fn spawn_upgrade_overlay(commands: &mut Commands, font: &Handle<Font>) {
    let panel = commands
        .spawn((
            UpgradePanel,
            NodeBundle {
                style: Style {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: Val::Percent(17.0),
                    top: Val::Percent(12.0),
                    width: Val::Percent(66.0),
                    height: Val::Percent(76.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.025, 0.035, 0.12, 0.97)),
                ..default()
            },
        ))
        .id();
    commands.entity(panel).with_children(|parent| {
        parent
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Px(620.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ..default()
            })
            .with_children(|content| {
                content.spawn(TextBundle {
                    text: Text::from_section(
                        "ROOM CLEARED",
                        TextStyle {
                            font: font.clone(),
                            font_size: 30.0,
                            color: Color::srgb(1.0, 0.75, 0.18),
                        },
                    ),
                    ..default()
                });
                content.spawn(TextBundle {
                    text: Text::from_section(
                        "A REWARD CHEST APPEARS",
                        TextStyle {
                            font: font.clone(),
                            font_size: 17.0,
                            color: Color::srgb(0.55, 0.95, 1.0),
                        },
                    ),
                    style: Style {
                        margin: UiRect::top(Val::Px(5.0)),
                        ..default()
                    },
                    ..default()
                });
                content
                    .spawn(NodeBundle {
                        style: Style {
                            position_type: PositionType::Relative,
                            width: Val::Px(190.0),
                            height: Val::Px(100.0),
                            margin: UiRect::vertical(Val::Px(18.0)),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.55, 0.22, 0.06)),
                        ..default()
                    })
                    .with_children(|chest| {
                        chest.spawn((
                            ChestLid,
                            NodeBundle {
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(0.0),
                                    width: Val::Px(190.0),
                                    height: Val::Px(34.0),
                                    ..default()
                                },
                                background_color: BackgroundColor(Color::srgb(1.0, 0.68, 0.12)),
                                ..default()
                            },
                        ));
                        chest.spawn(NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(82.0),
                                top: Val::Px(34.0),
                                width: Val::Px(26.0),
                                height: Val::Px(30.0),
                                ..default()
                            },
                            background_color: BackgroundColor(Color::srgb(1.0, 0.84, 0.2)),
                            ..default()
                        });
                    });
                content.spawn((
                    UpgradeChoicesText,
                    TextBundle {
                        text: Text::from_section(
                            "OPENING CHEST...",
                            TextStyle {
                                font: font.clone(),
                                font_size: 19.0,
                                color: Color::WHITE,
                            },
                        ),
                        style: Style {
                            width: Val::Px(590.0),
                            ..default()
                        },
                        ..default()
                    },
                ));
                content.spawn(TextBundle {
                    text: Text::from_section(
                        "Choose exactly one with 1, 2, or 3",
                        TextStyle {
                            font: font.clone(),
                            font_size: 16.0,
                            color: Color::srgb(0.55, 0.95, 1.0),
                        },
                    ),
                    style: Style {
                        margin: UiRect::top(Val::Px(14.0)),
                        ..default()
                    },
                    ..default()
                });
            });
    });
}

/// Opens the chest over time and reveals the three choices at the end.
pub(crate) fn update_upgrade_overlay(
    time: Res<Time>,
    mut state: ResMut<crate::GameState>,
    mut choices: Query<&mut Text, With<UpgradeChoicesText>>,
    mut lids: Query<&mut Style, With<ChestLid>>,
) {
    if state.mode == crate::GameMode::Upgrade {
        state.chest_open_progress =
            (state.chest_open_progress + time.delta_seconds() / 1.25).min(1.0);
    } else {
        state.chest_open_progress = 0.0;
    }
    for mut style in &mut lids {
        style.top = Val::Px(-26.0 * state.chest_open_progress);
    }
    if let Ok(mut text) = choices.get_single_mut() {
        text.sections[0].value = if state.chest_open_progress < 1.0 {
            "OPENING CHEST...".into()
        } else {
            let [a, b, c] = state.upgrade_choices;
            let (an, ar) = upgrade_label(a);
            let (bn, br) = upgrade_label(b);
            let (cn, cr) = upgrade_label(c);
            format!("[1] {an}\n    {ar}\n\n[2] {bn}\n    {br}\n\n[3] {cn}\n    {cr}")
        };
    }
}

/// Accepts exactly one of the three choices, and only after the chest opens.
pub(crate) fn choose_upgrade(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<crate::GameState>) {
    if state.mode != crate::GameMode::Upgrade || state.chest_open_progress < 1.0 {
        return;
    }
    let choice = if keys.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else {
        None
    };
    let Some(choice) = choice else { return };
    let kind = state.upgrade_choices[choice];
    apply_upgrade(&mut state, kind);
    state.mode = crate::GameMode::Playing;
    state.chest_open_progress = 0.0;
    state.room_notice = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_room_roll_is_deterministic_and_distinct() {
        for y in -1..=1 {
            for x in -1..=1 {
                let first = upgrade_choices(IVec2::new(x, y));
                assert_eq!(first, upgrade_choices(IVec2::new(x, y)));
                assert_ne!(first[0], first[1]);
                assert_ne!(first[0], first[2]);
                assert_ne!(first[1], first[2]);
            }
        }
    }

    #[test]
    fn room_rolls_are_not_all_identical() {
        let first = upgrade_choices(IVec2::new(-1, -1));
        assert!((-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| IVec2::new(x, y)))
            .any(|room| room != IVec2::new(-1, -1) && upgrade_choices(room) != first));
    }

    #[test]
    fn every_upgrade_has_a_name_and_known_rarity() {
        for kind in [
            UpgradeKind::Speed,
            UpgradeKind::AttackSpeed,
            UpgradeKind::Power,
            UpgradeKind::ExtraAttack,
            UpgradeKind::ExtraHealth,
            UpgradeKind::ExtraMana,
            UpgradeKind::ExtraHealthMana,
        ] {
            let (name, description) = upgrade_label(kind);
            assert!(!name.is_empty());
            assert!(
                description.starts_with("COMMON")
                    || description.starts_with("UNCOMMON")
                    || description.starts_with("RARE")
            );
        }
    }

    #[test]
    fn each_stat_upgrade_changes_only_its_stat() {
        let mut speed = crate::GameState::default();
        apply_upgrade(&mut speed, UpgradeKind::Speed);
        assert_eq!(speed.move_speed_multiplier, 1.12);
        assert_eq!(speed.power_multiplier, 1.0);

        let mut attack_speed = crate::GameState::default();
        apply_upgrade(&mut attack_speed, UpgradeKind::AttackSpeed);
        assert_eq!(attack_speed.cooldown_multiplier, 0.85);
        assert_eq!(attack_speed.power_multiplier, 1.0);

        let mut power = crate::GameState::default();
        apply_upgrade(&mut power, UpgradeKind::Power);
        assert_eq!(power.power_multiplier, 1.18);
        assert_eq!(power.move_speed_multiplier, 1.0);
    }

    #[test]
    fn extra_attack_is_enabled_and_safe_to_stack() {
        let mut state = crate::GameState::default();
        apply_upgrade(&mut state, UpgradeKind::ExtraAttack);
        apply_upgrade(&mut state, UpgradeKind::ExtraAttack);
        assert!(state.extra_attack);
    }

    #[test]
    fn health_upgrade_adds_base_health_and_heals() {
        let mut state = crate::GameState::default();
        state.health = 1;
        apply_upgrade(&mut state, UpgradeKind::ExtraHealth);
        assert_eq!((state.max_health, state.health), (120, 120));
    }

    #[test]
    fn mana_upgrade_adds_base_mana_and_refills() {
        let mut state = crate::GameState::default();
        state.mana = 1;
        apply_upgrade(&mut state, UpgradeKind::ExtraMana);
        assert_eq!((state.max_mana, state.mana), (120, 120));
    }

    #[test]
    fn health_and_mana_upgrade_refills_both_resources() {
        let mut state = crate::GameState::default();
        state.health = 1;
        state.mana = 1;
        apply_upgrade(&mut state, UpgradeKind::ExtraHealthMana);
        assert_eq!((state.max_health, state.health), (120, 120));
        assert_eq!((state.max_mana, state.mana), (120, 120));
    }

    fn choose_upgrade_in_test(kind: UpgradeKind, key: KeyCode) -> crate::GameState {
        let mut state = crate::GameState::default();
        state.mode = crate::GameMode::Upgrade;
        state.chest_open_progress = 1.0;
        state.upgrade_choices = [kind, UpgradeKind::Speed, UpgradeKind::Power];
        let mut app = App::new();
        app.insert_resource(state)
            .insert_resource(ButtonInput::<KeyCode>::default())
            .add_systems(Update, choose_upgrade);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut().remove_resource::<ButtonInput<KeyCode>>();
        app.world_mut()
            .remove_resource::<crate::GameState>()
            .unwrap()
    }

    #[test]
    fn upgrade_choice_is_locked_until_the_chest_finishes_opening() {
        let mut state = crate::GameState::default();
        state.mode = crate::GameMode::Upgrade;
        state.chest_open_progress = 0.99;
        state.upgrade_choices = [
            UpgradeKind::Power,
            UpgradeKind::Speed,
            UpgradeKind::AttackSpeed,
        ];
        let mut app = App::new();
        app.insert_resource(state)
            .insert_resource(ButtonInput::<KeyCode>::default())
            .add_systems(Update, choose_upgrade);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();
        let state = app.world().resource::<crate::GameState>();
        assert_eq!(state.mode, crate::GameMode::Upgrade);
        assert_eq!(state.power_multiplier, 1.0);
    }

    #[test]
    fn each_visible_choice_key_selects_an_upgrade() {
        for (key, kind) in [
            (KeyCode::Digit1, UpgradeKind::Power),
            (KeyCode::Digit2, UpgradeKind::Speed),
            (KeyCode::Digit3, UpgradeKind::AttackSpeed),
        ] {
            assert_eq!(
                choose_upgrade_in_test(kind, key).mode,
                crate::GameMode::Playing
            );
        }
    }

    #[test]
    fn keys_outside_the_three_visible_choices_are_ignored() {
        let state = choose_upgrade_in_test(UpgradeKind::Power, KeyCode::Digit4);
        assert_eq!(state.mode, crate::GameMode::Upgrade);
        assert_eq!(state.power_multiplier, 1.0);
    }

    #[test]
    fn selecting_an_upgrade_closes_the_chest_and_clears_the_notice() {
        let state = choose_upgrade_in_test(UpgradeKind::Speed, KeyCode::Digit1);
        assert_eq!(state.mode, crate::GameMode::Playing);
        assert_eq!(state.chest_open_progress, 0.0);
        assert_eq!(state.room_notice, 0.0);
    }

    #[test]
    fn upgrade_selection_changes_only_the_selected_reward_path() {
        let state = choose_upgrade_in_test(UpgradeKind::Power, KeyCode::Digit1);
        assert_eq!(state.power_multiplier, 1.18);
        assert_eq!(state.move_speed_multiplier, 1.0);
        assert_eq!(state.cooldown_multiplier, 1.0);
    }

    #[test]
    fn repeated_health_and_mana_rewards_stack_by_twenty_base_points() {
        let mut state = crate::GameState::default();
        apply_upgrade(&mut state, UpgradeKind::ExtraHealth);
        apply_upgrade(&mut state, UpgradeKind::ExtraHealth);
        apply_upgrade(&mut state, UpgradeKind::ExtraMana);
        apply_upgrade(&mut state, UpgradeKind::ExtraMana);
        assert_eq!(state.max_health, 140);
        assert_eq!(state.max_mana, 140);
        assert_eq!(state.health, 140);
        assert_eq!(state.mana, 140);
    }
}
