//! GTK-based tray backend using tray-icon crate

use anyhow::Result;
use egui_i18n::tr;
use std::sync::{Arc, Mutex};
use crossbeam_queue::SegQueue;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, MenuId},
    Icon, TrayIconBuilder, TrayIcon, TrayIconEvent,
};

use super::{TrayBackend, TrayExitAction};
use super::logic::TrayLogic;

pub struct GtkTrayBackend {
    logic: TrayLogic,
}

impl TrayBackend for GtkTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Early GTK availability check
        if !is_gtk_available() {
            return Err(anyhow::anyhow!("GTK tray manager not available"));
        }
        Ok(Self { logic })
    }

    fn run(mut self) -> Result<TrayExitAction> {
        log::info!("=== Starting GTK tray backend ===");

        // Create event loop
        let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

        // Get global event queues
        let tray_queue = super::super::TRAY_ICON_EVENTS.get()
            .expect("Tray event handlers not initialized");
        let menu_queue = super::super::MENU_EVENTS.get()
            .expect("Menu event handlers not initialized");

        let exit_action = Arc::new(Mutex::new(TrayExitAction::Quit));
        let exit_action_for_return = exit_action.clone();

        let mut tray_icon: Option<TrayIcon> = None;
        let mut menu_items: Option<MenuItems> = None;

        let tray_queue = tray_queue.clone();
        let menu_queue = menu_queue.clone();

        event_loop.run_return(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            while let Some(_tray_event) = tray_queue.pop() {
                // Handle tray icon events if needed
            }

            while let Some(menu_event) = menu_queue.pop() {
                if let Some(ref items) = menu_items {
                    if menu_event.id == items.show_app {
                        *exit_action_for_return.lock().unwrap() = TrayExitAction::OpenGui;
                        *control_flow = ControlFlow::Exit;
                        continue;
                    } else if menu_event.id == items.cache_dir {
                        let _ = self.logic.open_cache_directory();
                    } else if menu_event.id == items.next_market {
                        if let Ok(true) = self.logic.set_next_market_wallpaper() {
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.keep_current {
                        if self.logic.can_keep() {
                            let _ = self.logic.keep_current_image();
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.blacklist_current {
                        if self.logic.can_blacklist() {
                            let _ = self.logic.blacklist_current_image();
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.random_favorite {
                        if let Ok(true) = self.logic.set_kept_wallpaper() {
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.quit {
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }

            match event {
                Event::NewEvents(_) => {
                    if tray_icon.is_none() {
                        let icon = load_tray_icon();
                        let (menu, items) = create_tray_menu(&mut self.logic);

                        let new_tray_icon = TrayIconBuilder::new()
                            .with_menu(Box::new(menu))
                            .with_tooltip("BingTray")
                            .with_icon(icon)
                            .build()
                            .expect("Failed to build tray icon");

                        tray_icon = Some(new_tray_icon);
                        menu_items = Some(items);
                    }

                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                _ => {}
            }
        });

        Ok(*exit_action.lock().unwrap())
    }
}

fn is_gtk_available() -> bool {
    EventLoopBuilder::<UserEvent>::with_user_event()
        .build()
        .is_ok()
}

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}

struct MenuItems {
    show_app: MenuId,
    cache_dir: MenuId,
    next_market: MenuId,
    current_title: MenuId,
    keep_current: MenuId,
    blacklist_current: MenuId,
    random_favorite: MenuId,
    quit: MenuId,
}

fn load_tray_icon() -> Icon {
    let icon_bytes = include_bytes!("../../resources/logo.png");
    let image = image::load_from_memory(icon_bytes).expect("Failed to load icon");
    let rgba = image.to_rgba8();
    Icon::from_rgba(rgba.to_vec(), image.width(), image.height())
        .expect("Failed to create icon")
}

fn create_tray_menu(logic: &mut TrayLogic) -> (Menu, MenuItems) {
    let menu = Menu::new();

    let show_app = MenuItem::new(format!("{}", tr!("tray-show-app")), true, None);
    let cache_dir = MenuItem::new(format!("{}", tr!("tray-cache-dir")), true, None);

    let wallpaper_status = logic.get_wallpaper_page_status();
    let has_next = logic.has_next_available();
    let next_market = MenuItem::new(
        format!("{}\n{}", tr!("tray-next-market"), wallpaper_status),
        has_next,
        None
    );

    let current_title_text = logic.get_current_image_title();
    let current_title_display = if !current_title_text.is_empty() {
        format!("📷 {}", current_title_text)
    } else {
        format!("📷 {}", tr!("tray-no-wallpaper"))
    };
    let current_title_item = MenuItem::new(current_title_display, false, None);

    let can_keep = logic.can_keep();
    let keep_text = if can_keep {
        format!("{}", tr!("tray-keep-with-title", { title: current_title_text.clone() }))
    } else {
        format!("{}", tr!("tray-keep-current"))
    };
    let keep_current = MenuItem::new(keep_text, can_keep, None);

    let can_blacklist = logic.can_blacklist();
    let blacklist_text = if can_blacklist {
        format!("{}", tr!("tray-blacklist-with-title", { title: current_title_text.clone() }))
    } else {
        format!("{}", tr!("tray-blacklist-current"))
    };
    let blacklist_current = MenuItem::new(blacklist_text, can_blacklist, None);

    let has_kept = logic.has_kept_wallpapers();
    let random_favorite = MenuItem::new(
        format!("{}", tr!("tray-random-favorite")),
        has_kept,
        None,
    );

    let quit = MenuItem::new(format!("{}", tr!("tray-quit")), true, None);

    let menu_items = MenuItems {
        show_app: show_app.id().clone(),
        cache_dir: cache_dir.id().clone(),
        next_market: next_market.id().clone(),
        current_title: current_title_item.id().clone(),
        keep_current: keep_current.id().clone(),
        blacklist_current: blacklist_current.id().clone(),
        random_favorite: random_favorite.id().clone(),
        quit: quit.id().clone(),
    };

    menu.append(&show_app).ok();
    menu.append(&MenuItem::new("", false, None)).ok();
    menu.append(&cache_dir).ok();
    menu.append(&next_market).ok();
    menu.append(&current_title_item).ok();
    menu.append(&keep_current).ok();
    menu.append(&blacklist_current).ok();
    menu.append(&random_favorite).ok();
    menu.append(&MenuItem::new("", false, None)).ok();
    menu.append(&quit).ok();

    (menu, menu_items)
}

fn update_tray_menu(
    tray_icon: &TrayIcon,
    logic: &mut TrayLogic,
    menu_items: &mut MenuItems,
) {
    let (new_menu, new_menu_items) = create_tray_menu(logic);
    *menu_items = new_menu_items;
    tray_icon.set_menu(Some(Box::new(new_menu)));
}
