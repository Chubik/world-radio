use tray_icon::menu::{Menu, MenuId, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct Tray {
    pub _icon: TrayIcon,
    pub shuffle_all: MenuId,
    pub shuffle_fav: MenuId,
    pub toggle: MenuId,
    pub quit: MenuId,
}

pub fn build() -> anyhow::Result<Tray> {
    let menu = Menu::new();
    let shuffle_all = MenuItem::new("Shuffle", true, None);
    let shuffle_fav = MenuItem::new("Shuffle favorites", true, None);
    let toggle = MenuItem::new("Play / Stop", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append(&shuffle_all)?;
    menu.append(&shuffle_fav)?;
    menu.append(&toggle)?;
    menu.append(&quit)?;

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("World Radio Mini")
        .with_title("WR")
        .build()?;

    Ok(Tray {
        _icon: icon,
        shuffle_all: shuffle_all.id().clone(),
        shuffle_fav: shuffle_fav.id().clone(),
        toggle: toggle.id().clone(),
        quit: quit.id().clone(),
    })
}
