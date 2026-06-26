use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct Tray {
    pub _icon: TrayIcon,
}

pub fn build() -> anyhow::Result<Tray> {
    let icon = TrayIconBuilder::new()
        .with_tooltip("World Radio Mini")
        .with_title("WR")
        .build()?;

    Ok(Tray { _icon: icon })
}
