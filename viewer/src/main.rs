use std::{f32::consts::FRAC_PI_2, ops::Range};

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::mesh::PrimitiveTopology,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use cellular::*;
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
}

impl Default for Config3d {
    fn default() -> Self {
        Self {
            size: UVec3::new(32, 32, 32),
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
struct Config {
    pub seed: u64,
    pub auto_regenerate: bool,
    pub fill_rate: f32,
    pub smooth_iter: i32,
    pub config_nd: ConfigNd,
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 42,
            auto_regenerate: true,
            fill_rate: 0.70,
            smooth_iter: 4,
            config_nd: default(),
            mesh: default(),
            material: default(),
        }
    }
}

impl Config {
    pub fn default_2d() -> Self {
        Self {
            fill_rate: 0.45,
            smooth_iter: 1,
            config_nd: ConfigNd::D2(default()),
            ..default()
        }
    }

    pub fn default_3d() -> Self {
        Self {
            fill_rate: 0.70,
            smooth_iter: 4,
            config_nd: ConfigNd::D3(default()),
            ..default()
        }
    }
}

#[derive(Debug, Resource)]
struct CameraSettings {
    pub distance_speed: f32,
    pub pitch_speed: f32,
    // Clamp pitch to this range
    pub pitch_range: Range<f32>,
    pub roll_speed: f32,
    pub yaw_speed: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        // Limiting pitch stops some unexpected rotation past 90° up or down.
        let pitch_limit = FRAC_PI_2 - 0.01;
        Self {
            distance_speed: 3.0,
            pitch_speed: 0.003,
            pitch_range: -pitch_limit..pitch_limit,
            roll_speed: 1.0,
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
    mut config: ResMut<Config>,
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

fn ui_config(mut contexts: EguiContexts, mut config: ResMut<Config>) {
    egui::Window::new("Config").show(contexts.ctx_mut(), |ui| {
        let mut old_config = config.clone();

        ui.checkbox(&mut old_config.auto_regenerate, "Auto-regenerate");
        if !old_config.auto_regenerate && ui.button("Regenerate").clicked() {
            // Just "touch" the config to mark it changed, which will trigger a regenerate
            config.set_changed();
        }

        if let ConfigNd::D3(config_3d) = &mut old_config.config_nd {
            ui.add(egui::Slider::new(&mut config_3d.size.x, 4..=128).text("Size"));
            config_3d.size = UVec3::splat(config_3d.size.x);
        }

        ui.separator();

        ui.label("Initial fill");
        ui.indent(1, |ui| {
            ui.add(egui::Slider::new(&mut old_config.fill_rate, 0.0..=1.0).text("Fill rate"));
            ui.add(egui::Slider::new(&mut old_config.smooth_iter, 0..=5).text("Smooth iterations"));
        });

        // Ensure we don't trigger the Bevy change detection if nothing changed
        config.set_if_neq(old_config);
    });
}

fn generate_mesh(
    mut meshes: ResMut<Assets<Mesh>>,
    config: Res<Config>,
    q_root: Query<&Mesh3d, With<Root>>,
) {
    if !config.is_changed() || !config.auto_regenerate {
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

                    cave.smooth();
                }

                cave
            };

            {
                #[cfg(feature = "trace")]
                let _span = info_span!("build_mesh").entered();

                //let offset = -config_3d.size.as_vec3() / 2.;
                *mesh = build_mesh(&cave);
            }
        }

        ConfigNd::D2(config_2d) => {
            let mut cave = Grid2::new(config_2d.size);
            let mut prng = ChaCha8Rng::seed_from_u64(config.seed);
            cave.fill_rand(config.fill_rate, &mut prng);
        }
    }
}

fn generate_cubes(mut commands: Commands, config: Res<Config>, q_root: Query<Entity, With<Root>>) {
    if !config.is_changed() || !config.auto_regenerate {
        return;
    }

    // Clear all cubes
    let mut cmd = commands.entity(q_root.single());
    cmd.despawn_descendants();

    // Spawn new cubes
    cmd.with_children(|parent| match &config.config_nd {
        ConfigNd::D3(config_3d) => {
            let mut cave = Grid3::new(config_3d.size);
            let mut prng = ChaCha8Rng::seed_from_u64(config.seed);
            cave.fill_rand(config.fill_rate, &mut prng);
            for _ in 0..config.smooth_iter {
                cave.smooth();
            }

            let offset = -config_3d.size.as_vec3() / 2.;
            for k in 0..config_3d.size.z {
                for j in 0..config_3d.size.y {
                    for i in 0..config_3d.size.x {
                        let pos = IVec3::new(i as i32, j as i32, k as i32);
                        if cave.cell(pos).unwrap_or(false) {
                            parent.spawn((
                                Mesh3d(config.mesh.clone()),
                                MeshMaterial3d(config.material.clone()),
                                Transform::from_translation(offset + pos.as_vec3()),
                            ));
                        }
                    }
                }
            }
        }

        ConfigNd::D2(config_2d) => {
            let mut cave = Grid2::new(config_2d.size);
            let mut prng = ChaCha8Rng::seed_from_u64(config.seed);
            cave.fill_rand(config.fill_rate, &mut prng);
        }
    });
}

fn build_mesh(grid: &Grid3) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    let x = grid.size.x as usize;
    let y = grid.size.y as usize;
    let z = grid.size.z as usize;
    let capacity = ((z + 1) * x * y + (y + 1) * z * x + (x + 1) * z * y) * 6;
    let mut vertices = Vec::with_capacity(capacity);
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

                    if prev {
                        vertices.push(min);
                        vertices.push(min + Vec3::Y);
                        vertices.push(min + Vec3::Z);

                        vertices.push(min + Vec3::Z);
                        vertices.push(min + Vec3::Y);
                        vertices.push(min + Vec3::Y + Vec3::Z);
                    } else {
                        vertices.push(min);
                        vertices.push(min + Vec3::Z);
                        vertices.push(min + Vec3::Y);

                        vertices.push(min + Vec3::Y);
                        vertices.push(min + Vec3::Z);
                        vertices.push(min + Vec3::Y + Vec3::Z);
                    }
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

                    if prev {
                        vertices.push(min);
                        vertices.push(min + Vec3::Z);
                        vertices.push(min + Vec3::X);

                        vertices.push(min + Vec3::X);
                        vertices.push(min + Vec3::Z);
                        vertices.push(min + Vec3::X + Vec3::Z);
                    } else {
                        vertices.push(min);
                        vertices.push(min + Vec3::X);
                        vertices.push(min + Vec3::Z);

                        vertices.push(min + Vec3::Z);
                        vertices.push(min + Vec3::X);
                        vertices.push(min + Vec3::X + Vec3::Z);
                    }
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

                    if prev {
                        vertices.push(min);
                        vertices.push(min + Vec3::X);
                        vertices.push(min + Vec3::Y);

                        vertices.push(min + Vec3::Y);
                        vertices.push(min + Vec3::X);
                        vertices.push(min + Vec3::X + Vec3::Y);
                    } else {
                        vertices.push(min);
                        vertices.push(min + Vec3::Y);
                        vertices.push(min + Vec3::X);

                        vertices.push(min + Vec3::X);
                        vertices.push(min + Vec3::Y);
                        vertices.push(min + Vec3::X + Vec3::Y);
                    }
                }
            }
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

    mesh.with_computed_flat_normals()
}

fn orbit_camera(
    mut camera: Single<&mut Transform, With<Camera>>,
    camera_settings: Res<CameraSettings>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
) {
    if !mouse_buttons.pressed(MouseButton::Right) {
        return;
    }

    let delta = -mouse_motion.delta;
    // let mut delta_roll = 0.0;

    // if mouse_buttons.pressed(MouseButton::Left) {
    //     delta_roll -= 1.0;
    // }
    // if mouse_buttons.pressed(MouseButton::Right) {
    //     delta_roll += 1.0;
    // }

    let mut distance = camera.translation.length();
    distance -= mouse_scroll.delta.y * camera_settings.distance_speed;

    // Mouse motion is one of the few inputs that should not be multiplied by delta
    // time, as we are already receiving the full movement since the last frame
    // was rendered. Multiplying by delta time here would make the movement
    // slower that it should be.
    let delta_pitch = delta.y * camera_settings.pitch_speed;
    let delta_yaw = delta.x * camera_settings.yaw_speed;

    // Conversely, we DO need to factor in delta time for mouse button inputs.
    //delta_roll *= camera_settings.roll_speed * time.delta_secs();

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

    // Adjust the translation to maintain the correct orientation toward the orbit
    // target. In our example it's a static target, but this could easily be
    // customized.
    let target = Vec3::ZERO;
    camera.translation = target - camera.forward() * distance;
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cave Viewer".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .init_resource::<Config>()
        .init_resource::<CameraSettings>()
        .add_systems(Startup, setup)
        .add_systems(Update, close_on_esc)
        .add_systems(Update, ui_config)
        .add_systems(Update, orbit_camera)
        //.add_systems(PostUpdate, generate_cubes)
        .add_systems(PostUpdate, generate_mesh)
        .run();
}
