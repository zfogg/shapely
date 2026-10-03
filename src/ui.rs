//! Menus, HUD widgets, and presentation-only update systems.

use bevy::prelude::*;

use crate::damageable::Damageable;
use crate::enemy::Enemy;
use crate::items::ItemKind;
use crate::upgrades::UpgradePanel;

#[derive(Component)]
pub(crate) struct Hud;
#[derive(Component)]
pub(crate) struct HealthBarFill;
#[derive(Component)]
pub(crate) struct InventoryPanel;
#[derive(Component)]
pub(crate) struct InventoryText;
#[derive(Component)]
pub(crate) struct EnemyHealthBar {
    pub(crate) max_health: i32,
}
#[derive(Component)]
pub(crate) struct EnemyName;
#[derive(Component)]
pub(crate) struct Crosshair;
#[derive(Component)]
pub(crate) struct TitlePanel;
#[derive(Component)]
pub(crate) struct PausePanel;
#[derive(Component)]
pub(crate) struct SettingsPanel;
#[derive(Component)]
pub(crate) struct DeathPanel;
#[derive(Component)]
pub(crate) struct RoomClearText;
#[derive(Component)]
pub(crate) struct FloatingText {
    pub(crate) velocity: Vec2,
}
#[derive(Component)]
pub(crate) struct MenuButton(pub(crate) MenuAction);
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum MenuAction {
    Play,
    Resume,
    Settings,
    Back,
    Restart,
    Quit,
}

pub(crate) fn spawn_hud(commands: &mut Commands, assets: &Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        Hud,
        TextBundle {
            text: Text::from_section(
                "",
                TextStyle {
                    font: font.clone(),
                    font_size: 18.0,
                    color: Color::WHITE,
                },
            ),
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                top: Val::Px(18.0),
                ..default()
            },
            ..default()
        },
    ));
    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                top: Val::Px(72.0),
                width: Val::Px(220.0),
                height: Val::Px(18.0),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.12, 0.02, 0.05, 0.95)),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                HealthBarFill,
                NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    background_color: BackgroundColor(Color::srgb(1.0, 0.12, 0.25)),
                    ..default()
                },
            ));
        });
    commands
        .spawn((
            InventoryPanel,
            NodeBundle {
                style: Style {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: Val::Percent(18.0),
                    top: Val::Percent(12.0),
                    width: Val::Percent(64.0),
                    height: Val::Percent(70.0),
                    padding: UiRect::all(Val::Px(28.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.04, 0.055, 0.14, 0.97)),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                InventoryText,
                TextBundle {
                    text: Text::from_section(
                        "INVENTORY // 24 SLOTS",
                        TextStyle {
                            font: font.clone(),
                            font_size: 24.0,
                            color: Color::srgb(0.3, 0.9, 1.0),
                        },
                    ),
                    ..default()
                },
            ));
        });
    commands.spawn((
        Crosshair,
        TextBundle {
            text: Text::from_section(
                "✦",
                TextStyle {
                    font: font.clone(),
                    font_size: 22.0,
                    color: Color::srgb(0.3, 0.95, 1.0),
                },
            ),
            style: Style {
                position_type: PositionType::Absolute,
                ..default()
            },
            ..default()
        },
    ));
    commands.spawn((
        RoomClearText,
        TextBundle {
            text: Text::from_section(
                "",
                TextStyle {
                    font: font.clone(),
                    font_size: 22.0,
                    color: Color::srgb(0.4, 1.0, 0.65),
                },
            ),
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Percent(38.0),
                top: Val::Px(28.0),
                ..default()
            },
            ..default()
        },
    ));
    spawn_controls_overlay(commands, &font, TitlePanel, true);
    spawn_controls_overlay(commands, &font, PausePanel, false);
    spawn_overlay(
        commands,
        &font,
        SettingsPanel,
        "SETTINGS\n\nAudio cues: ON\nDisplay: neon\n\nPress Esc to return",
        false,
    );
    crate::upgrades::spawn_upgrade_overlay(commands, &font);
    spawn_overlay(
        commands,
        &font,
        DeathPanel,
        "YOU WERE SHATTERED\n\nPress R to RESTART",
        false,
    );
}

fn spawn_overlay<T: Component>(
    commands: &mut Commands,
    font: &Handle<Font>,
    marker: T,
    message: &str,
    title: bool,
) {
    let panel = commands
        .spawn((
            marker,
            NodeBundle {
                style: Style {
                    display: if title { Display::Flex } else { Display::None },
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
        parent.spawn(TextBundle {
            text: Text::from_section(
                message,
                TextStyle {
                    font: font.clone(),
                    font_size: if title { 27.0 } else { 22.0 },
                    color: Color::srgb(0.55, 0.95, 1.0),
                },
            ),
            ..default()
        });
    });
}

fn spawn_controls_overlay<T: Component>(
    commands: &mut Commands,
    font: &Handle<Font>,
    marker: T,
    title: bool,
) {
    let panel = commands
        .spawn((
            marker,
            NodeBundle {
                style: Style {
                    display: if title { Display::Flex } else { Display::None },
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
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ..default()
            })
            .with_children(|content| {
                content.spawn(TextBundle {
                    text: Text::from_section(
                        if title { "SHAPELY" } else { "PAUSED" },
                        TextStyle {
                            font: font.clone(),
                            font_size: 34.0,
                            color: Color::srgb(0.3, 0.95, 1.0),
                        },
                    ),
                    ..default()
                });
                content.spawn(TextBundle {
                    text: Text::from_section(
                        if title { "PLAY" } else { "RESUME" },
                        TextStyle {
                            font: font.clone(),
                            font_size: 27.0,
                            color: Color::srgb(0.3, 0.95, 1.0),
                        },
                    ),
                    style: Style {
                        margin: UiRect::bottom(Val::Px(28.0)),
                        ..default()
                    },
                    ..default()
                });
                content.spawn(TextBundle {
                    text: Text::from_section(
                        "ATTACK CONTROLS",
                        TextStyle {
                            font: font.clone(),
                            font_size: 21.0,
                            color: Color::srgb(1.0, 0.45, 0.75),
                        },
                    ),
                    style: Style {
                        margin: UiRect::bottom(Val::Px(10.0)),
                        ..default()
                    },
                    ..default()
                });
                content
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            margin: UiRect::bottom(Val::Px(5.0)),
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(TextBundle {
                            text: Text::from_section(
                                "ATTACK",
                                TextStyle {
                                    font: font.clone(),
                                    font_size: 17.0,
                                    color: Color::srgb(0.55, 0.95, 1.0),
                                },
                            ),
                            style: Style {
                                width: Val::Px(190.0),
                                ..default()
                            },
                            ..default()
                        });
                        row.spawn(TextBundle {
                            text: Text::from_section(
                                "KEYBOARD",
                                TextStyle {
                                    font: font.clone(),
                                    font_size: 17.0,
                                    color: Color::srgb(0.55, 0.95, 1.0),
                                },
                            ),
                            style: Style {
                                width: Val::Px(180.0),
                                ..default()
                            },
                            ..default()
                        });
                        row.spawn(TextBundle {
                            text: Text::from_section(
                                "MOUSE",
                                TextStyle {
                                    font: font.clone(),
                                    font_size: 17.0,
                                    color: Color::srgb(0.55, 0.95, 1.0),
                                },
                            ),
                            ..default()
                        });
                    });
                for (attack, keyboard, mouse) in [
                    ("LASERBEAM", "1", "G5"),
                    ("SLASH", "2", "Right-click"),
                    ("FIREBALL", "3", "G4"),
                ] {
                    content
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                margin: UiRect::bottom(Val::Px(7.0)),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn(TextBundle {
                                text: Text::from_section(
                                    attack,
                                    TextStyle {
                                        font: font.clone(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                style: Style {
                                    width: Val::Px(190.0),
                                    ..default()
                                },
                                ..default()
                            });
                            row.spawn(TextBundle {
                                text: Text::from_section(
                                    keyboard,
                                    TextStyle {
                                        font: font.clone(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                style: Style {
                                    width: Val::Px(180.0),
                                    ..default()
                                },
                                ..default()
                            });
                            row.spawn(TextBundle {
                                text: Text::from_section(
                                    mouse,
                                    TextStyle {
                                        font: font.clone(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                ..default()
                            });
                        });
                }
                content.spawn(TextBundle {
                    text: Text::from_section(
                        "WASD / ARROWS   move\nI   inventory\n? / Esc   pause",
                        TextStyle {
                            font: font.clone(),
                            font_size: 20.0,
                            color: Color::srgb(0.55, 0.95, 1.0),
                        },
                    ),
                    style: Style {
                        margin: UiRect::top(Val::Px(22.0)),
                        ..default()
                    },
                    ..default()
                });
                content.spawn(TextBundle {
                    text: Text::from_section(
                        if title {
                            "Press ENTER to PLAY"
                        } else {
                            "Press ? or Esc to resume\nQ   quit"
                        },
                        TextStyle {
                            font: font.clone(),
                            font_size: 20.0,
                            color: Color::srgb(0.55, 0.95, 1.0),
                        },
                    ),
                    style: Style {
                        margin: UiRect::top(Val::Px(24.0)),
                        ..default()
                    },
                    ..default()
                });
            });
    });
}

pub(crate) fn question_mark_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.just_pressed(KeyCode::Slash)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
}

pub(crate) fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<crate::GameState>,
    mut exit: EventWriter<AppExit>,
) {
    let question_mark = question_mark_pressed(&keys);
    match state.mode {
        crate::GameMode::Title => {
            if keys.just_pressed(KeyCode::Enter) {
                state.mode = crate::GameMode::Playing
            }
        }
        crate::GameMode::Playing => {
            if question_mark || (keys.just_pressed(KeyCode::Escape) && !state.inventory_open) {
                state.mode = crate::GameMode::Paused
            }
        }
        crate::GameMode::Paused => {
            if question_mark || keys.just_pressed(KeyCode::Escape) {
                state.mode = crate::GameMode::Playing
            }
        }
        crate::GameMode::Settings => {
            if keys.just_pressed(KeyCode::Escape) {
                state.mode = crate::GameMode::Title
            }
        }
        crate::GameMode::Upgrade | crate::GameMode::Dead => {}
    }
    if keys.just_pressed(KeyCode::KeyQ)
        && matches!(state.mode, crate::GameMode::Title | crate::GameMode::Paused)
    {
        exit.send(AppExit::Success);
    }
}

pub(crate) fn menu_button_interaction(
    mut state: ResMut<crate::GameState>,
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.mode = match button.0 {
            MenuAction::Play | MenuAction::Resume => crate::GameMode::Playing,
            MenuAction::Settings => crate::GameMode::Settings,
            MenuAction::Back => crate::GameMode::Title,
            MenuAction::Restart => crate::GameMode::Dead,
            MenuAction::Quit => state.mode,
        };
    }
}

pub(crate) fn update_menu_visibility(
    state: Res<crate::GameState>,
    mut panels: Query<
        (
            &mut Style,
            Option<&TitlePanel>,
            Option<&PausePanel>,
            Option<&SettingsPanel>,
            Option<&UpgradePanel>,
            Option<&DeathPanel>,
        ),
        Or<(
            With<TitlePanel>,
            With<PausePanel>,
            With<SettingsPanel>,
            With<UpgradePanel>,
            With<DeathPanel>,
        )>,
    >,
) {
    for (mut style, title, pause, settings, upgrade, death) in &mut panels {
        let visible = (title.is_some() && state.mode == crate::GameMode::Title)
            || (pause.is_some() && state.mode == crate::GameMode::Paused)
            || (settings.is_some() && state.mode == crate::GameMode::Settings)
            || (upgrade.is_some() && state.mode == crate::GameMode::Upgrade)
            || (death.is_some() && state.mode == crate::GameMode::Dead);
        style.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<crate::GameState>,
    mut panel: Query<&mut Style, With<InventoryPanel>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    if keys.just_pressed(KeyCode::KeyI) || keys.just_pressed(KeyCode::Escape) {
        state.inventory_open = if keys.just_pressed(KeyCode::Escape) {
            false
        } else {
            !state.inventory_open
        };
        if let Ok(mut style) = panel.get_single_mut() {
            style.display = if state.inventory_open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

pub(crate) fn update_hud(
    state: Res<crate::GameState>,
    mut hud: Query<&mut Text, (With<Hud>, Without<InventoryText>, Without<RoomClearText>)>,
    mut inventory_text: Query<
        &mut Text,
        (With<InventoryText>, Without<Hud>, Without<RoomClearText>),
    >,
    mut health_bar: Query<&mut Style, With<HealthBarFill>>,
    mut clear_text: Query<&mut Text, With<RoomClearText>>,
) {
    let inv = state
        .inventory
        .iter()
        .take(6)
        .map(|i| match i {
            ItemKind::HealthPotion => "♥",
            ItemKind::ManaPotion => "◆",
            ItemKind::SpeedPotion => "✦",
            ItemKind::AttackSpeedPotion => "⚡",
        })
        .collect::<Vec<_>>()
        .join(" ");
    if let Ok(mut text) = hud.get_single_mut() {
        text.sections[0].value = format!("SHAPELY  //  ROOM {:+},{:+}\nHP {:>3}/{}   MANA {:>3}/{}   [1] LASER  [2] SLASH  [3] FIREBALL\nQUICK SLOTS: {}   [I] INVENTORY   [H] HEALTH  [M] MANA  [B] BOOST", state.room.x, state.room.y, state.health, state.max_health, state.mana, state.max_mana, if inv.is_empty() { "—".into() } else { inv });
    }
    if let Ok(mut text) = inventory_text.get_single_mut() {
        let slots = (0..24)
            .map(|slot| match state.inventory.get(slot) {
                Some(ItemKind::HealthPotion) => "[♥ HP]",
                Some(ItemKind::ManaPotion) => "[◆ MP]",
                Some(ItemKind::SpeedPotion) => "[✦ SPD]",
                Some(ItemKind::AttackSpeedPotion) => "[⚡ ATK]",
                None => "[     ]",
            })
            .collect::<Vec<_>>()
            .join("  ");
        text.sections[0].value = format!("INVENTORY // 24 SLOTS\n\n{}\n\nH health   M mana   B speed (25 seconds)\nI / Esc close", slots);
    }
    if let Ok(mut style) = health_bar.get_single_mut() {
        style.width = Val::Percent((state.health.max(0) as f32 / state.max_health as f32) * 100.0);
    }
    if let Ok(mut text) = clear_text.get_single_mut() {
        text.sections[0].value = if state.room_notice > 0.0 {
            "ROOM CLEARED  //  CHOOSE AN UPGRADE".into()
        } else {
            "".into()
        };
    }
}

pub(crate) fn update_crosshair(
    state: Res<crate::GameState>,
    windows: Query<&Window>,
    mut crosshair: Query<&mut Style, With<Crosshair>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok(mut style) = crosshair.get_single_mut() else {
        return;
    };
    style.display = if state.mode == crate::GameMode::Playing {
        Display::Flex
    } else {
        Display::None
    };
    if let Some(position) = window.cursor_position() {
        style.left = Val::Px(position.x - 10.0);
        style.top = Val::Px(position.y - 10.0);
    }
}

pub(crate) fn update_enemy_bars(
    enemies: Query<(&Enemy, &Children)>,
    mut bars: Query<(&mut Sprite, &EnemyHealthBar)>,
) {
    for (enemy, children) in &enemies {
        for child in children.iter() {
            if let Ok((mut sprite, bar)) = bars.get_mut(*child) {
                let ratio =
                    (enemy.health.max(0) as f32 / enemy.max_health().max(1) as f32).clamp(0.0, 1.0);
                sprite.custom_size = Some(Vec2::new((enemy.radius * 2.0) * ratio, 5.0));
                sprite.color = if ratio < 0.35 {
                    Color::srgb(1.0, 0.15, 0.2)
                } else {
                    Color::srgb(0.2, 0.95, 0.45)
                };
                let _ = bar.max_health;
            }
        }
    }
}

pub(crate) fn floating_text_system(
    time: Res<Time>,
    state: Res<crate::GameState>,
    mut texts: Query<(&mut Transform, &FloatingText)>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    for (mut transform, floating) in &mut texts {
        transform.translation += floating.velocity.extend(0.0) * time.delta_seconds();
    }
}
