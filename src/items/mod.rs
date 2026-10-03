//! Pickups and their small, composable item definitions.

use bevy::prelude::*;

pub mod attackspeedpotion;
pub mod healthpotion;
pub mod manapotion;
pub mod potion;
pub mod speedpotion;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemKind {
    HealthPotion,
    ManaPotion,
    SpeedPotion,
    AttackSpeedPotion,
}

#[derive(Component)]
pub(crate) struct Pickup {
    pub(crate) item: ItemKind,
}

pub(crate) fn item_color(item: ItemKind) -> Color {
    match item {
        ItemKind::HealthPotion => healthpotion::COLOR,
        ItemKind::ManaPotion => manapotion::COLOR,
        ItemKind::SpeedPotion => speedpotion::COLOR,
        ItemKind::AttackSpeedPotion => attackspeedpotion::COLOR,
    }
}

pub(crate) fn pickup_message(item: ItemKind) -> &'static str {
    match item {
        ItemKind::HealthPotion => healthpotion::MESSAGE,
        ItemKind::ManaPotion => manapotion::MESSAGE,
        ItemKind::SpeedPotion => speedpotion::MESSAGE,
        ItemKind::AttackSpeedPotion => attackspeedpotion::MESSAGE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_has_distinct_feedback_text() {
        let messages = [
            pickup_message(ItemKind::HealthPotion),
            pickup_message(ItemKind::ManaPotion),
            pickup_message(ItemKind::SpeedPotion),
            pickup_message(ItemKind::AttackSpeedPotion),
        ];
        for (index, message) in messages.iter().enumerate() {
            assert!(!message.is_empty());
            assert!(!messages[..index].contains(message));
        }
    }

    #[test]
    fn item_colors_are_not_all_the_same() {
        let colors = [
            item_color(ItemKind::HealthPotion),
            item_color(ItemKind::ManaPotion),
            item_color(ItemKind::SpeedPotion),
            item_color(ItemKind::AttackSpeedPotion),
        ];
        assert!(colors.windows(2).any(|pair| pair[0] != pair[1]));
    }
}
