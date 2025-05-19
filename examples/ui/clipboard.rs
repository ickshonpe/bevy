//! This example demonstrates accessing the clipboard to retrieve and display text.

use bevy::{
    clipboard::{Clipboard, ClipboardRead},
    color::palettes::css::{GREY, NAVY, RED},
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FrameTimeDiagnosticsPlugin::default()))
        .add_systems(Startup, setup)
        .add_systems(Update, paste_text_system)
        .run();
}

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);

/// Button discriminator
#[derive(Component)]
pub enum ButtonAction {
    /// The button pastes some text from the clipboard
    PasteText,
    /// The button sends some text to the clipboard
    SetText,
}

/// Marker component for text box paste target
#[derive(Component)]
pub struct PasteTarget;

/// Marker component for image box paste target
#[derive(Component)]
pub struct ImagePasteTarget;

fn setup(mut commands: Commands) {
    // UI camera
    commands.spawn(Camera2d);

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(30.)),
                row_gap: Val::Px(20.),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(NAVY.into()),
            children![
                (
                    Node {
                        align_self: AlignSelf::Center,
                        ..Default::default()
                    },
                    Text::new("Bevy clipboard example"),
                ),
                (
                    Node::default(),
                    children![
                        (
                            Node {
                                width: Val::Px(500.),
                                height: Val::Px(250.),
                                padding: UiRect::all(Val::Px(3.)),
                                border: UiRect::all(Val::Px(2.)),
                                ..Default::default()
                            },
                            BorderColor(Color::WHITE),
                            BackgroundColor(Color::BLACK),
                            children![(
                                Text::new("Nothing pasted yet."),
                                TextColor(GREY.into()),
                                PasteTarget
                            )],
                        ),
                        (
                            Node {
                                width: Val::Px(250.),
                                height: Val::Px(250.),
                                padding: UiRect::all(Val::Px(3.)),
                                border: UiRect::all(Val::Px(2.)),
                                ..Default::default()
                            },
                            BorderColor(Color::WHITE),
                            BackgroundColor(Color::BLACK),
                            children![(ImageNode::default(), ImagePasteTarget)],
                        ),
                    ],
                ),
                (
                    Node {
                        border: UiRect::all(Val::Px(2.)),
                        padding: UiRect::all(Val::Px(10.)),
                        align_self: AlignSelf::Center,
                        ..Default::default()
                    },
                    Button,
                    ButtonAction::PasteText,
                    BorderColor(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                    children![Text::new("Click to paste")],
                ),
                (
                    Node {
                        border: UiRect::all(Val::Px(2.)),
                        padding: UiRect::all(Val::Px(10.)),
                        align_self: AlignSelf::Center,
                        ..Default::default()
                    },
                    Button,
                    ButtonAction::SetText,
                    BorderColor(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                    children![Text::new("Click to copy 'Hello bevy!'\nto the clipboard")],
                ),
            ]
        ),],
    ));
}

fn paste_text_system(
    mut text_paste: Local<Option<ClipboardRead<String>>>,
    mut image_paste: Local<Option<ClipboardRead<Image>>>,
    mut clipboard: ResMut<Clipboard>,
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &ButtonAction,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    asset_server: Res<AssetServer>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<PasteTarget>>,
    mut image_query: Query<&mut ImageNode, With<ImagePasteTarget>>,
) {
    if let Some(contents) = text_paste.as_mut() {
        if let Some(contents) = contents.poll_result() {
            let (message, color) = match contents {
                Ok(text) => (text, Color::WHITE),
                Err(error) => (format!("{error:?}"), RED.into()),
            };
            for (mut text, mut text_color) in text_query.iter_mut() {
                text.0 = message.clone();
                text_color.0 = color;
            }
            *text_paste = None;
        }
    }
    if let Some(contents) = image_paste.as_mut() {
        if let Some(contents) = contents.poll_result() {
            info!("recieved image");
            match contents {
                Ok(image) => {
                    if let Ok(mut image_node) = image_query.single_mut() {
                        image_node.image = asset_server.add(image);
                    }
                }
                Err(error) => {
                    info!("{error:?}");
                }
            }
            *image_paste = None;
        }
    }
    for (interaction, mut color, mut border_color, button_action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                match button_action {
                    ButtonAction::PasteText => {
                        *text_paste = Some(clipboard.fetch_text());
                        *image_paste = Some(clipboard.fetch_image());
                    }
                    ButtonAction::SetText => {
                        clipboard.set_text("Hello bevy!").ok();
                    }
                };
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
                border_color.0 = Color::WHITE;
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
                border_color.0 = GREY.into();
            }
        }
    }
}
