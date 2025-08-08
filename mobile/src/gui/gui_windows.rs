use std::collections::BTreeSet;
use egui::{Context, ScrollArea, Ui};
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
                Box::<super::bingtray_app::BingtrayApp>::default(),
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
    /// Show the app ui (menu bar and windows).
    pub fn ui(&mut self, ctx: &Context) {
        self.mobile_ui(ctx);
    }

    fn mobile_ui(&mut self, ctx: &Context) {
        self.mobile_top_bar(ctx);
        self.groups.windows(ctx, &mut self.open);
    }

    fn mobile_top_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            // put top padding above top menu_bar for Android status bar
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
