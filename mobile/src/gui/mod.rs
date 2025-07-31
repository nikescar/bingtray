mod http_app;
pub mod gui_windows;
// mod svg_test;

// pub use http_app::HttpApp;
// pub use svg_test::SvgTest;

pub use {
    gui_windows::DemoWindows,
};

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub trait Demo {
    /// Is the demo enabled for this integration?
    fn is_enabled(&self, _ctx: &egui::Context) -> bool {
        true
    }

    /// `&'static` so we can also use it as a key to store open/close state.
    fn name(&self) -> &'static str;

    /// Show windows, etc
    fn show(&mut self, ctx: &egui::Context, open: &mut bool);
}
