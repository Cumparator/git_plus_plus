use eframe::egui;

pub fn run_gui() -> Result<(), eframe::Error> {
    // 1. ИСПОЛЬЗУЕМ НОВЫЙ СПОСОБ (Viewport) - чтобы ушла ошибка "no field initial_window_size"
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Git++ GUI",
        options,
        // 2. ИСПОЛЬЗУЕМ СТАРЫЙ ВОЗВРАТ (Box без Ok) - чтобы ушла ошибка "expected Box, found Result"
        Box::new(|_cc| Box::new(GppApp::default())),
    )
}

struct GppApp {
    commit_message: String,
    logs: Vec<String>,
}

impl Default for GppApp {
    fn default() -> Self {
        Self {
            commit_message: String::new(),
            logs: vec![
                "Initial commit (Hash: a1b2)".to_string(),
                "Fix login bug (Hash: c3d4)".to_string(),
                "Update documentation (Hash: e5f6)".to_string(),
            ],
        }
    }
}

impl eframe::App for GppApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Git++ Visual Interface");
            ui.separator();

            ui.collapsing("Create Commit", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Message:");
                    ui.text_edit_singleline(&mut self.commit_message);
                });

                if ui.button("Commit Changes").clicked() {
                    if !self.commit_message.is_empty() {
                        self.logs.insert(0, format!("{} (New!)", self.commit_message));
                        self.commit_message.clear();
                    }
                }
            });

            ui.add_space(20.0);
            ui.separator();

            ui.heading("History / Graph");
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, log) in self.logs.iter().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}.", i + 1));
                            ui.strong(log);
                        });
                    });
                }
            });
            
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.separator();
                ui.label("Powered by Git++ Core & egui");
            });
        });
    }
}