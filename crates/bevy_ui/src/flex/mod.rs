mod convert;

use crate::{CalculatedSize, Node, Style, UiScale};
use bevy_derive::{DerefMut, Deref};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    query::{Changed, With, Without, Or},
    removal_detection::RemovedComponents,
    system::{Commands, Query, Res, ResMut, Resource, ParamSet}, world::{Mut, World},
};
use bevy_hierarchy::{Children, Parent, BuildChildren};
use bevy_math::Vec2;
use bevy_render::{view::VisibilityBundle, prelude::SpatialBundle};
use bevy_transform::{components::Transform, prelude::GlobalTransform};
use bevy_window::{PrimaryWindow, Window, WindowScaleFactorChanged};
use taffy::{
    prelude::{AvailableSpace, Size, Layout, TaffyWorld},
    style_helpers::TaffyMaxContent, node::{NeedsMeasure, SizeCache, MeasureFunc},
};

#[derive(Resource, Debug, Default, PartialEq)]
pub struct UiView {
    pub scale_factor: f64,
    pub physical_to_logical_factor: f64,
    pub physical_size: Vec2,
    pub min_size: f32,
    pub max_size: f32,
}

impl UiView {
    /// create new a [`LayoutContext`] from the window's physical size and scale factor
    fn new(ui_scale: f64, logical_to_physical_factor: f64, physical_size: Vec2) -> Self {
        let scale_factor = logical_to_physical_factor * ui_scale;
        let physical_to_logical_factor = 1.0 / logical_to_physical_factor;
        Self {
            scale_factor,
            physical_size,
            physical_to_logical_factor,
            min_size: physical_size.x.min(physical_size.y),
            max_size: physical_size.x.max(physical_size.y),
        }
    }
}

#[derive(Resource, Debug)]
pub struct UiState {
    root_node: Entity,
    full_update: bool,
}

fn insert_node(
    commands: &mut Commands,
    entity: Entity,
    style: &Style,
    calculated_size: Option<&CalculatedSize>,
    context: &UiView,
) {
    let style = convert::from_style(context, style);

    if let Some(calculated_size) = calculated_size {
        let measure = make_measure(*calculated_size, context.scale_factor);
        commands.entity(entity).insert((
            style,
            measure,
            NeedsMeasure(true),
            SizeCache::default(),
            Layout::new()
        ));
    } else {
        commands.entity(entity).insert((
            style,
            NeedsMeasure(false),
            SizeCache::default(),
            Layout::new()
        ));
    }
}

fn update_node(
    commands: &mut Commands,
    entity: Entity,
    style: &Style,
    calculated_size: Option<&CalculatedSize>,
    context: &UiView, 
    needs_measure: &mut NeedsMeasure,
    taffy_style: &mut taffy::style::Style,
    measure_func: Option<Mut<MeasureFunc>>,
) {
    *taffy_style = convert::from_style(context, style);

    if let Some(calculated_size) = calculated_size {
        let measure = make_measure(*calculated_size, context.scale_factor);
        if let Some(mut measure_func) = measure_func {
            *measure_func = measure;
        } else {
            commands.entity(entity).insert(measure);
        }
        *needs_measure = NeedsMeasure(true);
    } else {
        commands.entity(entity).remove::<MeasureFunc>();
        *needs_measure = NeedsMeasure(false);
    }
}

#[derive(Debug)]
pub enum FlexError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct DirtyNodes(bevy_utils::HashSet<Entity>);

pub fn manage_ui_windows(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
    mut resize_events: EventReader<bevy_window::WindowResized>,
    maybe_ui_state: Option<ResMut<UiState>>,
    mut commands: Commands,
    mut ui_view: ResMut<UiView>,
    mut taffy_style_query: Query<&mut taffy::style::Style>,
    mut dirty: ResMut<DirtyNodes>,
) {
    println!("manage windows");
    let (primary_window_entity, logical_to_physical_factor, physical_size) =
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
        commands.remove_resource::<UiState>();
        return;
    };
    let ui_view_new = UiView::new(ui_scale.scale, logical_to_physical_factor, physical_size);
    if *ui_view != ui_view_new {
        *ui_view = ui_view_new;
    }

    let resized = resize_events
        .iter()
        .any(|resized_window| resized_window.window == primary_window_entity);

    let full_update = !scale_factor_events.is_empty() || ui_scale.is_changed() || resized;
    scale_factor_events.clear();

    if let Some(mut ui_state) = maybe_ui_state {
        if full_update {
            println!("full update");
            ui_state.full_update = true;
            taffy_style_query
                .get_mut(ui_state.root_node)
                .unwrap()
                .size = taffy::geometry::Size {
                    width: taffy::style::Dimension::Points(physical_size.x as f32),
                    height: taffy::style::Dimension::Points(physical_size.y as f32),
                };   
            dirty.insert(ui_state.root_node);
            println!("root node: {:?}", ui_state.root_node);
        }
    } else {
        
        let style = taffy::style::Style {
            size: taffy::geometry::Size {
                width: taffy::style::Dimension::Points(physical_size.x as f32),
                height: taffy::style::Dimension::Points(physical_size.y as f32),
            },
            ..Default::default()
        };
        let root_node = commands.spawn((
            style,
            NeedsMeasure(false),
            SizeCache::default(),
            Layout::new(),
            SpatialBundle::default(),
        )).id();

        commands.insert_resource(UiState {
            root_node,
            full_update: true,
        });

        println!("new ui state, full update");
        println!("root node: {:?}", root_node);
    }
    
}

#[allow(clippy::too_many_arguments)]
pub fn update_ui_nodes(
    orphan_node_query: Query<Entity, (With<Node>, Without<Parent>)>,
    mut node_queries: ParamSet<(
        Query<(Entity, &Style, &mut taffy::style::Style), With<Node>>,
        Query<(Entity, &Style, &mut taffy::style::Style), (With<Node>, Changed<Style>)>,
    )>,
    mut changed_size_query: Query<
        (Entity, &CalculatedSize, &mut NeedsMeasure, Option<&mut MeasureFunc>),
        (With<Node>, Changed<CalculatedSize>),
    >,
    mut removed_nodes: RemovedComponents<Node>,
    mut commands: Commands,
    new_node_query: Query<
        (Entity, &Style, Option<&CalculatedSize>),
        (With<Node>, Without<taffy::style::Style>),
    >,
    changed_relationships_query: Query<
        Entity, (With<Node>, Or<(Changed<Children>, Changed<Parent>)>),
    >,
    mut dirty: ResMut<DirtyNodes>,
    maybe_ui_state: Option<Res<UiState>>,
    ui_view: Res<UiView>,
) {
    println!("update nodes");
    let Some(ui_state) = maybe_ui_state else { return };

    for (entity, style, calculated_size) in new_node_query.iter() {
        insert_node(&mut commands, entity, style, calculated_size, &ui_view);
        dirty.insert(entity);
    }

    if ui_state.full_update {
        for (entity, style, mut taffy_style) in node_queries.p0().iter_mut() {
            dirty.insert(entity);
            *taffy_style = convert::from_style(&ui_view, style);
        }
    } else {
        for (entity, style, mut taffy_style) in node_queries.p1().iter_mut() {
            dirty.insert(entity);
            *taffy_style = convert::from_style(&ui_view, style);
        }
    }

    for (entity, calculated_size, mut needs_measure, maybe_measure_func) in changed_size_query.iter_mut() {
        dirty.insert(entity);
        needs_measure.0 = true;
        let measure = make_measure(*calculated_size, ui_view.scale_factor);
        if let Some(mut measure_func) = maybe_measure_func {
            *measure_func = measure;
        } else {
            commands.entity(entity).insert(measure);
        }
    }

    // clean up removed nodes
    for entity in removed_nodes.iter() {
        commands.entity(entity)
            .remove::<(
                taffy::style::Style,
                Layout,
                NeedsMeasure,
                SizeCache,
                MeasureFunc,
            )>()
            .remove_parent()
            .clear_children();
    }

    //  dirty any nodes with changed relationships
    for entity in changed_relationships_query.iter() {
        dirty.insert(entity);
    }

    // dirty ophans
    for entity in orphan_node_query.iter() {
        dirty.insert(entity);
    }

    // set orphaned nodes as children of the root node
    commands.entity(ui_state.root_node).push_children(&orphan_node_query.iter().collect::<Vec<_>>());

}


pub fn compute_ui_layouts(
    world: &mut World,
) {
    println!("compute layouts");
    if let Some(ui_state) = world.get_resource::<UiState>() {
       let root_node = ui_state.root_node;
        world.resource_scope(|world, mut dirty: Mut<DirtyNodes>| {
            for dirty in dirty.drain() {
                world.mark_dirty_internal(dirty);
            }
        });
        world.compute_layout(root_node, Size::MAX_CONTENT).unwrap();
    }
}

pub fn update_ui_node_transforms(
    ui_state: Option<Res<UiState>>,
    ui_view: Res<UiView>,
    mut node_transform_query: Query<(&Layout, &mut Node, &mut Transform, &Parent)>,
    layout_query: Query<&Layout>,
) {
    println!("update transforms");
    let Some(root_node) = ui_state.map(|ui_state| ui_state.root_node) else {
        return;
    };

    let to_logical = |v| (ui_view.physical_to_logical_factor * v as f64) as f32;

    // PERF: try doing this incrementally
    for (layout, mut node, mut transform, parent) in &mut node_transform_query {
        println!("layout: {:?}", layout);
        // let layout = flex_surface.taffy.layout(taffy_node.key).unwrap();
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
        let parent_entity = parent.get();
        if parent_entity != root_node {
            if let Ok(parent_layout) = layout_query.get(parent_entity) {
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

pub fn make_measure(
    calculated_size: CalculatedSize,
    scale_factor: f64,
) -> taffy::node::MeasureFunc {
    taffy::node::MeasureFunc::Boxed(Box::new(
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
    ))
}
