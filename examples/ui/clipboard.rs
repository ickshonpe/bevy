//! This example demonstrates accessing the clipboard to retrieve and display text.

use bevy::{
    clipboard::{Clipboard, ClipboardError, ClipboardRead},
    color::palettes::css::{GREY, NAVY, RED},
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    ui::widget::NodeImageMode,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FrameTimeDiagnosticsPlugin::default()))
        .add_systems(Startup, (setup, load_clipboard_image).chain())
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

/// Marker component for the image display target.
#[derive(Component)]
pub struct ImageTarget;

/// Marker component for the image status line.
#[derive(Component)]
pub struct ImageStatus;

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
                width: px(560.),
                flex_direction: FlexDirection::Column,
                padding: px(30.).all(),
                row_gap: px(20.),
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
                    Node {
                        width: px(500.),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8.),
                        align_items: AlignItems::Center,
                        padding: px(10.).all(),
                        border: px(2.).all(),
                        ..Default::default()
                    },
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                    children![
                        (
                            Node {
                                width: px(240.),
                                height: px(240.),
                                border: px(1.).all(),
                                ..Default::default()
                            },
                            BorderColor::all(GREY),
                            ImageNode::default().with_mode(NodeImageMode::Stretch),
                            ImageTarget,
                        ),
                        (
                            Text::new("Checking clipboard for image..."),
                            TextColor(GREY.into()),
                            ImageStatus,
                        ),
                    ],
                ),
                (
                    Node {
                        width: px(500.),
                        min_height: px(90.),
                        padding: px(8.).all(),
                        border: px(2.).all(),
                        ..Default::default()
                    },
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                    children![(
                        Text::new("Nothing pasted yet."),
                        TextColor(GREY.into()),
                        PasteTarget
                    )],
                ),
                (
                    Node {
                        border: px(2.).all(),
                        padding: px(10.).all(),
                        align_self: AlignSelf::Center,
                        ..Default::default()
                    },
                    Button,
                    ButtonAction::PasteText,
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                    children![Text::new("Click to paste text")],
                ),
                (
                    Node {
                        border: px(2.).all(),
                        padding: px(10.).all(),
                        align_self: AlignSelf::Center,
                        ..Default::default()
                    },
                    Button,
                    ButtonAction::SetText,
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::BLACK),
                    children![Text::new("Click to copy 'Hello bevy!'\nto the clipboard")],
                ),
            ]
        ),],
    ));
}

fn load_clipboard_image(
    mut clipboard: ResMut<Clipboard>,
    mut images: ResMut<Assets<Image>>,
    mut image_node: Single<&mut ImageNode, With<ImageTarget>>,
    mut status_node: Single<(&mut Text, &mut TextColor), With<ImageStatus>>,
) {
    let mut read = clipboard.fetch_image();

    let (message, color) = match read.poll_result() {
        Some(Ok(clipboard_image)) => {
            let width = clipboard_image.width();
            let height = clipboard_image.height();
            image_node.image = images.add(clipboard_image);

            (
                format!("Loaded clipboard image ({width} x {height})"),
                Color::WHITE,
            )
        }
        Some(Err(ClipboardError::ContentNotAvailable)) => {
            ("No image found on the clipboard.".to_owned(), GREY.into())
        }
        Some(Err(error)) => (format!("Clipboard image error: {error:?}"), RED.into()),
        None => unreachable!(),
    };

    status_node.0 .0 = message.clone();
    status_node.1 .0 = color;
}

fn paste_text_system(
    mut paste: Local<Option<ClipboardRead>>,
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
    mut text_query: Query<(&mut Text, &mut TextColor), With<PasteTarget>>,
) {
    if let Some(contents) = paste.as_mut()
        && let Some(contents) = contents.poll_result()
    {
        let (message, color) = match contents {
            Ok(text) => (text, Color::WHITE),
            Err(error) => (format!("{error:?}"), RED.into()),
        };
        for (mut text, mut text_color) in text_query.iter_mut() {
            text.0 = message.clone();
            text_color.0 = color;
        }
        *paste = None;
    }

    for (interaction, mut color, mut border_color, button_action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                match button_action {
                    ButtonAction::PasteText => {
                        *paste = Some(clipboard.fetch_text());
                    }
                    ButtonAction::SetText => {
                        clipboard.set_text("Hello bevy!").ok();
                    }
                };
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
                border_color.set_all(Color::WHITE);
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
                border_color.set_all(GREY);
            }
        }
    }
}
