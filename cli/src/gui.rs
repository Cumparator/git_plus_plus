use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2, FontId};
use std::collections::{HashMap, HashSet};
use std::fs;

// Импортируем типы из ядра
use gpp_core::types::{Node, NodeId};

// --- КОНСТАНТЫ ОТРИСОВКИ ---
const NODE_RADIUS: f32 = 10.0;   // Радиус узла
const Y_SPACING: f32 = 80.0;     // Вертикальный отступ между поколениями
const BRANCH_STEP: f32 = 180.0;  // Горизонтальный отступ ветки (достаточно широкий для текста)
const TREE_GAP: f32 = 150.0;     // Отступ между независимыми деревьями
const PADDING: f32 = 60.0;       // Отступ от краев окна
const FONT_SIZE: f32 = 14.0;     // Размер шрифта
const MAX_MSG_LEN: usize = 10;   // Максимальная длина сообщения перед обрезкой

pub fn run_gui() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Git++ Visualizer",
        options,
        Box::new(|_cc| Box::new(GppApp::new())),
    )
}

/// Структура для отрисовки ноды.
/// Содержит уже вычисленные координаты и цвет.
struct VisualNode {
    id: NodeId,
    display_message: String, // Обрезанное сообщение для вывода на граф
    author: String,
    row: usize,       // Y - поколение
    x: f32,           // X - точная позиция
    color: Color32,
}

// --- ПАЛИТРА И СМЕШИВАНИЕ (CMY) ---
struct Palette {
    /// Карта: Имя ремоута -> Базовый цвет
    remote_colors: HashMap<String, Color32>,
    /// Пул цветов (CMY приоритет для субтрактивного смешивания)
    pool: Vec<[u8; 3]>, 
}

impl Palette {
    fn new() -> Self {
        Self {
            remote_colors: HashMap::new(),
            // Порядок выдачи цветов: Cyan, Magenta, Yellow. 
            // Cyan=[0,255,255], Magenta=[255,0,255], Yellow=[255,255,0]
            pool: vec![
                [0, 255, 255],   // 1. Cyan (Голубой) -> Origin
                [255, 0, 255],   // 2. Magenta (Малиновый)
                [255, 255, 0],   // 3. Yellow (Желтый)
                [255, 128, 0],   // 4. Orange
                [0, 255, 128],   // 5. Spring Green
                [128, 0, 255],   // 6. Purple
            ],
        }
    }

    /// Назначает цвета всем встреченным ремоутам
    fn assign_colors(&mut self, nodes: &HashMap<NodeId, Node>) {
        let mut all_remotes: HashSet<String> = HashSet::new();
        for node in nodes.values() {
            for remote in &node.remotes {
                all_remotes.insert(remote.name.clone());
            }
        }

        // Сортируем для детерминизма
        let mut sorted_remotes: Vec<String> = all_remotes.into_iter().collect();
        sorted_remotes.sort();

        // Приоритет: origin всегда должен быть первым (Cyan)
        if let Some(pos) = sorted_remotes.iter().position(|r| r == "origin") {
            let val = sorted_remotes.remove(pos);
            sorted_remotes.insert(0, val);
        }

        self.remote_colors.clear();
        for (i, name) in sorted_remotes.iter().enumerate() {
            let raw_color = self.pool[i % self.pool.len()];
            self.remote_colors.insert(name.clone(), Color32::from_rgb(raw_color[0], raw_color[1], raw_color[2]));
        }
    }

    /// Магическая формула смешивания (Multiply / Умножение)
    fn get_mixed_color(&self, node_remotes: &HashSet<gpp_core::types::RemoteRef>) -> Color32 {
        if node_remotes.is_empty() {
            return Color32::from_gray(80); // Серый для локальных нод
        }

        // Начинаем с белого (255, 255, 255)
        let mut r_acc: u16 = 255;
        let mut g_acc: u16 = 255;
        let mut b_acc: u16 = 255;

        for remote in node_remotes {
            if let Some(color) = self.remote_colors.get(&remote.name) {
                // Formula: (Base * Layer) / 255
                r_acc = (r_acc * color.r() as u16) / 255;
                g_acc = (g_acc * color.g() as u16) / 255;
                b_acc = (b_acc * color.b() as u16) / 255;
            }
        }

        Color32::from_rgb(r_acc as u8, g_acc as u8, b_acc as u8)
    }
}

struct GppApp {
    raw_nodes: HashMap<NodeId, Node>,
    visual_nodes: HashMap<NodeId, VisualNode>,
    connections: Vec<(NodeId, NodeId)>,
    error_msg: Option<String>,
    palette: Palette, 
    
    // Размеры холста
    max_row: usize,
    total_width: f32,
}

impl GppApp {
    fn new() -> Self {
        let mut app = Self {
            raw_nodes: HashMap::new(),
            visual_nodes: HashMap::new(),
            connections: Vec::new(),
            error_msg: None,
            palette: Palette::new(),
            max_row: 0,
            total_width: 0.0,
        };
        
        if let Err(e) = app.load_graph() {
            app.error_msg = Some(format!("Failed to load repository: {}", e));
        } else {
            app.calculate_layout();
        }
        
        app
    }

    fn load_graph(&mut self) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;
        let db_path = current_dir.join(".gitpp").join("graph.json");
        
        if !db_path.exists() {
            return Err(anyhow::anyhow!("Repo not found at {:?}. Run 'gpp init' first.", db_path));
        }

        let content = fs::read_to_string(db_path)?;
        self.raw_nodes = serde_json::from_str(&content)?;
        
        // 1. Раздаем цвета ремоутам
        self.palette.assign_colors(&self.raw_nodes);
        
        Ok(())
    }

    fn calculate_layout(&mut self) {
        self.visual_nodes.clear();
        self.connections.clear();

        if self.raw_nodes.is_empty() { return; }

        // Находим корни
        let mut roots: Vec<NodeId> = self.raw_nodes.values()
            .filter(|n| n.parents.is_empty())
            .map(|n| n.id.clone())
            .collect();
        
        roots.sort_by(|a, b| a.0.cmp(&b.0));

        let mut visited = HashSet::new();
        let mut current_global_x = 0.0;

        for root in roots {
            let tree_width_used = self.layout_tree(
                &root, 
                0,                  // row
                current_global_x,   // base_x
                0,                  // depth
                &mut visited
            );

            current_global_x += tree_width_used + TREE_GAP;
        }
        
        self.max_row = self.visual_nodes.values().map(|n| n.row).max().unwrap_or(0);
        self.total_width = current_global_x;
    }

    fn layout_tree(
        &mut self, 
        node_id: &NodeId, 
        row: usize, 
        base_x: f32,      
        depth: usize,     
        visited: &mut HashSet<NodeId>,
    ) -> f32 {
        if visited.contains(node_id) { return 0.0; }
        visited.insert(node_id.clone());

        let node = match self.raw_nodes.get(node_id) {
            Some(n) => n,
            None => return 0.0,
        };

        // --- ЦВЕТ ---
        let node_color = self.palette.get_mixed_color(&node.remotes);

        // --- ПОЗИЦИЯ ---
        let node_x_offset = depth as f32 * BRANCH_STEP;
        let absolute_x = base_x + node_x_offset;

        // --- ТЕКСТ (ОБРЕЗКА) ---
        let full_msg = node.message.lines().next().unwrap_or("").to_string();
        let display_msg = if full_msg.chars().count() > MAX_MSG_LEN {
            let truncated: String = full_msg.chars().take(MAX_MSG_LEN).collect();
            format!("{}...", truncated)
        } else {
            full_msg
        };

        // Считаем ширину обрезанного текста
        let text_width = estimate_text_width(&display_msg);
        
        // Общая ширина ноды (для сдвига следующего дерева)
        let node_width_usage = node_x_offset + (NODE_RADIUS * 2.0) + 10.0 + text_width;

        let v_node = VisualNode {
            id: node_id.clone(),
            display_message: display_msg,
            author: node.author.name.clone(),
            row,
            x: absolute_x,
            color: node_color,
        };
        self.visual_nodes.insert(node_id.clone(), v_node);

        // --- ДЕТИ ---
        let mut children_vec: Vec<NodeId> = node.children.iter().cloned().collect();
        children_vec.sort_by(|a, b| a.0.cmp(&b.0));

        let mut max_width_in_subtree = node_width_usage;

        for (i, child_id) in children_vec.iter().enumerate() {
            self.connections.push((node_id.clone(), child_id.clone()));

            // ВЕЕРНОЕ РАСПОЛОЖЕНИЕ:
            // Каждый следующий ребенок уходит глубже вправо (depth + i), 
            // чтобы не накладываться на предыдущего.
            let next_depth = depth + i;
            
            let child_width = self.layout_tree(
                child_id, 
                row + 1, 
                base_x, 
                next_depth, 
                visited
            );
            
            if child_width > max_width_in_subtree {
                max_width_in_subtree = child_width;
            }
        }

        max_width_in_subtree
    }
}

// Оценка ширины текста в пикселях
fn estimate_text_width(msg: &str) -> f32 {
    let chars = msg.chars().count() + 8; // + место под хеш
    chars as f32 * (FONT_SIZE * 0.6) 
}

impl eframe::App for GppApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        // --- ПАНЕЛЬ ЛЕГЕНДЫ ---
        egui::SidePanel::left("legend_panel")
            .resizable(false)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.heading("Remotes");
                ui.separator();

                let mut sorted_legend: Vec<_> = self.palette.remote_colors.iter().collect();
                sorted_legend.sort_by_key(|(k, _)| *k);

                if sorted_legend.is_empty() {
                     ui.label(egui::RichText::new("Local only").italics());
                } else {
                    for (name, color) in sorted_legend {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 6.0, *color);
                            ui.label(name);
                        });
                    }
                }

                ui.add_space(20.0);
                ui.heading("Mixing Logic");
                ui.separator();
                ui.label("Multiply Blending:");
                
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Cyan").color(Color32::from_rgb(0, 255, 255)));
                    ui.label("+");
                    ui.label(egui::RichText::new("Yellow").color(Color32::YELLOW));
                    ui.label("=");
                    ui.label(egui::RichText::new("Green").color(Color32::GREEN));
                });
                 ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Cyan").color(Color32::from_rgb(0, 255, 255)));
                    ui.label("+");
                    ui.label(egui::RichText::new("Magenta").color(Color32::from_rgb(255, 0, 255)));
                    ui.label("=");
                    ui.label(egui::RichText::new("Blue").color(Color32::from_rgb(50, 50, 255)));
                });
            });

        // --- ГРАФ ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Git++ Forest");
            
            if let Some(err) = &self.error_msg {
                ui.colored_label(Color32::RED, err);
                if ui.button("Retry Load").clicked() {
                    self.error_msg = None;
                    if let Ok(_) = self.load_graph() {
                        self.calculate_layout();
                    }
                }
                return;
            }

            egui::ScrollArea::both().show(ui, |ui| {
                let width = self.total_width + PADDING * 2.0;
                let height = (self.max_row + 2) as f32 * Y_SPACING + PADDING * 2.0;
                
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(width.max(ui.available_width()), height.max(ui.available_height())), 
                    egui::Sense::hover()
                );
                
                let to_screen = |row: usize, x: f32| -> Pos2 {
                    let start = response.rect.min;
                    Pos2::new(
                        start.x + PADDING + x,
                        start.y + PADDING + row as f32 * Y_SPACING,
                    )
                };

                // 1. ЛИНИИ
                for (start_id, end_id) in &self.connections {
                    if let (Some(start), Some(end)) = (self.visual_nodes.get(start_id), self.visual_nodes.get(end_id)) {
                        let p1 = to_screen(start.row, start.x);
                        let p2 = to_screen(end.row, end.x);
                        
                        let control_scale = (p2.y - p1.y) * 0.6;
                        let c1 = Pos2::new(p1.x, p1.y + control_scale);
                        let c2 = Pos2::new(p2.x, p2.y - control_scale);
                        
                        let line_color = start.color.gamma_multiply(0.4);
                        let stroke = Stroke::new(2.0, line_color);

                        painter.add(eframe::epaint::CubicBezierShape::from_points_stroke(
                            [p1, c1, c2, p2],
                            false,
                            Color32::TRANSPARENT,
                            stroke,
                        ));
                    }
                }

                // 2. НОДЫ
                for node in self.visual_nodes.values() {
                    let center = to_screen(node.row, node.x);
                    
                    painter.circle_filled(center, NODE_RADIUS, node.color);
                    painter.circle_stroke(center, NODE_RADIUS, Stroke::new(1.5, Color32::WHITE));

                    let text_pos = center + Vec2::new(NODE_RADIUS + 8.0, 0.0);
                    painter.text(
                        text_pos,
                        egui::Align2::LEFT_CENTER,
                        // Используем обрезанное сообщение
                        format!("{} ({})", node.display_message, &node.id.0[..6]),
                        FontId::proportional(FONT_SIZE),
                        Color32::LIGHT_GRAY,
                    );

                    // TOOLTIP (ПОЛНАЯ ИНФОРМАЦИЯ)
                    let node_rect = Rect::from_center_size(center, Vec2::splat(NODE_RADIUS * 2.0));
                    if let Some(pointer_pos) = response.hover_pos() {
                        if node_rect.contains(pointer_pos) {
                            egui::show_tooltip(ctx, response.id, |ui| {
                                ui.strong("Node Details");
                                ui.label(format!("ID: {}", node.id.0));
                                ui.label(format!("Author: {}", node.author));
                                
                                if let Some(raw) = self.raw_nodes.get(&node.id) {
                                    let remotes: Vec<_> = raw.remotes.iter().map(|r| r.name.as_str()).collect();
                                    ui.colored_label(Color32::LIGHT_BLUE, format!("Remotes: {:?}", remotes));
                                    ui.separator();
                                    // Показываем полное сообщение здесь
                                    ui.label(format!("Message:\n{}", raw.message));
                                }
                            });
                        }
                    }
                }
            });
        });
    }
}