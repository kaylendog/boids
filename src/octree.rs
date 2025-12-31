use bevy::prelude::*;
use oktree::prelude::*;

use crate::boids::{Boid, BoidSettings};

pub struct OctreePlugin;

impl Plugin for OctreePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OctreeSettings>()
            .add_message::<BuildOctreeMesage>()
            .add_systems(
                Update,
                (
                    build_octree,
                    render_octree,
                    auto_update_octree.run_if(|s: Res<OctreeSettings>| s.auto_update),
                ),
            );
    }
}

#[derive(Resource)]
pub struct OctreeSettings {
    pub resolution: u32,
    pub auto_update: bool,
    pub auto_update_timer: Timer,
    pub max_size_shown: u32,
    pub min_size_shown: u32,
    pub opacity: f32,
}

impl Default for OctreeSettings {
    fn default() -> Self {
        Self {
            resolution: 512,
            auto_update: false,
            auto_update_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            max_size_shown: 4,
            min_size_shown: 1,
            opacity: 0.2,
        }
    }
}

#[derive(Resource)]
pub struct Tree(Octree<u32, TUVec3u32>);

#[derive(Message, Default)]
pub struct BuildOctreeMesage;

fn build_octree(
    mut commands: Commands,
    mut m_build: MessageReader<BuildOctreeMesage>,
    r_octree_settings: Res<OctreeSettings>,
    r_boid_settings: Res<BoidSettings>,
    q_boids: Query<&Transform, With<Boid>>,
) {
    for _ in m_build.read() {
        let mut tree = Octree::from_aabb(Aabb::from_min_max(
            TUVec3::zero(),
            TUVec3::splat(r_octree_settings.resolution),
        ));

        for tr in q_boids {
            let offset_pos = tr.translation + Vec3::splat(r_boid_settings.world_halfsize as f32);
            let cell = (offset_pos
                / ((r_boid_settings.world_halfsize as f32 * 2.0)
                    / r_octree_settings.resolution as f32))
                .floor()
                .as_uvec3()
                .clamp(UVec3::ZERO, UVec3::splat(r_octree_settings.resolution - 1));

            let _ = tree.insert(TUVec3u32::new(cell.x, cell.y, cell.z));
        }

        commands.insert_resource(Tree(tree));
    }
}

fn auto_update_octree(
    r_time: Res<Time>,
    mut r_octree_settings: ResMut<OctreeSettings>,
    mut msg_w_octree: MessageWriter<BuildOctreeMesage>,
) {
    r_octree_settings.auto_update_timer.tick(r_time.delta());
    if r_octree_settings.auto_update_timer.just_finished() {
        msg_w_octree.write(BuildOctreeMesage);
    }
}

fn render_octree(
    r_octree: Option<Res<Tree>>,
    r_boid_settings: Res<BoidSettings>,
    r_octree_settings: Res<OctreeSettings>,
    mut gizmos: Gizmos,
) {
    let Tree(tree) = match &r_octree {
        Some(r_octree) => r_octree.as_ref(),
        None => return,
    };

    let half_f = r_boid_settings.world_halfsize as f32;
    let inv_res = 1.0 / r_octree_settings.resolution as f32;

    // World units per *half bin*
    let half_bin = half_f * inv_res;

    for node in tree.iter_nodes().filter(|node| {
        (r_octree_settings.min_size_shown..=r_octree_settings.max_size_shown)
            .contains(&node.aabb.size())
    }) {
        let size = node.aabb.size().max(1) as f32;

        let center =
            (Vec3::from(node.aabb.center()) * 2.0 + Vec3::ONE) * half_bin - Vec3::splat(half_f);
        let extent = Vec3::splat(size * 2.0 * half_bin);

        gizmos.cuboid(
            Transform::from_translation(center).with_scale(extent),
            Color::Hsva(Hsva {
                hue: (node.aabb.size().ilog2() as f32
                    / r_octree_settings.max_size_shown.ilog2() as f32)
                    * 360.0,
                saturation: 1.0,
                value: 1.0,
                alpha: r_octree_settings.opacity,
            }),
        );
    }
}
