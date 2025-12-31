use bevy::{
    camera::{CameraOutputMode, visibility::RenderLayers},
    prelude::*,
    render::render_resource::BlendState,
};
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext,
};
use strum::VariantArray;

use crate::{
    boids::{Attractor, AttractorMode, Boid, BoidSettings},
    octree::{BuildOctreeMesage, OctreeSettings},
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<State>()
            .init_gizmo_group::<UiGizmos>()
            .insert_gizmo_config(
                UiGizmos,
                GizmoConfig {
                    depth_bias: -1.0,
                    ..default()
                },
            )
            .add_systems(Startup, setup_camera_system)
            .add_systems(Update, draw_gizmo)
            .add_systems(EguiPrimaryContextPass, ui);
    }
}

#[derive(Resource, Default)]
struct State {
    pub selected_attractor: Option<Entity>,
}

#[derive(GizmoConfigGroup, Reflect, Default)]
struct UiGizmos;

fn setup_camera_system(
    mut commands: Commands,
    mut egui_global_settings: ResMut<EguiGlobalSettings>,
) {
    egui_global_settings.auto_create_primary_context = false;
    commands.spawn((
        Camera2d,
        PrimaryEguiContext,
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
}

fn ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    time: Res<Time>,
    mut r_state: ResMut<State>,
    mut r_boid_settings: ResMut<BoidSettings>,
    mut r_octree_settings: ResMut<OctreeSettings>,
    mut m_octree_msgs: MessageWriter<BuildOctreeMesage>,
    q_boids: Query<&Boid>,
    q_attractors_read: Query<Entity, With<Attractor>>,
    mut q_attractors_write: Query<(&mut Transform, &mut Attractor)>,
) -> Result {
    egui::Window::new("Stats").show(contexts.ctx_mut()?, |ui| {
        ui.label(format!("Total boids: {}", q_boids.count()));
        ui.label(format!("Frame time: {:.2} ms", time.delta_secs() * 1000.0));

        ui.add(egui::Checkbox::new(&mut r_boid_settings.paused, "Paused"));
    });

    egui::Window::new("Boids").show(contexts.ctx_mut()?, |ui| {
        ui.add(egui::Slider::new(&mut r_boid_settings.count, 0..=50000).text("Count"));
        ui.add(
            egui::Slider::new(&mut r_boid_settings.sense_distance, 0.0..=1000.0)
                .text("Sense Distance"),
        );
        ui.add(
            egui::Slider::new(&mut r_boid_settings.neighbour_limit, 0..=32).text("Neighbour Limit"),
        );
        ui.add(egui::Slider::new(&mut r_boid_settings.max_speed, 0.0..=50.0).text("Max Speed"));
        ui.add(egui::Slider::new(&mut r_boid_settings.min_speed, 0.0..=50.0).text("Min Speed"));
        ui.add(
            egui::Slider::new(&mut r_boid_settings.max_acceleration, 0.0..=100.0)
                .text("Max Acceleration"),
        );
        ui.add(
            egui::Slider::new(&mut r_boid_settings.alignment_weight, 0.0..=5.0).text("Alignment"),
        );
        ui.add(egui::Slider::new(&mut r_boid_settings.cohesion_weight, 0.0..=5.0).text("Cohesion"));
        ui.add(
            egui::Slider::new(&mut r_boid_settings.separation_weight, 0.0..=500.0)
                .text("Separation"),
        );
    });

    egui::Window::new("Octree").show(contexts.ctx_mut()?, |ui| {
        ui.horizontal(|ui| {
            ui.label("Resolution");
            let resolutions = [512, 1024, 2048, 4096];
            for &res in &resolutions {
                ui.radio_value(&mut r_octree_settings.resolution, res, format!("{}", res));
            }
        });

        let resolution = r_octree_settings.resolution;
        let min_size = r_octree_settings.min_size_shown;
        let max_size = r_octree_settings.max_size_shown;

        ui.add(
            egui::Slider::new(&mut r_octree_settings.max_size_shown, min_size..=resolution)
                .logarithmic(true)
                .text("Max Size"),
        );
        ui.add(
            egui::Slider::new(&mut r_octree_settings.min_size_shown, 1..=max_size).text("Min Size"),
        );

        ui.add(egui::Slider::new(&mut r_octree_settings.opacity, 0.0..=1.0).text("Opacity"));

        ui.add(egui::Checkbox::new(
            &mut r_octree_settings.auto_update,
            "Auto Update",
        ));

        if ui.button("Build").clicked() {
            m_octree_msgs.write(BuildOctreeMesage);
        }
    });

    let mut new_attractor = None;

    egui::Window::new("Attractors").show(contexts.ctx_mut()?, |ui| {
        ui.horizontal(|ui| {
            // Adds a new attractor when pressed. This is deferred until the end of the UI update
            // to prevent issues accessing a non-existent entity.
            if ui.button("+").clicked() {
                new_attractor = Some(
                    commands
                        .spawn((
                            Transform::default(),
                            Attractor {
                                mode: AttractorMode::default(),
                                strength: 1.0,
                                repulsive: false,
                            },
                        ))
                        .id(),
                );
            }

            // Removes the currently selected attractor when pressed.
            if ui
                .add_enabled(r_state.selected_attractor.is_some(), egui::Button::new("-"))
                .clicked()
            {
                if let Some(selected) = r_state.selected_attractor {
                    commands.entity(selected).despawn();
                    r_state.selected_attractor = None;
                }
            }

            // Dropdown for selected attractor.
            egui::ComboBox::from_label("Attractor")
                .selected_text(format!("{:?}", r_state.selected_attractor))
                .show_ui(ui, |ui| {
                    for idx in q_attractors_read {
                        ui.selectable_value(
                            &mut r_state.selected_attractor,
                            Some(idx),
                            format!("{:?}", idx),
                        );
                    }
                });
        });

        if r_state.selected_attractor.is_none() {
            return;
        }

        let (mut tr, mut attractor) = q_attractors_write
            .get_mut(r_state.selected_attractor.unwrap())
            .unwrap();

        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut tr.translation.x).speed(0.1));
            ui.add(egui::DragValue::new(&mut tr.translation.y).speed(0.1));
            ui.add(egui::DragValue::new(&mut tr.translation.z).speed(0.1));
        });

        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Mode")
                .selected_text(format!("{}", attractor.mode))
                .show_ui(ui, |ui| {
                    for mode in AttractorMode::VARIANTS {
                        ui.selectable_value(&mut attractor.mode, *mode, format!("{}", mode));
                    }
                });
            ui.checkbox(&mut attractor.repulsive, "Repulsive");
        });
        ui.add(egui::Slider::new(&mut attractor.strength, 0.0..=1000.0).logarithmic(true));
    });

    // Select newly created attractor.
    if let Some(new_attractor) = new_attractor {
        r_state.selected_attractor = Some(new_attractor);
    }

    Ok(())
}

fn draw_gizmo(
    mut gizmos: Gizmos<UiGizmos>,
    r_state: Res<State>,
    q_attractors: Query<(Entity, &Transform), With<Attractor>>,
) {
    for (idx, tr) in q_attractors {
        let color = if Some(idx) == r_state.selected_attractor {
            Color::srgb(1.0, 0.0, 0.0)
        } else {
            Color::srgb(1.0, 1.0, 1.0)
        };
        gizmos.sphere(Isometry3d::from_translation(tr.translation), 10.0, color);
    }
}
