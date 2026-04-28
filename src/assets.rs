use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "ui/"]
pub struct Assets;
