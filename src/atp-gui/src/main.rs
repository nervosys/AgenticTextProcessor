//! ATP GUI — Graphical User Interface for the Agentic Text Processor.
//!
//! Provides a desktop GUI optimized for human use with:
//! - Visual search with highlighted results
//! - Transform preview with diff display
//! - Pipeline builder with drag-and-drop
//! - File browser integration

mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("ATP — Agentic Text Processor"),
        ..Default::default()
    };

    eframe::run_native(
        "ATP — Agentic Text Processor",
        options,
        Box::new(|cc| Ok(Box::new(app::AtpGuiApp::new(cc)))),
    )
}
