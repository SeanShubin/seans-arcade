//! Shape description interpreter.
//!
//! Loads a shape description from RON data and produces a tree of Bevy entities
//! with meshes and materials. Supports templates, mirror combinator, and
//! hierarchical part composition.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

// =====================================================================
// Data format (deserialized from RON)
// =====================================================================

#[derive(Deserialize, Clone, Debug)]
pub struct ShapeFile {
    #[serde(default)]
    pub templates: HashMap<String, ShapeNode>,
    pub root: ShapeNode,
    #[serde(default)]
    pub animations: Vec<AnimState>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ShapeNode {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub shape: Option<PrimitiveShape>,
    #[serde(default)]
    pub at: (f32, f32, f32),
    #[serde(default)]
    pub pivot: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub color: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub emissive: bool,
    #[serde(default)]
    pub orient: Option<Axis>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub children: Vec<ShapeNode>,
    #[serde(default)]
    pub mirror: Option<Axis>,
    #[serde(default)]
    pub repeat: Option<RepeatSpec>,
}

#[derive(Deserialize, Clone, Debug)]
pub enum PrimitiveShape {
    Box { size: (f32, f32, f32) },
    Sphere { radius: f32 },
    Cylinder { radius: f32, height: f32 },
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub enum Axis { X, Y, Z }

#[derive(Deserialize, Clone, Debug)]
pub struct RepeatSpec {
    pub count: u32,
    pub spacing: f32,
    pub along: Axis,
    #[serde(default)]
    pub center: bool,
}

// =====================================================================
// Animation data
// =====================================================================

#[derive(Deserialize, Clone, Debug)]
pub enum JointMotion {
    /// Oscillates: sin(phase * speed + offset) * amplitude
    Oscillate { amplitude: f32, speed: f32, #[serde(default)] offset: f32 },
    /// Continuous spin: phase * rate
    Spin { rate: f32 },
    /// Constant bob: sin(time * freq) * amplitude (always active, ignores walk phase)
    Bob { amplitude: f32, freq: f32 },
}

/// An animation channel: which part, what property, what motion.
#[derive(Deserialize, Clone, Debug)]
pub struct AnimChannel {
    /// Name of the part to animate (matches ShapeNode.name).
    pub part: String,
    /// Which property to animate.
    pub property: AnimProperty,
    /// The motion curve.
    pub motion: JointMotion,
    /// Which axis to apply the motion on.
    pub axis: Axis,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub enum AnimProperty {
    /// Rotate around the axis (radians).
    Rotation,
    /// Translate along the axis.
    Translation,
}

/// A named animation state (e.g., "walk", "idle", "attack").
#[derive(Deserialize, Clone, Debug)]
pub struct AnimState {
    pub name: String,
    pub channels: Vec<AnimChannel>,
}

// =====================================================================
// Components
// =====================================================================

#[derive(Component, Clone, Debug)]
pub struct ShapePart {
    pub name: Option<String>,
}

/// Stores the original transform of a part, so animation can reset to it.
#[derive(Component, Clone, Debug)]
pub struct BaseTransform(pub Transform);

#[derive(Component)]
pub struct ShapeRoot;

/// Runtime animation state, attached to the root entity.
#[derive(Component, Clone, Debug)]
pub struct ShapeAnimator {
    pub states: Vec<AnimState>,
    pub active_state: Option<usize>,
    pub phase: f32,
    pub speed: f32,
    pub needs_reset: bool,
}

impl ShapeAnimator {
    pub fn new(states: Vec<AnimState>) -> Self {
        let active = if states.is_empty() { None } else { Some(0) };
        Self { states, active_state: active, phase: 0.0, speed: 1.0, needs_reset: false }
    }

    pub fn active_name(&self) -> &str {
        self.active_state
            .and_then(|i| self.states.get(i))
            .map(|s| s.name.as_str())
            .unwrap_or("(none)")
    }

    pub fn cycle_state(&mut self) {
        if self.states.is_empty() { return; }
        self.active_state = Some(match self.active_state {
            Some(i) => (i + 1) % self.states.len(),
            None => 0,
        });
        self.phase = 0.0;
        self.needs_reset = true;
    }
}

/// System that advances animation phase and applies animation channels to parts.
pub fn animate_shapes(
    time: Res<Time>,
    mut animators: Query<(&mut ShapeAnimator, &Children), With<ShapeRoot>>,
    parts: Query<(&ShapePart, Option<&Children>)>,
    base_transforms: Query<&BaseTransform>,
    mut transforms: Query<&mut Transform>,
) {
    for (mut animator, root_children) in &mut animators {
        animator.phase += time.delta_secs() * animator.speed;
        let phase = animator.phase;
        let t = time.elapsed_secs();

        // Build a map of part name → entity by walking the tree
        let mut name_map: HashMap<String, Vec<Entity>> = HashMap::new();
        collect_named_parts(root_children, &parts, &mut name_map);

        // Reset all parts to base transforms before applying new animation
        if animator.needs_reset {
            animator.needs_reset = false;
            for entities in name_map.values() {
                for &entity in entities {
                    if let Ok(base) = base_transforms.get(entity) {
                        if let Ok(mut tf) = transforms.get_mut(entity) {
                            *tf = base.0;
                        }
                    }
                }
            }
        }

        let Some(state_idx) = animator.active_state else { continue };
        let Some(state) = animator.states.get(state_idx) else { continue };

        for channel in &state.channels {
            let Some(entities) = name_map.get(&channel.part) else { continue };

            let value = evaluate_motion(&channel.motion, phase, t);

            for &entity in entities {
                let base = base_transforms.get(entity).map(|b| b.0).unwrap_or_default();
                let Ok(mut tf) = transforms.get_mut(entity) else { continue };
                match channel.property {
                    AnimProperty::Rotation => {
                        let rot = match channel.axis {
                            Axis::X => Quat::from_rotation_x(value),
                            Axis::Y => Quat::from_rotation_y(value),
                            Axis::Z => Quat::from_rotation_z(value),
                        };
                        tf.rotation = base.rotation * rot;
                    }
                    AnimProperty::Translation => {
                        // Start from base, add animation offset
                        match channel.axis {
                            Axis::X => tf.translation.x = base.translation.x + value,
                            Axis::Y => tf.translation.y = base.translation.y + value,
                            Axis::Z => tf.translation.z = base.translation.z + value,
                        }
                    }
                }
            }
        }
    }
}

fn collect_named_parts(
    children: &Children,
    parts: &Query<(&ShapePart, Option<&Children>)>,
    map: &mut HashMap<String, Vec<Entity>>,
) {
    for child in children.iter() {
        if let Ok((part, grandchildren)) = parts.get(child) {
            if let Some(ref name) = part.name {
                map.entry(name.clone()).or_default().push(child);
            }
            if let Some(gc) = grandchildren {
                collect_named_parts(gc, parts, map);
            }
        }
    }
}

fn evaluate_motion(motion: &JointMotion, phase: f32, time: f32) -> f32 {
    match motion {
        JointMotion::Oscillate { amplitude, speed, offset } => {
            (phase * speed + offset).sin() * amplitude
        }
        JointMotion::Spin { rate } => {
            phase * rate
        }
        JointMotion::Bob { amplitude, freq } => {
            (time * freq).sin() * amplitude
        }
    }
}

// =====================================================================
// Interpreter
// =====================================================================

fn to_vec3(t: (f32, f32, f32)) -> Vec3 {
    Vec3::new(t.0, t.1, t.2)
}

pub fn load_shape(ron_str: &str) -> Result<ShapeFile, String> {
    ron::from_str(ron_str).map_err(|e| format!("Failed to parse shape: {e}"))
}

pub fn spawn_shape(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    shape_file: &ShapeFile,
) -> Entity {
    let default_color = (0.5, 0.5, 0.5);
    let root_tf = Transform::from_translation(to_vec3(shape_file.root.at));
    let root = commands.spawn((
        ShapeRoot,
        ShapePart { name: shape_file.root.name.clone() },
        BaseTransform(root_tf),
        ShapeAnimator::new(shape_file.animations.clone()),
        root_tf,
        Visibility::default(),
    )).id();

    process_node(commands, meshes, materials, root, &shape_file.root, &shape_file.templates, default_color);
    root
}

pub fn despawn_shape(commands: &mut Commands, roots: &[Entity]) {
    for &e in roots {
        commands.entity(e).despawn();
    }
}

fn process_node(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    parent: Entity,
    node: &ShapeNode,
    templates: &HashMap<String, ShapeNode>,
    inherited_color: (f32, f32, f32),
) {
    let color = node.color.unwrap_or(inherited_color);

    if let Some(template_name) = &node.template {
        if let Some(template) = templates.get(template_name) {
            let merged = merge_template(node, template);
            process_node(commands, meshes, materials, parent, &merged, templates, color);
            return;
        }
    }

    if let Some(repeat) = &node.repeat {
        let start = if repeat.center {
            -(repeat.count as f32 - 1.0) * repeat.spacing * 0.5
        } else {
            0.0
        };
        for i in 0..repeat.count {
            let mut instance = node.clone();
            instance.repeat = None;
            let at = &mut instance.at;
            match repeat.along {
                Axis::X => at.0 += start + i as f32 * repeat.spacing,
                Axis::Y => at.1 += start + i as f32 * repeat.spacing,
                Axis::Z => at.2 += start + i as f32 * repeat.spacing,
            }
            if let Some(ref name) = instance.name {
                instance.name = Some(format!("{name}_{i}"));
            }
            spawn_child(commands, meshes, materials, parent, &instance, templates, color);
        }
        return;
    }

    if let Some(axis) = &node.mirror {
        for child in &node.children {
            spawn_child(commands, meshes, materials, parent, child, templates, color);
            let mirrored = mirror_node(child, *axis);
            spawn_child(commands, meshes, materials, parent, &mirrored, templates, color);
        }
        return;
    }

    if let Some(shape) = &node.shape {
        let (mesh, material) = make_mesh(meshes, materials, shape, color, node.emissive);
        let mesh_offset = node.pivot.map(to_vec3).unwrap_or(Vec3::ZERO);
        let orient = orient_rotation(node.orient);

        if node.children.is_empty() {
            // Leaf node: attach mesh directly to parent
            commands.entity(parent).with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(mesh_offset).with_rotation(orient),
            ));
        } else {
            // Has children: split shape into its own named child so it can be
            // toggled independently from the children
            let shape_name = node.name.as_ref()
                .map(|n| format!("{n}_shape"))
                .unwrap_or_else(|| "shape".to_string());
            let shape_entity = commands.spawn((
                ShapePart { name: Some(shape_name) },
                BaseTransform(Transform::default()),
                Transform::default(),
                Visibility::default(),
            )).id();
            commands.entity(parent).add_child(shape_entity);
            commands.entity(shape_entity).with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(mesh_offset).with_rotation(orient),
            ));
        }
    }

    for child in &node.children {
        spawn_child(commands, meshes, materials, parent, child, templates, color);
    }
}

fn spawn_child(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    parent: Entity,
    node: &ShapeNode,
    templates: &HashMap<String, ShapeNode>,
    inherited_color: (f32, f32, f32),
) {
    let child_tf = Transform::from_translation(to_vec3(node.at));
    let child = commands.spawn((
        ShapePart { name: node.name.clone() },
        BaseTransform(child_tf),
        child_tf,
        Visibility::default(),
    )).id();
    commands.entity(parent).add_child(child);

    let color = node.color.unwrap_or(inherited_color);
    process_node(commands, meshes, materials, child, node, templates, color);
}

// =====================================================================
// Helpers
// =====================================================================

fn make_mesh(
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    shape: &PrimitiveShape,
    color: (f32, f32, f32),
    emissive: bool,
) -> (Handle<Mesh>, Handle<StandardMaterial>) {
    let mesh = match shape {
        PrimitiveShape::Box { size } => meshes.add(Cuboid::new(size.0, size.1, size.2)),
        PrimitiveShape::Sphere { radius } => meshes.add(Sphere::new(*radius)),
        PrimitiveShape::Cylinder { radius, height } => meshes.add(Cylinder::new(*radius, *height)),
    };

    let base_color = Color::srgb(color.0, color.1, color.2);
    let material = if emissive {
        materials.add(StandardMaterial {
            base_color,
            emissive: base_color.into(),
            ..default()
        })
    } else {
        materials.add(StandardMaterial::from_color(base_color))
    };

    (mesh, material)
}

/// Orient a shape so its primary axis (default Y) points along the given axis.
fn orient_rotation(orient: Option<Axis>) -> Quat {
    match orient {
        Some(Axis::X) => Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),  // Y axis → X axis
        Some(Axis::Z) => Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),  // Y axis → Z axis
        _ => Quat::IDENTITY, // Y (default) or None
    }
}

fn mirror_node(node: &ShapeNode, axis: Axis) -> ShapeNode {
    let mut m = node.clone();
    match axis {
        Axis::X => {
            m.at.0 = -m.at.0;
            if let Some(ref mut p) = m.pivot { p.0 = -p.0; }
        }
        Axis::Y => {
            m.at.1 = -m.at.1;
            if let Some(ref mut p) = m.pivot { p.1 = -p.1; }
        }
        Axis::Z => {
            m.at.2 = -m.at.2;
            if let Some(ref mut p) = m.pivot { p.2 = -p.2; }
        }
    }
    m.children = m.children.iter().map(|c| mirror_node(c, axis)).collect();
    if let Some(ref name) = m.name {
        m.name = Some(format!("{name}_mirrored"));
    }
    m
}

fn merge_template(instance: &ShapeNode, template: &ShapeNode) -> ShapeNode {
    ShapeNode {
        name: instance.name.clone().or(template.name.clone()),
        shape: instance.shape.clone().or(template.shape.clone()),
        at: instance.at,
        pivot: instance.pivot.or(template.pivot),
        color: instance.color.or(template.color),
        emissive: instance.emissive || template.emissive,
        orient: instance.orient.or(template.orient),
        template: None,
        children: if instance.children.is_empty() {
            template.children.clone()
        } else {
            instance.children.clone()
        },
        mirror: instance.mirror.or(template.mirror),
        repeat: instance.repeat.clone().or(template.repeat.clone()),
    }
}
