// GROVE — Graph-Rendered Ontology for Visual Exploration
// A semantic mind-map tool for rapid learning of complex systems.

mod app;
mod graph;
mod parser;
mod vault;

use app::GroveApp;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let vault_path = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("GROVE"),
        ..Default::default()
    };

    eframe::run_native(
        "GROVE",
        native_options,
        Box::new(move |cc| Ok(Box::new(GroveApp::new(cc, vault_path)))),
    )
}
