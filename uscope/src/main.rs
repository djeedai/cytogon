use std::{f32::consts::FRAC_PI_2, ops::Range};

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::mesh::{PrimitiveTopology, VertexAttributeValues},
};
use bevy_egui::{
    egui::{self, CollapsingHeader, Rounding},
    EguiContexts, EguiPlugin,
};
use cytogon::*;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng as _;

#[derive(Clone, Copy, PartialEq)]
struct Config2d {
    pub size: UVec2,
}

impl Default for Config2d {
    fn default() -> Self {
        Self {
            size: UVec2::new(128, 64),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Config3d {
    pub size: UVec3,
    pub rule: Rule3,
}

impl Default for Config3d {
    fn default() -> Self {
        Self {
            size: UVec3::splat(64),
            rule: Rule3::SMOOTH,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ConfigNd {
    D2(Config2d),
    D3(Config3d),
}

impl Default for ConfigNd {
    fn default() -> Self {
        Self::D3(default())
    }
}

#[derive(Resource, Clone, PartialEq)]
struct Settings {
    pub seed: u64,
    pub auto_regenerate: bool,
    pub fill_rate: f32,
    pub smooth_iter: i32,
    pub config_nd: ConfigNd,
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            seed: 42,
            auto_regenerate: true,
            fill_rate: 0.508,
            smooth_iter: 20,
            config_nd: default(),
            mesh: default(),
            material: default(),
        }
    }
}

impl Settings {
    /// Default configuration for 2D.
    pub fn default_2d() -> Self {
        Self {
            fill_rate: 0.45,
            smooth_iter: 1,
            config_nd: ConfigNd::D2(default()),
            ..default()
        }
    }

    /// Default configuration for 3D.
    pub fn default_3d() -> Self {
        Self {
            fill_rate: 0.508,
            smooth_iter: 20,
            config_nd: ConfigNd::D3(default()),
            ..default()
        }
    }
}

#[derive(Debug, Resource)]
struct CameraSettings {
    pub distance_speed: f32,
    pub pitch_speed: f32,
    pub pitch_range: Range<f32>,
    pub yaw_speed: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        // Limiting pitch stops some unexpected rotation past 90° up or down.
        let pitch_limit = FRAC_PI_2 - 0.01;
        Self {
            distance_speed: 6.0,
            pitch_speed: 0.003,
            pitch_range: -pitch_limit..pitch_limit,
            yaw_speed: 0.004,
        }
    }
}

#[derive(Component)]
struct Root;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut config: ResMut<Settings>,
) {
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 48.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // light
    commands.spawn((
        DirectionalLight {
            color: LinearRgba::rgb(1.0, 0.8, 0.8).into(),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-30.0, 48.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            color: LinearRgba::rgb(0.8, 0.8, 1.0).into(),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(50.0, -32.0, -16.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // rendering data for cubes
    config.mesh = meshes.add(Cuboid::from_size(Vec3::ONE));
    config.material = materials.add(Color::srgb_u8(124, 144, 255));

    // object root
    //commands.spawn((Transform::IDENTITY, Visibility::Visible, Root));
    commands.spawn((
        Transform::IDENTITY,
        Mesh3d(config.mesh.clone()),
        MeshMaterial3d(config.material.clone()),
        Root,
    ));
}

/// Helper system to enable closing the example application by pressing the
/// escape key (ESC).
pub fn close_on_esc(mut ev_app_exit: EventWriter<AppExit>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::Escape) {
        ev_app_exit.send(AppExit::Success);
    }
}

/// Setup UI theme after egui was initialized.
fn setup_ui(mut contexts: EguiContexts) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style
            .text_styles
            .get_mut(&egui::TextStyle::Heading)
            .map(|ts| ts.size = 14.0);
        style.visuals.window_rounding = Rounding::ZERO;
        style.visuals.button_frame = true;
        style.visuals.collapsing_header_frame = true;
        style.visuals.indent_has_left_vline = false;
        style.visuals.slider_trailing_fill = true;
    });
}

fn ui_settings(mut contexts: EguiContexts, mut settings: ResMut<Settings>) {
    egui::Window::new("Settings").show(contexts.ctx_mut(), |ui| {
        let mut prev_settings = settings.clone();

        let mut size = match &prev_settings.config_nd {
            ConfigNd::D2(config_2d) => config_2d.size.x,
            ConfigNd::D3(config_3d) => config_3d.size.x,
        };
        ui.add(egui::Slider::new(&mut size, 4..=128).text("Grid size"));

        CollapsingHeader::new("Initial fill")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut prev_settings.fill_rate, 0.0..=1.0).text("Fill rate"),
                );
            });

        match &mut prev_settings.config_nd {
            ConfigNd::D3(config_3d) => {
                config_3d.size = UVec3::splat(size);

                CollapsingHeader::new("Rule")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Slider::new(&mut prev_settings.smooth_iter, 0..=50)
                                .text("Repeat count"),
                        );
                        ui.label("Birth");
                        let mut bits = config_3d.rule.birth.to_array();
                        ui.checkbox(&mut bits[0], "0");
                        ui.horizontal(|ui| {
                            for b in &mut bits[1..13] {
                                ui.checkbox(b, "");
                            }
                            ui.checkbox(&mut bits[13], "13");
                        });
                        ui.horizontal(|ui| {
                            for b in &mut bits[14..26] {
                                ui.checkbox(b, "");
                            }
                            ui.checkbox(&mut bits[26], "26");
                        });
                        config_3d.rule.birth = bits.into();

                        ui.label("Survive");
                        let mut bits = config_3d.rule.survive.to_array();
                        ui.checkbox(&mut bits[0], "0");
                        ui.horizontal(|ui| {
                            for b in &mut bits[1..13] {
                                ui.checkbox(b, "");
                            }
                            ui.checkbox(&mut bits[13], "13");
                        });
                        ui.horizontal(|ui| {
                            for b in &mut bits[14..26] {
                                ui.checkbox(b, "");
                            }
                            ui.checkbox(&mut bits[26], "26");
                        });
                        config_3d.rule.survive = bits.into();
                    });
            }
            ConfigNd::D2(config_2d) => {
                config_2d.size = UVec2::splat(size);
            }
        }

        ui.separator();

        ui.checkbox(
            &mut prev_settings.auto_regenerate,
            "Auto-regenerate on change",
        );
        if !prev_settings.auto_regenerate && ui.button("Regenerate").clicked() {
            // Just "touch" the config to mark it changed, which will trigger a regenerate
            settings.set_changed();
        }

        // Ensure we don't trigger the Bevy change detection if nothing changed
        settings.set_if_neq(prev_settings);
    });
}

fn generate_mesh(
    mut meshes: ResMut<Assets<Mesh>>,
    config: Res<Settings>,
    q_root: Query<&Mesh3d, With<Root>>,
) {
    if !config.is_changed() {
        return;
    }

    let handle = q_root.single();
    let mesh = meshes.get_mut(handle).unwrap();

    // Spawn new cubes
    match &config.config_nd {
        ConfigNd::D3(config_3d) => {
            let cave = {
                #[cfg(feature = "trace")]
                let _span = info_span!("generate").entered();

                let mut cave = Grid3::new(config_3d.size);
                {
                    #[cfg(feature = "trace")]
                    let _span = info_span!("fill_rand").entered();

                    let mut prng = ChaCha8Rng::seed_from_u64(config.seed);
                    cave.fill_rand(config.fill_rate, &mut prng);
                }

                for _ in 0..config.smooth_iter {
                    #[cfg(feature = "trace")]
                    let _span = info_span!("smooth").entered();

                    cave.apply_rule(&config_3d.rule);
                }

                cave
            };

            {
                #[cfg(feature = "trace")]
                let _span = info_span!("rebuild_mesh").entered();

                //let offset = -config_3d.size.as_vec3() / 2.;
                rebuild_mesh(&cave, mesh);
            }
        }

        ConfigNd::D2(config_2d) => {
            let mut cave = Grid2::new(config_2d.size);
            let mut prng = ChaCha8Rng::seed_from_u64(config.seed);
            cave.fill_rand(config.fill_rate, &mut prng);
        }
    }
}

fn generate_cubes(
    mut commands: Commands,
    settings: Res<Settings>,
    q_root: Query<Entity, With<Root>>,
) {
    if !settings.is_changed() || !settings.auto_regenerate {
        return;
    }

    // Clear all cubes
    let mut cmd = commands.entity(q_root.single());
    cmd.despawn_descendants();

    // Spawn new cubes
    cmd.with_children(|parent| match &settings.config_nd {
        ConfigNd::D3(config_3d) => {
            let mut cave = Grid3::new(config_3d.size);
            let mut prng = ChaCha8Rng::seed_from_u64(settings.seed);
            cave.fill_rand(settings.fill_rate, &mut prng);
            for _ in 0..settings.smooth_iter {
                cave.apply_rule(&Rule3::SMOOTH);
            }

            let offset = -config_3d.size.as_vec3() / 2.;
            for k in 0..config_3d.size.z {
                for j in 0..config_3d.size.y {
                    for i in 0..config_3d.size.x {
                        let pos = IVec3::new(i as i32, j as i32, k as i32);
                        if cave.cell(pos).unwrap_or(false) {
                            parent.spawn((
                                Mesh3d(settings.mesh.clone()),
                                MeshMaterial3d(settings.material.clone()),
                                Transform::from_translation(offset + pos.as_vec3()),
                            ));
                        }
                    }
                }
            }
        }

        ConfigNd::D2(config_2d) => {
            let mut cave = Grid2::new(config_2d.size);
            let mut prng = ChaCha8Rng::seed_from_u64(settings.seed);
            cave.fill_rand(settings.fill_rate, &mut prng);
        }
    });
}

fn rebuild_mesh(grid: &Grid3, mesh: &mut Mesh) {
    assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
    assert_eq!(
        mesh.asset_usage,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD
    );

    // Ensure the geometry is not indexed, otherwise compute_flat_normals() doesn't
    // work
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("remove_unused").entered();

        mesh.remove_indices();
        mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0);
    }

    let x = grid.size.x as usize;
    let y = grid.size.y as usize;
    let z = grid.size.z as usize;
    let capacity = ((z + 1) * x * y + (y + 1) * z * x + (x + 1) * z * y) * 6;

    {
        #[cfg(feature = "trace")]
        let _span = info_span!("build").entered();

        // Steal position array
        let values = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION).unwrap();
        let VertexAttributeValues::Float32x3(vertices) = values else {
            panic!("Mesh doesn't have Float32x3 vertices.");
        };
        let mut positions = std::mem::take(vertices);
        positions.reserve(capacity / 8); // heuristic
        positions.truncate(0);

        // Steal normal array
        let values = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL).unwrap();
        let VertexAttributeValues::Float32x3(normals) = values else {
            panic!("Mesh doesn't have Float32x3 normals.");
        };
        let mut normals = std::mem::take(normals);
        normals.reserve(capacity / 8); // heuristic
        normals.truncate(0);

        let offset = -grid.size.as_vec3() / 2.;

        // X faces
        for i in 0..=grid.size.x {
            for k in 0..grid.size.z {
                for j in 0..grid.size.y {
                    let pos = IVec3::new(i as i32, j as i32, k as i32);
                    let cur = grid.cell(pos).unwrap_or(false);
                    let prev = if i > 0 {
                        grid.cell(pos - IVec3::X).unwrap_or(false)
                    } else {
                        false
                    };
                    if cur != prev {
                        let half_size = Vec3::new(0.5, 0.5, 0.5);
                        let min = offset + pos.as_vec3() - half_size;

                        let mut n = [0f32; 3];
                        if prev {
                            n[0] = 1.0;
                            positions.push(min.to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::Y + Vec3::Z).to_array());
                        } else {
                            n[0] = -1.0;
                            positions.push(min.to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::Y + Vec3::Z).to_array());
                        }

                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                    }
                }
            }
        }
        // Y faces
        for j in 0..=grid.size.y {
            for k in 0..grid.size.z {
                for i in 0..grid.size.x {
                    let pos = IVec3::new(i as i32, j as i32, k as i32);
                    let cur = grid.cell(pos).unwrap_or(false);
                    let prev = if j > 0 {
                        grid.cell(pos - IVec3::Y).unwrap_or(false)
                    } else {
                        false
                    };
                    if cur != prev {
                        let half_size = Vec3::new(0.5, 0.5, 0.5);
                        let min = offset + pos.as_vec3() - half_size;

                        let mut n = [0f32; 3];
                        if prev {
                            n[1] = 1.0;
                            positions.push(min.to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::X + Vec3::Z).to_array());
                        } else {
                            n[1] = -1.0;
                            positions.push(min.to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::Z).to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::X + Vec3::Z).to_array());
                        }

                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                    }
                }
            }
        }
        // Z faces
        for k in 0..=grid.size.z {
            for j in 0..grid.size.y {
                for i in 0..grid.size.x {
                    let pos = IVec3::new(i as i32, j as i32, k as i32);
                    let cur = grid.cell(pos).unwrap_or(false);
                    let prev = if k > 0 {
                        grid.cell(pos - IVec3::Z).unwrap_or(false)
                    } else {
                        false
                    };
                    if cur != prev {
                        let half_size = Vec3::new(0.5, 0.5, 0.5);
                        let min = offset + pos.as_vec3() - half_size;

                        let mut n = [0f32; 3];
                        if prev {
                            n[2] = 1.0;
                            positions.push(min.to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::X + Vec3::Y).to_array());
                        } else {
                            n[2] = -1.0;
                            positions.push(min.to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::X).to_array());
                            positions.push((min + Vec3::Y).to_array());
                            positions.push((min + Vec3::X + Vec3::Y).to_array());
                        }

                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                        normals.push(n);
                    }
                }
            }
        }

        // Restore positions array
        let values = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION).unwrap();
        let VertexAttributeValues::Float32x3(dummy) = values else {
            panic!("Mesh doesn't have Float32x3 vertices.");
        };
        std::mem::swap(dummy, &mut positions);

        // Restore normals array
        let values = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL).unwrap();
        let VertexAttributeValues::Float32x3(dummy) = values else {
            panic!("Mesh doesn't have Float32x3 normals.");
        };
        std::mem::swap(dummy, &mut normals);
    }
}

fn orbit_camera(
    mut camera: Single<&mut Transform, With<Camera>>,
    camera_settings: Res<CameraSettings>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
) {
    let mut distance = camera.translation.length();
    distance -= mouse_scroll.delta.y * camera_settings.distance_speed;

    if mouse_buttons.pressed(MouseButton::Right) {
        // Mouse motion is one of the few inputs that should not be multiplied by delta
        // time, as we are already receiving the full movement since the last frame
        // was rendered. Multiplying by delta time here would make the movement
        // slower that it should be.
        let delta = -mouse_motion.delta;
        let delta_pitch = delta.y * camera_settings.pitch_speed;
        let delta_yaw = delta.x * camera_settings.yaw_speed;

        // Obtain the existing pitch, yaw, and roll values from the transform.
        let (yaw, pitch, roll) = camera.rotation.to_euler(EulerRot::YXZ);

        // Establish the new yaw and pitch, preventing the pitch value from exceeding
        // our limits.
        let pitch = (pitch + delta_pitch).clamp(
            camera_settings.pitch_range.start,
            camera_settings.pitch_range.end,
        );
        //let roll = roll + delta_roll;
        let yaw = yaw + delta_yaw;
        camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }

    let target = Vec3::ZERO;
    camera.translation = target - camera.forward() * distance;
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "🔬 μscope".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .init_resource::<Settings>()
        .init_resource::<CameraSettings>()
        .add_systems(Startup, setup)
        .add_systems(
            Startup,
            setup_ui.after(bevy_egui::EguiStartupSet::InitContexts),
        )
        .add_systems(Update, close_on_esc)
        .add_systems(Update, ui_settings)
        .add_systems(Update, orbit_camera)
        //.add_systems(PostUpdate, generate_cubes)
        .add_systems(PostUpdate, generate_mesh)
        .run();
}
