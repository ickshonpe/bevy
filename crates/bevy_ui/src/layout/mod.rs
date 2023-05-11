mod convert;
pub mod debug;

use crate::{ContentSize, Node, NodeOrder, Style, UiScale, UiTransform, ZIndex};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    prelude::{Bundle, Component},
    query::{With, Without},
    reflect::ReflectComponent,
    removal_detection::RemovedComponents,
    system::{Local, Query, Res, ResMut, Resource},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_log::warn;
use bevy_math::{Affine3A, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::view::{ComputedVisibility, Visibility};
use bevy_transform::{components::Transform, prelude::GlobalTransform};
use bevy_utils::{HashMap, HashSet};
use bevy_window::{PrimaryWindow, Window, WindowScaleFactorChanged};
use std::fmt;
use taffy::{prelude::Size, style_helpers::TaffyMaxContent, Taffy};

/// Used internally by `ui_layout_system`
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
pub struct UiLayoutData {
    #[reflect(ignore)]
    node_key: taffy::node::Node,
}

#[derive(Component, Default, Copy, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct UiLayoutOrder(pub i32);

#[derive(Bundle, Default)]
pub struct UiLayoutBundle {
    pub order: UiLayoutOrder,
    pub data: UiLayoutData,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visbility: ComputedVisibility,
}

pub struct LayoutContext {
    pub scale_factor: f64,
    pub physical_size: Vec2,
    pub min_size: f32,
    pub max_size: f32,
}

impl LayoutContext {
    /// create new a [`LayoutContext`] from the window's physical size and scale factor
    fn new(scale_factor: f64, physical_size: Vec2) -> Self {
        Self {
            scale_factor,
            physical_size,
            min_size: physical_size.x.min(physical_size.y),
            max_size: physical_size.x.max(physical_size.y),
        }
    }
}

#[derive(Debug)]
pub struct UiLayout {
    /// entity representing the layout
    /// children of this node are added to the layout as children of the root taffy node
    pub layout_entity: Entity,
    /// root taffy node for the layout, same size as the window resolution
    pub taffy_root: taffy::node::Node,
    /// sort order, lower drawn first
    pub order: i32,
}

#[derive(Resource)]
pub struct UiSurface {
    /// Ui Node entity to taffy layout tree node lookup map
    entity_to_taffy: HashMap<Entity, taffy::node::Node>,
    /// default layout node, orphaned Ui entities attach to here
    default_layout: Option<taffy::node::Node>,
    /// contains data for each distinct ui layout
    ui_layouts: Vec<UiLayout>,
    /// contains data for each layout child
    layout_children: HashMap<taffy::node::Node, Vec<Entity>>,
    /// the taffy layout tree
    taffy: Taffy,
}

fn _assert_send_sync_ui_surface_impl_safe() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<HashMap<Entity, taffy::node::Node>>();
    _assert_send_sync::<Taffy>();
    _assert_send_sync::<UiSurface>();
}

impl fmt::Debug for UiSurface {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UiSurface")
            .field("entity_to_taffy", &self.entity_to_taffy)
            .field("ui_layouts", &self.ui_layouts)
            .finish()
    }
}

impl bevy_ecs::world::FromWorld for UiSurface {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let mut taffy = Taffy::new();
        let default_layout = taffy.new_leaf(taffy::prelude::Style::default()).unwrap();
        let default_layout_entity = world
            .spawn(UiLayoutBundle {
                order: UiLayoutOrder(0),
                data: UiLayoutData {
                    node_key: default_layout,
                },
                ..Default::default()
            })
            .id();
        Self {
            entity_to_taffy: Default::default(),
            default_layout: Some(default_layout),
            ui_layouts: vec![UiLayout {
                layout_entity: default_layout_entity,
                taffy_root: default_layout,
                order: 0,
            }],
            taffy,
            layout_children: [(default_layout, vec![])].into_iter().collect(),
        }
    }
}

impl UiSurface {
    pub fn no_ui_layouts(&self) -> bool {
        self.ui_layouts.is_empty()
    }

    pub fn insert_ui_layout(&mut self, layout_entity: Entity, order: i32) -> taffy::node::Node {
        let layout_node = self.taffy.new_leaf(taffy::style::Style::default()).unwrap();
        self.ui_layouts.push(UiLayout {
            layout_entity,
            taffy_root: layout_node,
            order,
        });
        layout_node
    }

    /// Retrieves the taffy node corresponding to given entity exists, or inserts a new taffy node into the layout if no corresponding node exists.
    /// Then convert the given `Style` and use it update the taffy node's style.
    pub fn upsert_node(&mut self, entity: Entity, style: &Style, context: &LayoutContext) {
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

    /// Update the `MeasureFunc` of the taffy node corresponding to the given [`Entity`].
    pub fn update_measure(&mut self, entity: Entity, measure_func: taffy::node::MeasureFunc) {
        let taffy_node = self.entity_to_taffy.get(&entity).unwrap();
        self.taffy.set_measure(*taffy_node, Some(measure_func)).ok();
    }

    /// Update the children of the taffy node corresponding to the given [`Entity`].
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
    pub fn try_remove_children(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy.set_children(*taffy_node, &[]).unwrap();
        }
    }

    /// Removes the measure from the entity's taffy node if it exists. Does nothing otherwise.
    pub fn try_remove_measure(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy.set_measure(*taffy_node, None).unwrap();
        }
    }

    /// Update the size of each layout node to match the size of the window.
    pub fn update_layout_nodes(&mut self, size: Vec2) {
        let taffy = &mut self.taffy;
        for UiLayout { taffy_root, .. } in &self.ui_layouts {
            taffy
                .set_style(
                    *taffy_root,
                    taffy::style::Style {
                        size: taffy::geometry::Size {
                            width: taffy::style::Dimension::Points(size.x),
                            height: taffy::style::Dimension::Points(size.y),
                        },
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }

    /// Set the ui node entities without a [`Parent`] as children to the default root node in the taffy layout.
    pub fn set_default_layout_children(&mut self, children: impl Iterator<Item = Entity>) {
        if let Some(node) = self.default_layout {
            let childs = self.layout_children.get_mut(&node).unwrap();
            *childs = children.collect();
            let child_nodes = childs
                .iter()
                .map(|e| *self.entity_to_taffy.get(e).unwrap())
                .collect::<Vec<taffy::node::Node>>();
            self.taffy.set_children(node, &child_nodes).unwrap();
        }
    }

    pub fn set_layout_children(&mut self, layout: taffy::node::Node, children: &Children) {
        self.layout_children
            .insert(layout, children.iter().copied().collect());
        let child_nodes = children
            .iter()
            .map(|e| *self.entity_to_taffy.get(e).unwrap())
            .collect::<Vec<taffy::node::Node>>();
        self.taffy.set_children(layout, &child_nodes).unwrap();
    }

    /// Compute the layout for each window entity's corresponding root node in the layout.
    pub fn compute_all_layouts(&mut self) {
        for UiLayout { taffy_root, .. } in &self.ui_layouts {
            self.taffy
                .compute_layout(*taffy_root, Size::MAX_CONTENT)
                .unwrap();
        }
    }

    /// Removes each entity from the internal map and then removes their associated node from taffy
    pub fn remove_entities(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for entity in entities {
            if let Some(node) = self.entity_to_taffy.remove(&entity) {
                self.taffy.remove(node).unwrap();
            }
        }
    }

    /// Removes layouts and set a new default if necessary
    pub fn remove_layouts(
        &mut self,
        removed_entities: impl IntoIterator<Item = Entity>,
        maybe_next_default_layout_entity: Option<Entity>,
    ) {
        for entity in removed_entities {
            if let Some(node_to_delete) = self.entity_to_taffy.get(&entity).copied() {
                self.taffy.remove(node_to_delete).unwrap();
                self.layout_children.remove(&node_to_delete);
                if self.default_layout == Some(node_to_delete) {
                    self.default_layout =
                        maybe_next_default_layout_entity.and_then(|next_default_layout_entity| {
                            self.ui_layouts
                                .iter()
                                .find(|UiLayout { layout_entity, .. }| {
                                    *layout_entity == next_default_layout_entity
                                })
                                .map(|UiLayout { taffy_root, .. }| *taffy_root)
                        });
                }
                self.ui_layouts
                    .retain(|UiLayout { layout_entity, .. }| *layout_entity != entity);
            }
        }
    }

    /// Get the layout geometry for the taffy node corresponding to the ui node [`Entity`].
    /// Does not compute the layout geometry, `compute_window_layouts` should be run before using this function.
    pub fn get_layout(&self, entity: Entity) -> Result<&taffy::layout::Layout, LayoutError> {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy
                .layout(*taffy_node)
                .map_err(LayoutError::TaffyError)
        } else {
            warn!(
                "Styled child in a non-UI entity hierarchy. You are using an entity \
with UI components as a child of an entity without UI components, results may be unexpected."
            );
            Err(LayoutError::InvalidHierarchy)
        }
    }
}

#[derive(Debug)]
pub enum LayoutError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

pub fn sort_children_by_node_order(
    mut sorted: Local<HashSet<Entity>>,
    order_query: Query<(Ref<NodeOrder>, &Parent)>,
    mut children_query: Query<&mut Children>,
) {
    sorted.clear();
    for (order, parent) in order_query.iter() {
        if order.is_changed() && !sorted.contains(&parent.get()) {
            children_query
                .get_mut(parent.get())
                .unwrap()
                .sort_by(|c, d| {
                    let c_ord = order_query.get_component(*c).unwrap_or(&NodeOrder(0)).0;
                    let d_ord = order_query.get_component(*d).unwrap_or(&NodeOrder(0)).0;
                    c_ord.cmp(&d_ord)
                });
        }
    }
}

/// Updates the UI's layout tree, computes the new layout geometry and then updates the sizes and transforms of all the UI nodes.
#[allow(clippy::too_many_arguments)]
pub fn ui_layout_system(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
    mut resize_events: EventReader<bevy_window::WindowResized>,
    mut ui_surface: ResMut<UiSurface>,
    orphaned_node_query: Query<Entity, (With<Node>, Without<Parent>)>,
    style_query: Query<(Entity, Ref<Style>), With<Node>>,
    mut measure_query: Query<(Entity, &mut ContentSize)>,
    mut removed_children: RemovedComponents<Children>,
    mut removed_content_sizes: RemovedComponents<ContentSize>,
    mut removed_nodes: RemovedComponents<Node>,
    mut removed_layouts: RemovedComponents<UiLayoutOrder>,
    children_query: Query<(Entity, Ref<Children>), With<Node>>,
    mut layouts_query: Query<
        (Entity, &mut UiLayoutData, &UiLayoutOrder, Option<&Children>),
        Without<Node>,
    >,
) {
    // assume one window for time being...
    // TODO: Support window-independent scaling: https://github.com/bevyengine/bevy/issues/5621
    let (primary_window_entity, logical_to_physical_factor, window_physical_size) =
        if let Ok((entity, primary_window)) = primary_window.get_single() {
            (
                entity,
                primary_window.resolution.scale_factor(),
                Vec2::new(
                    primary_window.resolution.physical_width() as f32,
                    primary_window.resolution.physical_height() as f32,
                ),
            )
        } else {
            return;
        };

    let resized = resize_events
        .iter()
        .any(|resized_window| resized_window.window == primary_window_entity);

    // add new layouts
    for (layout_entity, mut layout_data, &UiLayoutOrder(order), _) in layouts_query.iter_mut() {
        if layout_data.node_key == taffy::node::Node::default() {
            layout_data.node_key = ui_surface.insert_ui_layout(layout_entity, order);
        }
    }

    // update layout nodes so their size matches the size of the primary window
    ui_surface.update_layout_nodes(window_physical_size);

    // sort layouts by order
    ui_surface
        .ui_layouts
        .sort_by_key(|UiLayout { order, .. }| *order);

    let scale_factor = logical_to_physical_factor * ui_scale.scale;

    let layout_context = LayoutContext::new(scale_factor, window_physical_size);

    if !scale_factor_events.is_empty() || ui_scale.is_changed() || resized {
        scale_factor_events.clear();
        // update all nodes
        for (entity, style) in style_query.iter() {
            ui_surface.upsert_node(entity, &style, &layout_context);
        }
    } else {
        for (entity, style) in style_query.iter() {
            if style.is_changed() {
                ui_surface.upsert_node(entity, &style, &layout_context);
            }
        }
    }

    for (entity, mut content_size) in measure_query.iter_mut() {
        if let Some(measure_func) = content_size.measure_func.take() {
            ui_surface.update_measure(entity, measure_func);
        }
    }

    // clean up removed layout nodes
    ui_surface.remove_layouts(
        removed_layouts.iter(),
        layouts_query.iter().next().map(|(entity, ..)| entity),
    );

    // clean up removed nodes
    ui_surface.remove_entities(removed_nodes.iter());

    // When a `ContentSize` component is removed from an entity, we need to remove the measure from the corresponding taffy node.
    for entity in removed_content_sizes.iter() {
        ui_surface.try_remove_measure(entity);
    }

    // update root ui nodes

    // update orphaned nodes as children of the default layout (for now assuming all Nodes live in the primary window)
    ui_surface.set_default_layout_children(orphaned_node_query.iter());

    for (_, data, _, children) in layouts_query.iter() {
        if let Some(children) = children {
            ui_surface.set_layout_children(data.node_key, children);
        }
    }

    // update and remove children
    for entity in removed_children.iter() {
        ui_surface.try_remove_children(entity);
    }
    for (entity, children) in &children_query {
        if children.is_changed() {
            ui_surface.update_children(entity, &children);
        }
    }

    // compute layouts
    ui_surface.compute_all_layouts();
}

pub fn update_nodes(
    mut ui_surface: ResMut<UiSurface>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut node_geometry_query: Query<(&mut Node, &mut UiTransform, &mut ZIndex)>,
    just_children_query: Query<&Children>,
) {
    let scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.0);

    let physical_to_logical_factor = scale_factor.recip();

    fn update_node_geometry_recursively(
        ui_surface: &UiSurface,
        inherited_transform: Affine3A,
        node_entity: Entity,
        node_geometry_query: &mut Query<(&mut Node, &mut UiTransform, &mut ZIndex)>,
        children_query: &Query<&Children>,
        physical_to_logical_factor: f64,
        order: &mut u32,
    ) {
        if let Ok((mut node, mut transform, mut z_index)) = node_geometry_query.get_mut(node_entity)
        {
            z_index.0 = *order;
            *order += 1;
            let layout = ui_surface.get_layout(node_entity).unwrap();
            let new_size = Vec2::new(
                (layout.size.width as f64 * physical_to_logical_factor) as f32,
                (layout.size.height as f64 * physical_to_logical_factor) as f32,
            );
            let half_size = (0.5 * new_size).extend(0.);
            if node.calculated_size != new_size {
                node.calculated_size = new_size;
            }
            let new_position = Vec2::new(
                (layout.location.x as f64 * physical_to_logical_factor) as f32,
                (layout.location.y as f64 * physical_to_logical_factor) as f32,
            );

            transform.0 = inherited_transform
                * Affine3A::from_translation(new_position.extend(0.) + half_size);

            if let Ok(children) = children_query.get(node_entity) {
                let next_transform = transform.0 * Affine3A::from_translation(-half_size);
                for child in children {
                    update_node_geometry_recursively(
                        ui_surface,
                        next_transform,
                        *child,
                        node_geometry_query,
                        children_query,
                        physical_to_logical_factor,
                        order,
                    );
                }
            }
        }
    }

    let mut order: u32 = 0;

    let mut layouts = vec![];
    let mut layout_children = HashMap::default();
    std::mem::swap(&mut ui_surface.ui_layouts, &mut layouts);
    std::mem::swap(&mut ui_surface.layout_children, &mut layout_children);

    for UiLayout { taffy_root, .. } in layouts.iter() {
        for child in layout_children.get(taffy_root).unwrap() {
            update_node_geometry_recursively(
                &ui_surface,
                Affine3A::default(),
                *child,
                &mut node_geometry_query,
                &just_children_query,
                physical_to_logical_factor,
                &mut order,
            );
        }
    }

    std::mem::swap(&mut ui_surface.ui_layouts, &mut layouts);
    std::mem::swap(&mut ui_surface.layout_children, &mut layout_children);
}
