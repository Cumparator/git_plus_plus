use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2, FontId};
use std::collections::{HashMap, HashSet};
use std::fs;

// Импортируем типы из ядра
use gpp_core::types::{Node, NodeId};

// --- КОНСТАНТЫ ОТРИСОВКИ ---
const NODE_RADIUS: f32 = 9.0;
const Y_SPACING: f32 = 60.0;     // Шаг по вертикали (между родителями и детьми)
const BRANCH_STEP: f32 = 30.0;   // Шаг отступа при ветвлении внутри одного дерева
const TREE_GAP: f32 = 80.0;      // Минимальное расстояние между независимыми деревьями
const PADDING: f32 = 40.0;       // Отступ от краев окна
const FONT_SIZE: f32 = 14.0;     // Размер шрифта

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

struct VisualNode {
    id: NodeId,
    message: String,
    author: String,
    row: usize,       // Y - остается дискретным (поколения)
    x: f32,           // X - теперь float (точная позиция в пикселях)
    color: Color32,
}

struct GppApp {
    raw_nodes: HashMap<NodeId, Node>,
    visual_nodes: HashMap<NodeId, VisualNode>,
    connections: Vec<(NodeId, NodeId)>,
    error_msg: Option<String>,
    
    // Размеры холста для скролла
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
        Ok(())
    }

    fn calculate_layout(&mut self) {
        self.visual_nodes.clear();
        self.connections.clear();

        if self.raw_nodes.is_empty() { return; }

        // 1. Находим корни
        let mut roots: Vec<NodeId> = self.raw_nodes.values()
            .filter(|n| n.parents.is_empty())
            .map(|n| n.id.clone())
            .collect();
        
        // Сортируем для стабильности (чтобы деревья не скакали при обновлении)
        roots.sort_by(|a, b| a.0.cmp(&b.0));

        let mut visited = HashSet::new();
        
        // Курсор глобальной позиции X. Каждое следующее дерево начнется отсюда.
        let mut current_global_x = 0.0;

        for root in roots {
            let tree_color = generate_vibrant_color(&root.0);
            
            // Запускаем рекурсивный лайаут для ОДНОГО дерева.
            // Он вернет ширину (в пикселях), которую заняло это дерево (включая текст).
            let tree_width_used = self.layout_tree(
                &root, 
                0,                  // row
                current_global_x,   // base_x (где начинается ствол дерева)
                0,                  // depth (отступ ветвления)
                &mut visited, 
                tree_color
            );

            // Сдвигаем курсор для следующего дерева: 
            // Ширина текущего дерева + безопасный зазор
            current_global_x += tree_width_used + TREE_GAP;
        }
        
        self.max_row = self.visual_nodes.values().map(|n| n.row).max().unwrap_or(0);
        self.total_width = current_global_x;
    }

    /// Рекурсивная функция. Возвращает МАКСИМАЛЬНУЮ ширину (от base_x),
    /// до которой дотянулось это поддерево (учитывая длину текста).
    fn layout_tree(
        &mut self, 
        node_id: &NodeId, 
        row: usize, 
        base_x: f32,      // Глобальное начало этого дерева
        depth: usize,     // Глубина ветвления (0, 1, 2...) внутри дерева
        visited: &mut HashSet<NodeId>,
        color: Color32,
    ) -> f32 {
        if visited.contains(node_id) { return 0.0; }
        visited.insert(node_id.clone());

        let node = match self.raw_nodes.get(node_id) {
            Some(n) => n,
            None => return 0.0,
        };

        // 1. Вычисляем позицию X для этой конкретной ноды
        let node_x_offset = depth as f32 * BRANCH_STEP;
        let absolute_x = base_x + node_x_offset;

        // 2. Оцениваем длину текста, чтобы знать, сколько места нода занимает справа
        let display_msg = node.message.lines().next().unwrap_or("").to_string();
        let text_width = estimate_text_width(&display_msg, &node.id.0);
        
        // Ширина, занятая этой конкретной нодой (относительно base_x)
        // offset + радиус + отступ текста + ширина текста
        let node_width_usage = node_x_offset + (NODE_RADIUS * 2.0) + 10.0 + text_width;

        // Сохраняем ноду
        let v_node = VisualNode {
            id: node_id.clone(),
            message: display_msg,
            author: node.author.name.clone(),
            row,
            x: absolute_x,
            color,
        };
        self.visual_nodes.insert(node_id.clone(), v_node);

        // 3. Обрабатываем детей
        let mut children_vec: Vec<NodeId> = node.children.iter().cloned().collect();
        children_vec.sort_by(|a, b| a.0.cmp(&b.0));

        // Нам нужно найти максимальную ширину среди всех детей и самой ноды
        let mut max_width_in_subtree = node_width_usage;

        for (i, child_id) in children_vec.iter().enumerate() {
            self.connections.push((node_id.clone(), child_id.clone()));

            // Если ребенок первый - он продолжает ствол (depth),
            // Если второй и далее - это ветвление (depth + 1)
            let next_depth = if i == 0 { depth } else { depth + 1 };
            
            let child_width = self.layout_tree(
                child_id, 
                row + 1, 
                base_x, 
                next_depth, 
                visited, 
                color
            );
            
            if child_width > max_width_in_subtree {
                max_width_in_subtree = child_width;
            }
        }

        max_width_in_subtree
    }
}

// Оценка ширины текста в пикселях (эвристика)
fn estimate_text_width(msg: &str, hash: &str) -> f32 {
    // Примерно 8 пикселей на символ для шрифта 14.0 + длина хеша
    let chars = msg.chars().count() + 8; // +8 символов на хеш (a1b2...) и скобки
    chars as f32 * (FONT_SIZE * 0.6) 
}

fn generate_vibrant_color(hash_seed: &str) -> Color32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    hash_seed.hash(&mut hasher);
    let hash = hasher.finish();

    let hue = (hash % 360) as f32;
    let saturation = 0.8; 
    let value = 0.9;      

    let c = value * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - c;

    let (r, g, b) = if hue < 60.0 { (c, x, 0.0) }
    else if hue < 120.0 { (x, c, 0.0) }
    else if hue < 180.0 { (0.0, c, x) }
    else if hue < 240.0 { (0.0, x, c) }
    else if hue < 300.0 { (x, 0.0, c) }
    else { (c, 0.0, x) };

    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

impl eframe::App for GppApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Git++ Forest Graph");
            
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
                // Вычисляем размер холста на основе реальных данных
                let width = self.total_width + PADDING * 2.0;
                let height = (self.max_row + 2) as f32 * Y_SPACING + PADDING * 2.0;
                
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(width.max(ui.available_width()), height.max(ui.available_height())), 
                    egui::Sense::hover()
                );
                
                // Функция трансформации: Row -> Y, X -> X
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
                    
                    // Рисуем точку
                    painter.circle_filled(center, NODE_RADIUS, node.color);
                    painter.circle_stroke(center, NODE_RADIUS, Stroke::new(1.5, Color32::WHITE));

                    // Текст
                    let text_pos = center + Vec2::new(NODE_RADIUS + 8.0, 0.0);
                    painter.text(
                        text_pos,
                        egui::Align2::LEFT_CENTER,
                        format!("{} ({})", node.message, &node.id.0[..6]),
                        FontId::proportional(FONT_SIZE),
                        Color32::LIGHT_GRAY,
                    );

                    // Tooltip (при наведении)
                    let node_rect = Rect::from_center_size(center, Vec2::splat(NODE_RADIUS * 2.0));
                    // Также добавляем rect текста, чтобы тултип работал и на тексте
                    // (для простоты тут только на кружочке)
                    
                    if let Some(pointer_pos) = response.hover_pos() {
                        if node_rect.contains(pointer_pos) {
                            egui::show_tooltip(ctx, response.id, |ui| {
                                ui.strong("Commit Details");
                                ui.label(format!("ID: {}", node.id.0));
                                ui.label(format!("Author: {}", node.author));
                                let full_msg = self.raw_nodes.get(&node.id).map(|n| n.message.as_str()).unwrap_or("?");
                                ui.label(format!("Message: \n{}", full_msg));
                            });
                        }
                    }
                }
            });
        });
    }
}