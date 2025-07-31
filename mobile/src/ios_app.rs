#[cfg(target_os = "ios")]
use bevy::prelude::*;

#[cfg(target_os = "ios")]
use bevy_egui::{
    egui, EguiContexts, EguiPlugin,
};

#[cfg(target_os = "ios")]
use crate::gui::DemoWindows;

#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn main_rs() {
    ios_main();
}

#[cfg(target_os = "ios")]
fn ios_main() {
    // Set up audio session for iOS
    unsafe {
        if let Err(e) = objc2_avf_audio::AVAudioSession::sharedInstance()
            .setCategory_error(objc2_avf_audio::AVAudioSessionCategoryAmbient.unwrap())
        {
            println!("Error setting audio session category: {:?}", e);
        }
    }

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.25, 0.25, 0.25)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resizable: false,
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .insert_resource(DemoWindows::default())
        .add_systems(Startup, setup_ios_system)
        .add_systems(Update, ui_ios_system)
        .run();
}

#[cfg(target_os = "ios")]
fn setup_ios_system(
    mut commands: Commands,
) {
    // Add a camera
    commands.spawn(Camera2d);
}

#[cfg(target_os = "ios")]
fn ui_ios_system(
    mut contexts: EguiContexts,
    mut demo_windows: ResMut<DemoWindows>,
) {
    let ctx = contexts.ctx_mut();
    
    // Render the same demo UI that we use on Android/Desktop
    demo_windows.ui(ctx);
}
