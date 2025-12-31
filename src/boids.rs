use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshTag, PrimitiveTopology},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use bevy_spatial::{AutomaticUpdate, SpatialAccess, TransformMode, kdtree::KDTree3};
use rand::Rng;
use strum::VariantArray;

pub struct BoidsPlugin;

impl Plugin for BoidsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<BoidMaterial>::default(),
            AutomaticUpdate::<Boid>::new().with_transform(TransformMode::GlobalTransform),
        ))
        .init_resource::<BoidSettings>()
        .add_systems(Startup, (setup_boids, setup_default_attractor))
        .add_systems(
            Update,
            (
                update_boid_acceleration.run_if(|r_settings: Res<BoidSettings>| !r_settings.paused),
                update_boid_positions.run_if(|r_settings: Res<BoidSettings>| !r_settings.paused),
                update_boid_count.run_if(|r_settings: Res<BoidSettings>| !r_settings.paused),
            )
                .chain(),
        );
    }
}

#[derive(Resource)]
/// Configuration settings for the Boids simulation.
pub struct BoidSettings {
    /// Pause the simulation.
    pub paused: bool,
    /// The number of boids to spawn in the simulation.
    pub count: u32,
    /// The distance within which a boid can sense other boids.
    pub sense_distance: f32,
    /// The maximum number of neighbors a boid will consider when calculating its behavior.
    pub neighbour_limit: usize,
    /// The maximum speed a boid can reach.
    pub max_speed: f32,
    /// The minimum speed a boid can maintain.
    pub min_speed: f32,
    /// The maximum acceleration a boid can have.
    pub max_acceleration: f32,
    /// Weight for the separation rule.
    pub separation_weight: f32,
    /// Weight for the alignment rule.
    pub alignment_weight: f32,
    /// Weight for the cohesion rule.
    pub cohesion_weight: f32,
    /// The shalf-size of the world.
    pub world_halfsize: u32,
}

impl Default for BoidSettings {
    fn default() -> Self {
        Self {
            paused: false,
            count: 32768,
            sense_distance: 50.0,
            neighbour_limit: 8,
            world_halfsize: 512,
            max_speed: 50.0,
            min_speed: 30.0,
            max_acceleration: 25.0,
            separation_weight: 500.0,
            alignment_weight: 5.0,
            cohesion_weight: 5.0,
        }
    }
}

#[derive(Resource)]
struct BoidAssets {
    mesh: Handle<Mesh>,
    material: Handle<BoidMaterial>,
    last_tag: u32,
}

/// Marker component for a boid.
#[derive(Component, Default)]
#[require(Velocity, Acceleration)]
pub struct Boid;

/// Represents the velocity of an entity in 3D space.
#[derive(Component, Default, Debug)]
pub struct Velocity(pub Vec3);

/// Represents the acceleration of an entity in 3D space.
#[derive(Component, Default, Debug)]
pub struct Acceleration(pub Vec3);

/// Represents an attractor.
#[derive(Component)]
pub struct Attractor {
    pub mode: AttractorMode,
    pub strength: f32,
    pub repulsive: bool,
}

#[derive(VariantArray, Default, Clone, Copy, PartialEq, Eq)]
pub enum AttractorMode {
    /// Constant attraction strength, unaffected by distance.
    #[default]
    Constant,
    /// Attraction strength increases linearly with distance.
    Linear,
    /// Attraction strength increases quadratically with distance.
    Quadratic,
    /// Attraction strength decreases linearly with distance.
    InverseLinear,
    /// Attraction strength decreases quadratically with distance.
    InverseQuadratic,
}

impl std::fmt::Display for AttractorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttractorMode::Constant => write!(f, "Constant"),
            AttractorMode::Linear => write!(f, "Linear"),
            AttractorMode::Quadratic => write!(f, "Quadratic"),
            AttractorMode::InverseLinear => write!(f, "Inverse Linear"),
            AttractorMode::InverseQuadratic => write!(f, "Inverse Quadratic"),
        }
    }
}

impl Attractor {
    pub fn weight(&self, distance: f32) -> f32 {
        let weight = match self.mode {
            AttractorMode::Constant => 1.0,
            AttractorMode::Linear => distance,
            AttractorMode::Quadratic => distance * distance,
            AttractorMode::InverseLinear => 1.0 / distance,
            AttractorMode::InverseQuadratic => 1.0 / (distance * distance),
        };
        weight * self.strength * self.repulsive.then_some(-1.0).unwrap_or(1.0)
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct BoidMaterial {}

impl Material for BoidMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/boids.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/boids.wgsl".into()
    }

    fn specialize(
        _: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _: &bevy::mesh::MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

fn setup_boids(
    mut commands: Commands,
    settings: Res<BoidSettings>,
    mut materials: ResMut<Assets<BoidMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mesh = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                // XZ plane
                [-0.5, 0.0, 0.0],
                [0.5, 0.0, 0.0],
                [0.0, 0.0, -1.0],
                // YZ plane
                [0.0, -0.5, 0.0],
                [0.0, 0.5, 0.0],
                [0.0, 0.0, -1.0],
            ],
        )
        .with_inserted_indices(Indices::U16(vec![0, 1, 2, 3, 4, 5]))
        .with_duplicated_vertices()
        .with_computed_flat_normals(),
    );
    let material = materials.add(BoidMaterial {});

    commands.insert_resource(BoidAssets {
        material: material.clone(),
        mesh: mesh.clone(),
        last_tag: settings.count,
    });

    let mut rng = rand::rng();

    for index in 0..settings.count {
        commands.spawn((
            Boid,
            Transform::from_translation(Vec3::new(
                rng.random_range(-50.0..50.0),
                rng.random_range(-50.0..50.0),
                rng.random_range(-50.0..50.0),
            )),
            Velocity(Vec3::new(
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
            )),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            MeshTag(index),
        ));
    }
}

fn setup_default_attractor(mut commands: Commands) {
    commands.spawn((
        Attractor {
            mode: AttractorMode::Linear,
            strength: 1.0,
            repulsive: false,
        },
        Transform::from_translation(Vec3::ZERO),
    ));
}

fn update_boid_count(
    mut commands: Commands,
    settings: Res<BoidSettings>,
    mut boid_assets: ResMut<BoidAssets>,
    q_boids: Query<Entity, With<Boid>>,
) {
    let mut rng = rand::rng();
    let count = q_boids.count() as u32;

    if count < settings.count {
        for idx in 0..(settings.count - count) {
            commands.spawn((
                Boid,
                Transform::from_translation(Vec3::new(
                    rng.random_range(-50.0..50.0),
                    rng.random_range(-50.0..50.0),
                    rng.random_range(-50.0..50.0),
                )),
                Velocity(Vec3::new(
                    rng.random_range(-10.0..10.0),
                    rng.random_range(-10.0..10.0),
                    rng.random_range(-10.0..10.0),
                )),
                Mesh3d(boid_assets.mesh.clone()),
                MeshMaterial3d(boid_assets.material.clone()),
                // Guarantee unique tag.
                MeshTag(boid_assets.last_tag + idx),
            ));
        }
        boid_assets.last_tag += settings.count - count;
    } else if count > settings.count {
        let excess = count - settings.count;
        for entity in q_boids.iter().take(excess as usize) {
            commands.entity(entity).despawn();
        }
    }
}

fn update_boid_acceleration(
    r_tree: Res<KDTree3<Boid>>,
    r_settings: Res<BoidSettings>,
    q_boid_read: Query<&Velocity, With<Boid>>,
    q_attractor: Query<(&Attractor, &Transform)>,
    mut q_boid_write: Query<(Entity, &Transform, &Velocity, &mut Acceleration), With<Boid>>,
) {
    q_boid_write
        .par_iter_mut()
        .for_each(|(a_idx, a_tr, a_vel, mut a_accel)| {
            let a_pos = a_tr.translation;

            let mut avg_pos = Vec3::ZERO;
            let mut avg_vel = Vec3::ZERO;
            let mut separation = Vec3::ZERO;
            let mut total_weight = 0.0;

            for (n_pos, n_idx) in r_tree
                .k_nearest_neighbour(a_pos, r_settings.neighbour_limit)
                .into_iter()
                .filter(|(n_pos, _)| n_pos.distance(a_pos) <= r_settings.sense_distance)
                .filter_map(|(n_pos, n_idx)| n_idx.map(|n_idx| (n_pos, n_idx)))
                .filter(|(_, n_idx)| *n_idx != a_idx)
            {
                let distance = a_pos.distance(n_pos);
                if distance > 0.0 {
                    let weight = 1.0 / distance;
                    total_weight += weight;

                    if let Ok(Velocity(n_vel)) = q_boid_read.get(n_idx) {
                        avg_pos += n_pos * weight;
                        avg_vel += *n_vel * weight;
                        separation += (a_pos - n_pos) / (a_pos - n_pos).length_squared();
                    }
                }
            }

            if total_weight > 0.0 {
                avg_pos /= total_weight;
                avg_vel /= total_weight;
                separation /= total_weight;
            }

            a_accel.0 = Vec3::ZERO;

            // 1. Separation
            a_accel.0 += separation * r_settings.separation_weight;

            // 2. Alignment
            a_accel.0 += (avg_vel - a_vel.0) * r_settings.alignment_weight;

            // 3. Cohesion
            a_accel.0 += (avg_pos - a_pos) * r_settings.cohesion_weight;

            // 4. Attraction
            for (attractor, tr) in q_attractor {
                a_accel.0 += (tr.translation - a_pos).normalize()
                    * attractor.weight(a_pos.distance(tr.translation));
            }
        })
}

fn update_boid_positions(
    r_time: Res<Time>,
    r_settings: Res<BoidSettings>,
    mut query: Query<(&Acceleration, &mut Velocity, &mut Transform)>,
) {
    query
        .par_iter_mut()
        .for_each(|(Acceleration(accel), mut velocity, mut transform)| {
            transform.translation += velocity.0 * r_time.delta_secs() / 2.0;
            velocity.0 = (velocity.0
                + accel.clamp_length_max(r_settings.max_acceleration) * r_time.delta_secs())
            .normalize()
            .clamp_length(r_settings.min_speed, r_settings.max_speed);
            transform.translation += velocity.0 * r_time.delta_secs() / 2.0;

            if let Ok(dir) = Dir3::new(velocity.0) {
                transform.look_to(dir, Dir3::Y);
            }

            let world_size = r_settings.world_halfsize as f32;
            if transform.translation.x > world_size {
                transform.translation.x = -world_size;
            } else if transform.translation.x < -world_size {
                transform.translation.x = world_size;
            }

            if transform.translation.y > world_size {
                transform.translation.y = -world_size;
            } else if transform.translation.y < -world_size {
                transform.translation.y = world_size;
            }

            if transform.translation.z > world_size {
                transform.translation.z = -world_size;
            } else if transform.translation.z < -world_size {
                transform.translation.z = world_size;
            }
        });
}
