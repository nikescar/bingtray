#[cfg(not(target_arch = "wasm32"))]
pub mod bingtray_app;
pub mod gui_windows;

pub use {
    gui_windows::DemoWindows,
};

#[cfg(not(target_arch = "wasm32"))]
pub use bingtray_app::BingtrayApp;

#[cfg(target_arch = "wasm32")]
pub use crate::wasm::WasmBingtrayApp;

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub fn is_mobile(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.screen_rect().width() < 768.0)
}

pub trait Demo {
    fn is_enabled(&self, _ctx: &egui::Context) -> bool {
        true
    }
    fn name(&self) -> &'static str;
    fn show(&mut self, ctx: &egui::Context, open: &mut bool);
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
