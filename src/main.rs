mod boids;
mod camera;
mod octree;
mod ui;

use bevy::prelude::*;
use clap::Parser;

use crate::{boids::BoidsPlugin, camera::CameraPlugin, octree::OctreePlugin, ui::UiPlugin};

#[derive(clap::Parser)]
pub struct Args {
    #[clap(short, default_value = "16384")]
    number: usize,
}

fn main() {
    let Args { .. } = Args::parse();
    App::new()
        .add_plugins((
            DefaultPlugins,
            BoidsPlugin,
            CameraPlugin,
            OctreePlugin,
            UiPlugin,
        ))
        .run();
}
