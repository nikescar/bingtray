use std::collections::BTreeSet;

use egui::{Context, Modifiers, ScrollArea, Ui};

// use super::HttpApp;
// use super::ImageViewer;
use crate::Demo;

// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------

pub struct GuiGroups {
    // http_app: HttpApp,
    // image_viewer: ImageViewer,
    demos: GuiGroup,
}

impl Default for GuiGroups {
    fn default() -> Self {
        Self {
            // http_app: HttpApp::default(),
            // image_viewer: ImageViewer::default(),
            demos: GuiGroup::new(vec![
                Box::<super::http_app::HttpApp>::default(),
                // Box::<super::svg_test::SvgTest>::default(),
                // Add actual demo modules here when available
            ]),
        }
    }
}

impl GuiGroups {
    pub fn checkboxes(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let Self {
            // http_app,
            // image_viewer,
            demos,
        } = self;

        {
            // let mut is_open = open.contains(http_app.name());
            // ui.toggle_value(&mut is_open, http_app.name());
            // set_open(open, http_app.name(), is_open);
        }
        ui.separator();
        // image_viewer.checkboxes(ui, open);
        ui.separator();
        demos.checkboxes(ui, open);
        ui.separator();
    }

    pub fn windows(&mut self, ctx: &Context, open: &mut BTreeSet<String>) {
        let Self {
            // http_app,
            // image_viewer,
            demos,
        } = self;
        {
            // let mut is_open = open.contains(http_app.name());
            // http_app.show(ctx, &mut is_open);
            // set_open(open, http_app.name(), is_open);
        }
        //image_viewer.windows(ctx, open);
        demos.windows(ctx, open);
    }
}

// ----------------------------------------------------------------------------

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

        // Open HTTP app by default
        // set_open(&mut open, HttpApp::default().name(), true);

        Self {
            groups: Default::default(),
            open,
        }
    }
}

impl DemoWindows {
    /// Show the app ui (menu bar and windows).
    pub fn ui(&mut self, ctx: &Context) {
        // if is_mobile(ctx) {
        //     self.mobile_ui(ctx);
        // } else {
        //     self.desktop_ui(ctx);
        // }
        self.mobile_ui(ctx);
    }

    // fn http_app_is_open(&self) -> bool {
    //     self.open.contains(HttpApp::default().name())
    // }

    fn mobile_ui(&mut self, ctx: &Context) {
        // if self.http_app_is_open() {
        //     let mut close = false;
        //     egui::CentralPanel::default().show(ctx, |ui| {
        //         egui::ScrollArea::vertical()
        //             .auto_shrink(false)
        //             .show(ui, |ui| {
        //                 #[cfg(target_os = "android")]
        //                 ui.add_space(40.0);
        //                 ui.vertical_centered_justified(|ui| {
        //                     if ui
        //                         .button(egui::RichText::new("Continue to the demo!").size(20.0))
        //                         .clicked()
        //                     {
        //                         // close = true;
        //                         ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        //                     }
        //                 });
        //                 self.groups.http_app.ui(ui);
                        
        //             });
        //     });
        //     if close {
        //         set_open(&mut self.open, HttpApp::default().name(), false);
        //     }
        // } else {
        //     self.mobile_top_bar(ctx);
        //     self.groups.windows(ctx, &mut self.open);
        // }
        self.mobile_top_bar(ctx);
        self.groups.windows(ctx, &mut self.open);
    }

    fn mobile_top_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            // put top padding above top menu_bar
            #[cfg(target_os = "android")]
            ui.add_space(40.0);
            egui::menu::bar(ui, |ui| {
                let font_size = 16.5;

                ui.menu_button(egui::RichText::new("⏷ menu").size(font_size), |ui| {
                    ui.set_style(ui.ctx().style()); // ignore the "menu" style set by `menu_button`.
                    self.demo_list_ui(ui);
                    if ui.ui_contains_pointer() && ui.input(|i| i.pointer.any_click()) {
                        ui.close_menu();
                    }
                });

            });
        });
    }

    fn desktop_ui(&mut self, ctx: &Context) {
        egui::SidePanel::right("menu")
            .resizable(false)
            .default_width(160.0)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.vertical_centered(|ui| {
                    ui.heading("Menu 메뉴");
                });

                ui.separator();

                self.demo_list_ui(ui);
            });

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                file_menu_button(ui);
            });
        });

        self.groups.windows(ctx, &mut self.open);
    }

    fn demo_list_ui(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                self.groups.checkboxes(ui, &mut self.open);
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Organize windows").clicked() {
                    ui.ctx().memory_mut(|mem| mem.reset_areas());
                }
            });
        });
    }
}

// ----------------------------------------------------------------------------

fn file_menu_button(ui: &mut Ui) {
    let organize_shortcut =
        egui::KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, egui::Key::O);
    let reset_shortcut =
        egui::KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, egui::Key::R);

    // NOTE: we must check the shortcuts OUTSIDE of the actual "File" menu,
    // or else they would only be checked if the "File" menu was actually open!

    if ui.input_mut(|i| i.consume_shortcut(&organize_shortcut)) {
        ui.ctx().memory_mut(|mem| mem.reset_areas());
    }

    if ui.input_mut(|i| i.consume_shortcut(&reset_shortcut)) {
        ui.ctx().memory_mut(|mem| *mem = Default::default());
    }

    ui.menu_button("File", |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

        // On the web the browser controls the zoom
        #[cfg(not(target_arch = "wasm32"))]
        {
            egui::gui_zoom::zoom_menu_buttons(ui);
            ui.weak(format!(
                "Current zoom: {:.0}%",
                100.0 * ui.ctx().zoom_factor()
            ))
            .on_hover_text("The UI zoom level, on top of the operating system's default value");
            ui.separator();
        }

        if ui
            .add(
                egui::Button::new("Organize Windows")
                    .shortcut_text(ui.ctx().format_shortcut(&organize_shortcut)),
            )
            .clicked()
        {
            ui.ctx().memory_mut(|mem| mem.reset_areas());
            ui.close_menu();
        }

        if ui
            .add(
                egui::Button::new("Reset egui memory")
                    .shortcut_text(ui.ctx().format_shortcut(&reset_shortcut)),
            )
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            ui.ctx().memory_mut(|mem| *mem = Default::default());
            ui.close_menu();
        }
    });
}
