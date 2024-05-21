// use bevy::{ecs::system::Command, prelude::*};
// use bevy_inspector_egui::quick::WorldInspectorPlugin;

// fn main() {
//     App::new()
//         .add_plugins((DefaultPlugins, MyPlugin))
//         .add_plugins(WorldInspectorPlugin::new())
//         .run();
// }

// #[derive(Component)]
// struct Person;

// #[derive(Component)]
// struct Name(String);

// #[derive(Component)]
// struct Card {
//     suit: String,
//     value: i32,
// }

// #[derive(Resource)]
// struct MyTimer(Timer);

// fn print_card_system(time: Res<Time>, mut timer: ResMut<MyTimer>, query: Query<&Card>) {
//     if timer.0.tick(time.delta()).just_finished() {
//         for card in &query {
//             println!("card: {} {}", card.suit, card.value);
//         }
//     }
// }

// pub struct MyPlugin;

// impl Plugin for MyPlugin {
//     fn build(&self, app: &mut App) {
//         app.insert_resource(MyTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
//             .add_systems(Startup, add_cards)
//             .add_systems(Update, print_card_system);
//     }
// }

// fn add_cards(mut commands: Commands) {
//     commands.spawn((Card {
//         suit: "heart".into(),
//         value: 1,
//     },));
//     commands.spawn((Person, Name("John K".to_string())));
// }
