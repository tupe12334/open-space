use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;

const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_STAGE_DISTANCE: f32 = 6.0;
const DISTANCE_STEP: f32 = 0.5;
const MIN_DISTANCE: f32 = 1.0;
const MAX_DISTANCE: f32 = 30.0;

#[derive(Resource, Clone)]
pub struct AppSettings {
    pub stage_distance: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            stage_distance: DEFAULT_STAGE_DISTANCE,
        }
    }
}

#[derive(Component)]
struct SettingsUiRoot;

#[derive(Component)]
struct DistanceLabel;

#[derive(Resource, Default)]
struct SettingsUiOpen(bool);

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = load_settings();
        app.insert_resource(settings)
            .init_resource::<SettingsUiOpen>()
            .add_systems(Startup, spawn_settings_ui)
            .add_systems(Update, (toggle_settings_ui, handle_settings_input, update_distance_label));
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(SETTINGS_FILE)
}

fn load_settings() -> AppSettings {
    let path = settings_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(d) = val.get("stage_distance").and_then(|v| v.as_f64()) {
                return AppSettings {
                    stage_distance: d as f32,
                };
            }
        }
    }
    AppSettings::default()
}

fn save_settings(settings: &AppSettings) {
    let val = serde_json::json!({
        "stage_distance": settings.stage_distance,
    });
    if let Ok(data) = serde_json::to_string_pretty(&val) {
        let _ = fs::write(settings_path(), data);
    }
}

fn spawn_settings_ui(mut commands: Commands, settings: Res<AppSettings>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                right: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.85)),
            Visibility::Hidden,
            SettingsUiRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Settings"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new(format!("Distance to stage: {:.1}", settings.stage_distance)),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.9, 0.9, 1.0)),
                DistanceLabel,
            ));

            parent.spawn((
                Text::new("[Up/Down] adjust | [Tab] close"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgba(0.6, 0.6, 0.6, 1.0)),
            ));
        });
}

fn toggle_settings_ui(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui_open: ResMut<SettingsUiOpen>,
    mut query: Query<&mut Visibility, With<SettingsUiRoot>>,
) {
    if keyboard.just_pressed(KeyCode::Tab) {
        ui_open.0 = !ui_open.0;
        for mut vis in &mut query {
            *vis = if ui_open.0 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn handle_settings_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    ui_open: Res<SettingsUiOpen>,
    mut settings: ResMut<AppSettings>,
    mut screen_transforms: Query<&mut Transform, With<crate::stage::ScreenMarker>>,
) {
    if !ui_open.0 {
        return;
    }

    let mut changed = false;

    if keyboard.just_pressed(KeyCode::ArrowUp) {
        settings.stage_distance = (settings.stage_distance + DISTANCE_STEP).min(MAX_DISTANCE);
        changed = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        settings.stage_distance = (settings.stage_distance - DISTANCE_STEP).max(MIN_DISTANCE);
        changed = true;
    }

    if changed {
        for mut transform in &mut screen_transforms {
            transform.translation.z = -settings.stage_distance;
        }
        save_settings(&settings);
    }
}

fn update_distance_label(
    settings: Res<AppSettings>,
    mut query: Query<&mut Text, With<DistanceLabel>>,
) {
    if settings.is_changed() {
        for mut text in &mut query {
            **text = format!("Distance to stage: {:.1}", settings.stage_distance);
        }
    }
}
