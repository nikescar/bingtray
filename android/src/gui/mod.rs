mod http_app;
pub mod gui_windows;

pub use {
    gui_windows::DemoWindows,
};

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub trait Demo {
    fn is_enabled(&self, _ctx: &egui::Context) -> bool {
        true
    }
    fn name(&self) -> &'static str;
    fn show(&mut self, ctx: &egui::Context, open: &mut bool);
}
