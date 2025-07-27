use egui::Image;
use poll_promise::Promise;
use egui::Vec2b;
use log::{trace, info, warn, error};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "android")]
use libc;

// Global cooldown for wallpaper setting to prevent UI blocking
static LAST_WALLPAPER_TIME: AtomicU64 = AtomicU64::new(0);

struct Resource {
    response: ehttp::Response,
    text: Option<String>,
    image: Option<Image<'static>>,
    colored_text: Option<ColoredText>,
}

impl Resource {
    fn from_response(ctx: &egui::Context, response: ehttp::Response) -> Self {
        let content_type = response.content_type().unwrap_or_default();
        if content_type.starts_with("image/") {
            ctx.include_bytes(response.url.clone(), response.bytes.clone());
            let image = Image::from_uri(response.url.clone());
            trace!("Image URL: {}", response.url);

            Self {
                response,
                text: None,
                colored_text: None,
                image: Some(image),
            }
        } else {
            let text = response.text();
            let colored_text = text.and_then(|text| syntax_highlighting(ctx, &response, text));
            let text = text.map(|text| text.to_owned());

            Self {
                response,
                text,
                colored_text,
                image: None,
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct HttpApp {
    title: String,
    title_bar: bool,
    collapsible: bool,
    resizable: bool,
    constrain: bool,
    scroll2: Vec2b,
    anchored: bool,
    anchor: egui::Align2,
    anchor_offset: egui::Vec2,

    url: String,
    #[cfg_attr(feature = "serde", serde(skip))]
    promise: Option<Promise<ehttp::Result<Resource>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    wallpaper_promise: Option<Promise<Result<bool, String>>>,
}

impl Default for HttpApp {
    fn default() -> Self {
        Self {
            title: "🗖 Window Options".to_owned(),
            title_bar: false,
            collapsible: false,
            resizable: false,
            constrain: false,
            scroll2: Vec2b::TRUE,
            anchored: true,
            anchor: egui::Align2::CENTER_TOP,
            anchor_offset: egui::Vec2::ZERO,

            url: "https://raw.githubusercontent.com/emilk/egui/master/README.md".to_owned(),
            promise: Default::default(),
            wallpaper_promise: Default::default(),
        }
    }
}

impl crate::Demo for HttpApp {
    fn name(&self) -> &'static str {
        "🌐 HTTP"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        use crate::View as _;
        let screen_size = ctx.screen_rect().size();
        // trace!("screen size: {:?}", screen_size);

        let mut window = egui::Window::new(&self.title)
            .default_width(screen_size.x)
            .default_height(screen_size.y)
            .id(egui::Id::new("demo_window_options")) // required since we change the title
            .resizable(self.resizable)
            .constrain(self.constrain)
            .collapsible(self.collapsible)
            .title_bar(self.title_bar)
            .scroll(self.scroll2);
        if self.anchored {
            window = window.anchor(self.anchor, self.anchor_offset);
        }
        window.show(ctx, |ui| self.ui(ui));
    }
}


impl crate::View for HttpApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let prev_url = self.url.clone();
        let trigger_fetch = ui_url(ui, &mut self.url);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("HTTP requests made using ");
            ui.hyperlink_to("ehttp", "https://www.github.com/emilk/ehttp");
            ui.label(".");
        });

        if trigger_fetch {
            let ctx = ui.ctx().clone();
            let (sender, promise) = Promise::new();
            let request = ehttp::Request::get(&self.url);
            ehttp::fetch(request, move |response| {
                ctx.forget_image(&prev_url);
                ctx.request_repaint(); // wake up UI thread
                let resource = response.map(|response| Resource::from_response(&ctx, response));
                sender.send(resource);
            });
            self.promise = Some(promise);
        }

        ui.separator();

        if let Some(promise) = &self.promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(resource) => {
                        ui_resource(ui, resource, &mut self.wallpaper_promise);
                    }
                    Err(error) => {
                        // This should only happen if the fetch API isn't available or something similar.
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            if error.is_empty() { "Error" } else { error },
                        );
                    }
                }
            } else {
                ui.spinner();
            }
        }

        // Handle wallpaper promise results
        if let Some(wallpaper_promise) = &self.wallpaper_promise {
            if let Some(result) = wallpaper_promise.ready() {
                match result {
                    Ok(success) => {
                        if *success {
                            ui.colored_label(egui::Color32::GREEN, "✓ Wallpaper set successfully!");
                        } else {
                            ui.colored_label(ui.visuals().error_fg_color, "✗ Failed to set wallpaper");
                        }
                    }
                    Err(error) => {
                        ui.colored_label(ui.visuals().error_fg_color, format!("✗ Error: {}", error));
                    }
                }
                // Clear the promise after showing the result for 3 seconds
                ui.ctx().request_repaint_after(std::time::Duration::from_secs(3));
                self.wallpaper_promise = None;
            } else {
                // Show progress while promise is pending
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Setting wallpaper... (this may take a few seconds)");
                });
                // Keep requesting repaints to update the spinner
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }
}


fn ui_url(ui: &mut egui::Ui, url: &mut String) -> bool {
    let mut trigger_fetch = false;
    #[cfg(target_os = "android")]
    ui.add_space(40.0);
    
    ui.horizontal(|ui| {
        if ui.button("Exit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });

    ui.horizontal(|ui| {
        ui.label("URL:");
        trigger_fetch |= ui
            .add(egui::TextEdit::singleline(url).desired_width(f32::INFINITY))
            .lost_focus();
    });

    ui.horizontal(|ui| {
        if ui.button("Fetch Bing Daily Image").clicked() {
            *url = "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1".to_owned();
            trigger_fetch = true;
        }
        if ui.button("Random image").clicked() {
            let seed = ui.input(|i| i.time);
            let side = 640;
            *url = format!("https://picsum.photos/seed/{seed}/{side}");
            trigger_fetch = true;
        }
    });

    trigger_fetch
}

fn ui_resource(ui: &mut egui::Ui, resource: &Resource, wallpaper_promise: &mut Option<Promise<Result<bool, String>>>) {
    let Resource {
        response,
        text,
        image,
        colored_text,
    } = resource;

    ui.monospace(format!("url:          {}", response.url));
    ui.monospace(format!(
        "status:       {} ({})",
        response.status, response.status_text
    ));
    ui.monospace(format!(
        "content-type: {}",
        response.content_type().unwrap_or_default()
    ));
    ui.monospace(format!(
        "size:         {:.1} kB",
        response.bytes.len() as f32 / 1000.0
    ));

    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            egui::CollapsingHeader::new("Response headers")
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new("response_headers")
                        .spacing(egui::vec2(ui.spacing().item_spacing.x * 2.0, 0.0))
                        .show(ui, |ui| {
                            for (k, v) in &response.headers {
                                ui.label(k);
                                ui.label(v);
                                ui.end_row();
                            }
                        })
                });

            ui.separator();

            if let Some(text) = &text {
                let tooltip = "Click to copy the response body";
                if ui.button("📋").on_hover_text(tooltip).clicked() {
                    ui.ctx().copy_text(text.clone());
                }
                ui.separator();
            }

            if let Some(image) = image {
                ui.add(image.clone());
                
                // Add wallpaper button for images
                if ui.button("Set this Wallpaper").clicked() {
                    #[cfg(target_os = "android")]
                    {
                        if !response.bytes.is_empty() {
                            let image_data = response.bytes.clone();
                            info!("Starting wallpaper setting operation...");
                            let temp_path = "/sdcard/Download/bingtray_temp_wallpaper.jpg";
                            // Immediately save the file and send initial progress
                            let save_result = std::fs::write(&temp_path, &image_data);
                            let temp_path_clone = temp_path.to_string();
                            std::thread::spawn(move ||{
                                let ret = crate::set_wallpaper_from_path(&temp_path_clone);
                                info!("Android set_wallpaper_from_path: {:?}", ret);
                            });
                            info!("Wallpaper set from path: {}", temp_path.to_string());
                            
                            // ui.colored_label(egui::Color32::GREEN, "Wallpaper set successfully!");
                            // // Give UI thread priority to update
                            // trace!("Requesting repaint after setting wallpaper");
                            // Final yield to give UI thread priority after operation
                            std::thread::yield_now();
                            // ui.ctx().request_repaint();

                            // // sleep 3 seconds
                            // std::thread::sleep(std::time::Duration::from_secs(3));
                            // // exit the app
                            // ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        
                        } else {
                            error!("No image data available");
                        }
                    }
                    #[cfg(not(target_os = "android"))]
                    {
                        warn!("Wallpaper setting is only available on Android");
                    }
                }
                
                // Show wallpaper promise status
                if let Some(wp_promise) = wallpaper_promise {
                    if wp_promise.ready().is_none() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Setting wallpaper...");
                        });
                        // Keep the UI updated while processing
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                    }
                }
            } else if let Some(colored_text) = colored_text {
                colored_text.ui(ui);
            } else if let Some(text) = &text {
                ui.add(egui::Label::new(text).selectable(true));
            } else {
                ui.monospace("[binary]");
            }
        });
    }

fn syntax_highlighting(
    ctx: &egui::Context,
    response: &ehttp::Response,
    text: &str,
) -> Option<ColoredText> {
    let extension_and_rest: Vec<&str> = response.url.rsplitn(2, '.').collect();
    let extension = extension_and_rest.first()?;
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(&ctx.style());
    Some(ColoredText(egui_extras::syntax_highlighting::highlight(
        ctx,
        &ctx.style(),
        &theme,
        text,
        extension,
    )))
}

struct ColoredText(egui::text::LayoutJob);

impl ColoredText {
    pub fn ui(&self, ui: &mut egui::Ui) {
        let mut job = self.0.clone();
        job.wrap.max_width = ui.available_width();
        let galley = ui.fonts(|f| f.layout_job(job));
        ui.add(egui::Label::new(galley).selectable(true));
    }
}