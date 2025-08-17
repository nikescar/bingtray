use std::collections::BTreeSet;
use egui::{Context, Ui};
use crate::gui::Demo;

struct GuiGroup {
    demos: Vec<Box<dyn Demo>>,
}

impl GuiGroup {
    pub fn new(demos: Vec<Box<dyn Demo>>) -> Self {
        Self { demos }
    }

    pub fn checkboxes(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let Self { demos } = self;
        for demo in demos {
            if demo.is_enabled(ui.ctx()) {
                let mut is_open = open.contains(demo.name());
                ui.toggle_value(&mut is_open, demo.name());
                set_open(open, demo.name(), is_open);
            }
        }
    }

    pub fn windows(&mut self, ctx: &Context, open: &mut BTreeSet<String>) {
        let Self { demos } = self;
        for demo in demos {
            let mut is_open = open.contains(demo.name());
            demo.show(ctx, &mut is_open);
            set_open(open, demo.name(), is_open);
        }
    }
}

fn set_open(open: &mut BTreeSet<String>, key: &'static str, is_open: bool) {
    if is_open {
        if !open.contains(key) {
            open.insert(key.to_owned());
        }
    } else {
        open.remove(key);
    }
}

pub struct GuiGroups {
    demos: GuiGroup,
}

impl Default for GuiGroups {
    fn default() -> Self {
        Self {
            demos: GuiGroup::new(vec![
                {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // Create a default BingtrayApp with no services initially
                        // Services will be injected when available through platform-specific setup
                        Box::<super::bingtray_app::BingtrayApp>::default()
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        // Use the WASM-specific app for web builds
                        Box::<crate::wasm::WasmBingtrayApp>::default()
                    }
                },
            ]),
        }
    }
}

impl GuiGroups {
    pub fn checkboxes(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let Self {
            demos,
        } = self;

        ui.separator();
        demos.checkboxes(ui, open);
        ui.separator();
    }

    pub fn windows(&mut self, ctx: &Context, open: &mut BTreeSet<String>) {
        let Self {
            demos,
        } = self;
        demos.windows(ctx, open);
    }
}

/// A menu bar in which you can select different demo windows to show.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct DemoWindows {
    #[cfg_attr(feature = "serde", serde(skip))]
    groups: GuiGroups,
    open: BTreeSet<String>,
}

impl Default for DemoWindows {
    fn default() -> Self {
        let open = BTreeSet::new();

        Self {
            groups: Default::default(),
            open,
        }
    }
}

impl DemoWindows {
    /// Set up platform services for the BingtrayApp (only available on non-WASM platforms)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn setup_services<W, S>(&mut self, wallpaper_setter: W, screen_size_provider: S) 
    where 
        W: super::bingtray_app::WallpaperSetter + 'static,
        S: super::bingtray_app::ScreenSizeProvider + 'static,
    {
        // Find the BingtrayApp and inject services
        for demo in &mut self.groups.demos.demos {
            if let Some(bingtray_app) = demo.as_any_mut().downcast_mut::<super::bingtray_app::BingtrayApp>() {
                bingtray_app.set_wallpaper_setter(std::sync::Arc::new(wallpaper_setter));
                bingtray_app.set_screen_size_provider(std::sync::Arc::new(screen_size_provider));
                break;
            }
        }
    }

    /// Show the app ui (menu bar and windows).
    pub fn ui(&mut self, ctx: &Context) {
        self.mobile_ui(ctx);
    }

    fn mobile_ui(&mut self, ctx: &Context) {
        self.groups.windows(ctx, &mut self.open);
    }

}
