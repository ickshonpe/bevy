mod convert;

use crate::{CalculatedSize, Node, Style, UiScale};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    prelude::{Component, DetectChangesMut, Bundle},
    query::{Changed, Or, With, Without},
    removal_detection::RemovedComponents,
    system::{Commands, Query, Res, ResMut, Resource},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_log::warn;
use bevy_math::Vec2;
use bevy_transform::components::Transform;
use bevy_utils::HashMap;
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use std::fmt;
use taffy::{
    prelude::{AvailableSpace, Size},
    style_helpers::TaffyMaxContent,
    Taffy,
};

#[derive(Bundle)]
pub struct UiSurfaceBundle {
    // resolution and scale
    pub ui_context: UiContext,
    // the ui layout
    pub ui_surface: UiSurface,
}

#[derive(Default, Resource)]
pub struct UiNodeMap {
    // maps ui node entities to the entity of the UiSurface they inhabit
    pub entity_to_surface: HashMap<Entity, Entity>,

    // default ui surface
    pub primary_ui_surface: Option<Entity>,
}

#[derive(Component, Default, PartialEq)]
pub struct UiContext {
    pub ui_scale: f64,
    pub scale_factor: f64,
    pub logical_to_physical_factor: f64,
    pub physical_to_logical_factor: f64,
    pub physical_size: Vec2,
    pub min_size: f32,
    pub max_size: f32,
}

#[derive(Component)]
pub struct UiSurfaceIdx(pub Entity);

impl UiContext {
    /// create new a [`LayoutContext`] from the window's physical size and scale factor
    pub fn new(ui_scale: &UiScale, window_resolution: &WindowResolution) -> Self {
        let physical_size = Vec2::new(
            window_resolution.physical_width() as f32,
            window_resolution.physical_height() as f32,
        );
        Self {
            ui_scale: ui_scale.scale,
            scale_factor: ui_scale.scale * window_resolution.scale_factor(),
            logical_to_physical_factor: window_resolution.scale_factor(),
            physical_to_logical_factor: 1. / window_resolution.scale_factor(),
            physical_size,
            min_size: physical_size.x.min(physical_size.y),
            max_size: physical_size.x.max(physical_size.y),
        }
    }

    /// create an unscaled [`LayoutContext`]
    #[cfg(test)]
    pub(crate) fn unscaled(size: Vec2) -> Self {
        Self {
            ui_scale: 1.0,
            scale_factor: 1.0,
            logical_to_physical_factor: 1.0,
            physical_to_logical_factor: 1.0,
            physical_size: size,
            min_size: size.x.min(size.y),
            max_size: size.x.max(size.y),
        }
    }

    /// create a scaled [`LayoutContext`]
    #[cfg(test)]
    pub(crate) fn scaled(scale: f64, size: Vec2) -> Self {
        Self {
            ui_scale: 1.0,
            scale_factor: scale,
            logical_to_physical_factor: scale,
            physical_to_logical_factor: 1. / scale,
            physical_size: size,
            min_size: size.x.min(size.y),
            max_size: size.x.max(size.y),
        }
    }
}

// #[derive(Default, Resource)]
// pub struct FlexLayouts {
//     /// Window Entity -> FlexSurface
//     pub window_to_surface: HashMap<Entity, UiSurface>,
    
//     /// The ui surface that is used by default
//     pub primary_ui_surface: Option<Entity>,

//     /// Node Entity -> Window Entity
//     pub node_to_window: HashMap<Entity, Entity>,
// }

#[derive(Component)]
pub struct UiSurface {
    /// maps bevy UI node entities to taffy nodes
    pub entity_to_taffy: HashMap<Entity, taffy::node::Node>,
    /// taffy root node that all other nodes in the layout descend from
    /// has the same size as the window  
    pub surface_node: taffy::node::Node,
    /// taffy layout tree
    pub taffy: Taffy,
}

// SAFETY: as long as MeasureFunc is Send + Sync. https://github.com/DioxusLabs/taffy/issues/146
unsafe impl Send for UiSurface {}
unsafe impl Sync for UiSurface {}

fn _assert_send_sync_flex_surface_impl_safe() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<HashMap<Entity, taffy::node::Node>>();
    _assert_send_sync::<Taffy>();
}

impl fmt::Debug for UiSurface {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FlexSurface")
            .field("entity_to_taffy", &self.entity_to_taffy)
            .finish()
    }
}

impl Default for UiSurface {
    fn default() -> Self {
        let mut taffy = Taffy::new();
        // surface node needs to be set to the same size as the viewport before being used,
        // all other nodes will be children of this node in the taffy layout tree
        let surface_node = taffy.new_leaf(taffy::prelude::Style::default()).unwrap();
        Self {
            entity_to_taffy: Default::default(),
            surface_node,
            taffy: Taffy::new(),
        }
    }
}

impl UiSurface {
    fn upsert_node(&mut self, entity: Entity, style: &Style, context: &UiContext) {
        let mut added = false;
        let taffy = &mut self.taffy;
        let taffy_node = self.entity_to_taffy.entry(entity).or_insert_with(|| {
            added = true;
            taffy.new_leaf(convert::from_style(context, style)).unwrap()
        });

        if !added {
            self.taffy
                .set_style(*taffy_node, convert::from_style(context, style))
                .unwrap();
        }
    }

    fn update_measure(
        &mut self,
        entity: Entity,
        calculated_size: CalculatedSize,
        scale_factor: f64,
    ) {
        let measure = taffy::node::MeasureFunc::Boxed(Box::new(
            move |constraints: Size<Option<f32>>, _available: Size<AvailableSpace>| {
                let mut size = Size {
                    width: (scale_factor * calculated_size.size.x as f64) as f32,
                    height: (scale_factor * calculated_size.size.y as f64) as f32,
                };
                match (constraints.width, constraints.height) {
                    (None, None) => {}
                    (Some(width), None) => {
                        if calculated_size.preserve_aspect_ratio {
                            size.height = width * size.height / size.width;
                        }
                        size.width = width;
                    }
                    (None, Some(height)) => {
                        if calculated_size.preserve_aspect_ratio {
                            size.width = height * size.width / size.height;
                        }
                        size.height = height;
                    }
                    (Some(width), Some(height)) => {
                        size.width = width;
                        size.height = height;
                    }
                }
                size
            },
        ));
        let node = self.entity_to_taffy[&entity];
        self.taffy.set_measure(node, Some(measure));
    }

    fn upsert_leaf(
        &mut self,
        entity: Entity,
        style: &Style,
        calculated_size: CalculatedSize,
        context: &UiContext,
    ) {
        let taffy = &mut self.taffy;
        let taffy_style = convert::from_style(context, style);
        let scale_factor = context.scale_factor;
        let measure = taffy::node::MeasureFunc::Boxed(Box::new(
            move |constraints: Size<Option<f32>>, _available: Size<AvailableSpace>| {
                let mut size = Size {
                    width: (scale_factor * calculated_size.size.x as f64) as f32,
                    height: (scale_factor * calculated_size.size.y as f64) as f32,
                };
                match (constraints.width, constraints.height) {
                    (None, None) => {}
                    (Some(width), None) => {
                        if calculated_size.preserve_aspect_ratio {
                            size.height = width * size.height / size.width;
                        }
                        size.width = width;
                    }
                    (None, Some(height)) => {
                        if calculated_size.preserve_aspect_ratio {
                            size.width = height * size.width / size.height;
                        }
                        size.height = height;
                    }
                    (Some(width), Some(height)) => {
                        size.width = width;
                        size.height = height;
                    }
                }
                size
            },
        ));
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy.set_style(*taffy_node, taffy_style).unwrap();
            self.taffy.set_measure(*taffy_node, Some(measure)).unwrap();
        } else {
            let taffy_node = taffy.new_leaf_with_measure(taffy_style, measure).unwrap();
            self.entity_to_taffy.insert(entity, taffy_node);
        }
    }

    pub fn update_children(&mut self, entity: Entity, children: &Children) {
        let mut taffy_children = Vec::with_capacity(children.len());
        for child in children {
            if let Some(taffy_node) = self.entity_to_taffy.get(child) {
                taffy_children.push(*taffy_node);
            } else {
                warn!(
                    "Unstyled child in a UI entity hierarchy. You are using an entity \
without UI components as a child of an entity with UI components, results may be unexpected."
                );
            }
        }

        let taffy_node = self.entity_to_taffy.get(&entity).unwrap();
        self.taffy
            .set_children(*taffy_node, &taffy_children)
            .unwrap();
    }

    /// Removes children from the entity's taffy node if it exists. Does nothing otherwise.
    fn try_remove_children(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy.set_children(*taffy_node, &[]).unwrap();
        }
    }

    /// Removes each entity from the internal map and then removes their associated node from taffy
    fn remove_entities(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for entity in entities {
            if let Some(node) = self.entity_to_taffy.remove(&entity) {
                self.taffy.remove(node).unwrap();
            }
        }
    }

    fn get_layout(&self, entity: Entity) -> Result<&taffy::layout::Layout, FlexError> {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy
                .layout(*taffy_node)
                .map_err(FlexError::TaffyError)
        } else {
            warn!(
                "Styled child in a non-UI entity hierarchy. You are using an entity \
with UI components as a child of an entity without UI components, results may be unexpected."
            );
            Err(FlexError::InvalidHierarchy)
        }
    }
}

#[derive(Debug)]
pub enum FlexError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

pub fn setup_primary_window_ui(
    mut commands: Commands,
    mut ui_map: ResMut<UiNodeMap>,
    primary_window_query: Query<Entity, With<PrimaryWindow>>,
) {
    // if no primary ui surface exists, try to set it to the primary window
    if ui_map.primary_ui_surface.is_none() {
        if let Ok(primary_window_id) = primary_window_query.get_single() {
            ui_map.primary_ui_surface = Some(primary_window_id);
            commands
                .entity(primary_window_id)
                .insert(UiContext::default());
        }
    }
}

pub fn update_window_ui_contexts(
    ui_scale: Res<UiScale>,
    mut windows: Query<(&Window, &mut UiContext)>,
) {
    for (window, mut layout_context) in windows.iter_mut() {
        let new_layout_context = UiContext::new(&ui_scale, &window.resolution);
        layout_context.set_if_neq(new_layout_context);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flex_node_system(
    root_node_query: Query<(Entity, Option<&UiSurface>), (With<Node>, Without<Parent>)>,
    full_node_query: Query<(Entity, &Style, Option<&CalculatedSize>), With<Node>>,
    changed_style_query: Query<
        (Entity, &Style),
        (With<Node>, Without<CalculatedSize>, Changed<Style>),
    >,
    changed_size_query: Query<
        (Entity, &Style, &CalculatedSize),
        (With<Node>, Or<(Changed<CalculatedSize>, Changed<Style>)>),
    >,
    children_query: Query<(Entity, &Children), (With<Node>, Changed<Children>)>,
    mut removed_children: RemovedComponents<Children>,
    mut removed_calculated_sizes: RemovedComponents<CalculatedSize>,
    mut node_transform_query: Query<(Entity, &mut Node, &mut Transform, Option<&Parent>)>,
    mut removed_node_entities: RemovedComponents<Node>,
    layout_context_query: Query<(Entity, Ref<UiContext>)>,
    mut ui_node_map: ResMut<UiNodeMap>,
) {
    for (&viewport_entity, flex_surface) in ui_node_map.window_to_surface.iter_mut() {   
        // get layout context
        let layout_context = layout_context_query
            .get(viewport_entity)
            .expect("Viewport entity must have a layout context")
            .1;

        // Create a new root node if one doesn't exist
        let UiSurface {
            entity_to_taffy,
            taffy,
            ..
        } = flex_surface;
        let viewport_node = *entity_to_taffy
            .entry(viewport_entity)
            .or_insert_with(|| taffy.new_leaf(taffy::style::Style::default()).unwrap());

        // update size of viewport node
        flex_surface
            .taffy
            .set_style(
                viewport_node,
                taffy::style::Style {
                    size: taffy::geometry::Size {
                        width: taffy::style::Dimension::Points(
                            layout_context.physical_size.x as f32,
                        ),
                        height: taffy::style::Dimension::Points(
                            layout_context.physical_size.y as f32,
                        ),
                    },
                    ..Default::default()
                },
            )
            .unwrap();

        if layout_context.is_changed() {
            // Window resize or scale factor change, update all nodes
            // * This is only required because Taffy doesn't support viewport coords?
            for (entity, style, calculated_size) in &full_node_query {
                if let Some(calculated_size) = calculated_size {
                    flex_surface.upsert_leaf(entity, style, *calculated_size, &layout_context);
                } else {
                    flex_surface.upsert_node(entity, style, &layout_context);
                }
            }
        } else {
            // Update changed nodes without a calculated size
            for (entity, style) in changed_style_query.iter() {
                flex_surface.upsert_node(entity, style, &layout_context);
            }

            // Update changed nodes with a calculated size
            for (entity, style, calculated_size) in changed_size_query.iter() {
                flex_surface.upsert_leaf(entity, style, *calculated_size, &layout_context);
            }
        }
    }

    // clean up removed nodes
    for removed_entity in removed_node_entities.iter() {
        let window_entity = ui_node_map.node_to_window.get(&removed_entity).unwrap();
        let flex_surface = ui_node_map.window_to_surface.get_mut(window_entity).unwrap();
        if let Some(node) = flex_surface.entity_to_taffy.remove(&removed_entity) {
            flex_surface.taffy.remove(node).unwrap();
        }
    }

    // remove measures from nodes with removed calculated sizes
    for removed_entity in removed_calculated_sizes.iter() {
        let window_entity = ui_node_map.node_to_window.get(&removed_entity).unwrap();
        let flex_surface = ui_node_map.window_to_surface.get_mut(window_entity).unwrap();
        flex_surface.taffy.remove_measure(removed_entity).unwrap();
    }

    // remove children
    for entity in removed_children.iter() {
        let window_entity = ui_node_map.node_to_window.get(&entity).unwrap();
        let flex_surface = ui_node_map.window_to_surface.get_mut(window_entity).unwrap();
        flex_surface.try_remove_children(entity);
    }

    // update modified children
    for (entity, children) in &children_query {
        let window_entity = ui_node_map.node_to_window.get(&entity).unwrap();
        let flex_surface = ui_node_map.window_to_surface.get_mut(window_entity).unwrap();
        flex_surface.update_children(entity, children);
    }

    // update root nodes
    for root_node in root_node_query.iter() {
        

    }

    for (layout_entity, layout_context) in layout_context_query.iter() {
        let layout_node = ui_node_map.layout_nodes[&layout_entity];
        let root_nodes = root_node_query
            .iter()
            .map(|root_entity| ui_node_map.entity_to_taffy[&root_entity])
            .collect::<Vec<taffy::node::Node>>();
        ui_node_map
            .taffy
            .set_children(layout_node, &root_nodes)
            .unwrap();

        ui_node_map
            .taffy
            .compute_layout(layout_node, Size::MAX_CONTENT)
            .unwrap();

        let to_logical = |v| (layout_context.physical_to_logical_factor * v as f64) as f32;

        // PERF: try doing this incrementally
        for (entity, mut node, mut transform, parent) in &mut node_transform_query {
            let layout = ui_node_map.get_layout(entity).unwrap();
            let new_size = Vec2::new(
                to_logical(layout.size.width),
                to_logical(layout.size.height),
            );
            // only trigger change detection when the new value is different
            if node.calculated_size != new_size {
                node.calculated_size = new_size;
            }
            let mut new_position = transform.translation;
            new_position.x = to_logical(layout.location.x + layout.size.width / 2.0);
            new_position.y = to_logical(layout.location.y + layout.size.height / 2.0);
            if let Some(parent) = parent {
                if let Ok(parent_layout) = ui_node_map.get_layout(**parent) {
                    new_position.x -= to_logical(parent_layout.size.width / 2.0);
                    new_position.y -= to_logical(parent_layout.size.height / 2.0);
                }
            }
            // only trigger change detection when the new value is different
            if transform.translation != new_position {
                transform.translation = new_position;
            }
        }
    }
}


fn update_ui_surfaces(
    mut ui_surface_query: Query<(&mut UiSurface, Ref<UiContext>)>,
    full_style_query: Query<&Style, With<Node>>,
    full_measure_query: Query<&Style, With<CalculatedSize>>,
    changed_node_query: Query<Ref<Style>, With<Node>>,
    changed_measure_query: Query<Ref<CalculatedSize>, With<Node>>,
) {
    for (mut surface, layout_context) in ui_surface_query.iter_mut() {
        /// update the surface node if context has changed
        if layout_context.is_changed() {
            surface
            .taffy
            .set_style(
                surface.surface_node,
                taffy::style::Style {
                    size: taffy::geometry::Size {
                        width: taffy::style::Dimension::Points(
                            layout_context.physical_size.x as f32,
                        ),
                        height: taffy::style::Dimension::Points(
                            layout_context.physical_size.y as f32,
                        ),
                    },
                    ..Default::default()
                },
            )
            .unwrap();

             // Window resize or scale factor change, update all nodes
            // * This is only required because Taffy doesn't support viewport coords?
            for &entity in surface.entity_to_taffy.keys() {
                let (style, maybe_calculated_size) = full_node_query.get(entity).unwrap();
                if let Some(calculated_size) = maybe_calculated_size {
                    surface.upsert_leaf(entity, style, *calculated_size, &layout_context);
                } else {
                    surface.upsert_node(entity, style, &layout_context);
                }
            }
        } else {
            for &entity in surface.entity_to_taffy.keys() {
                if let Ok((style, maybe_calculated_size)) = changed_node_query.get(entity) {
                    if let Some(calculated_size) = maybe_calculated_size {
                        if style.is_changed() || calculated_size.is_changed() {
                            surface.upsert_leaf(entity, &style, *calculated_size, &layout_context);
                        }
                    } else {
                        if style.is_changed() {
                            surface.upsert_node(entity, &style, &layout_context);
                        }
                    }
                }
            }
        }
    }
}