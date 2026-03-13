// GROVE — Graph-Rendered Ontology for Visual Exploration
// Main application UI, layout engine, and rendering.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use egui::{Color32, Pos2, Vec2};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::graph::KnowledgeGraph;
use crate::parser;
use crate::vault::Vault;

/// Color palette for verb groups (edges).
const VERB_COLORS: &[Color32] = &[
    Color32::from_rgb(99, 179, 237),   // blue
    Color32::from_rgb(154, 230, 180),  // green
    Color32::from_rgb(246, 173, 85),   // orange
    Color32::from_rgb(237, 100, 166),  // pink
    Color32::from_rgb(183, 148, 244),  // purple
    Color32::from_rgb(252, 211, 77),   // yellow
    Color32::from_rgb(129, 230, 217),  // teal
    Color32::from_rgb(245, 101, 101),  // red
    Color32::from_rgb(160, 174, 192),  // gray
    Color32::from_rgb(246, 224, 94),   // lime
];

/// Color palette for tags (nodes) — distinct from verb palette.
const TAG_COLORS: &[Color32] = &[
    Color32::from_rgb(66, 135, 245),   // royal blue
    Color32::from_rgb(52, 199, 89),    // green
    Color32::from_rgb(255, 149, 0),    // orange
    Color32::from_rgb(175, 82, 222),   // purple
    Color32::from_rgb(255, 59, 48),    // red
    Color32::from_rgb(0, 199, 190),    // teal
    Color32::from_rgb(255, 204, 0),    // gold
    Color32::from_rgb(88, 86, 214),    // indigo
    Color32::from_rgb(255, 105, 180),  // hot pink
    Color32::from_rgb(50, 205, 50),    // lime green
    Color32::from_rgb(210, 105, 30),   // chocolate
    Color32::from_rgb(0, 191, 255),    // deep sky blue
];

/// Node position and size for rendering.
#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub pos: Pos2,
    pub radius: f32,
    pub depth: usize,
}

/// Main application state.
pub struct GroveApp {
    vault: Vault,
    knowledge_graph: Option<KnowledgeGraph>,

    // UI state
    selected_file: Option<PathBuf>,
    editor_text: String,
    editor_dirty: bool,

    // Visualization state
    root_node: Option<NodeIndex>,
    node_layouts: HashMap<NodeIndex, NodeLayout>,
    pan_offset: Vec2,
    zoom: f32,
    max_depth: usize,
    max_depth_limit: usize,

    // Pinned "must include" nodes
    pinned_nodes_text: String,

    // Verb filtering
    verb_enabled: HashMap<String, bool>,
    verb_colors: HashMap<String, Color32>,
    /// Verbs present in the current depth-reachable view (ignoring verb filters)
    visible_verbs: Vec<String>,

    // Tag filtering and coloring
    tag_enabled: HashMap<String, bool>,
    tag_colors: HashMap<String, Color32>,

    // Layout needs recompute
    layout_dirty: bool,

    // Intro zoom animation
    intro_zoom_target: f32,
    intro_animating: bool,

    // File watcher
    _watcher: Option<RecommendedWatcher>,
    watcher_rx: Option<mpsc::Receiver<PathBuf>>,
}

impl GroveApp {
    pub fn new(cc: &eframe::CreationContext<'_>, vault_path: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut app = Self {
            vault: Vault::new(),
            knowledge_graph: None,
            selected_file: None,
            editor_text: String::new(),
            editor_dirty: false,
            root_node: None,
            node_layouts: HashMap::new(),
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            max_depth: 10,
            max_depth_limit: 10,
            pinned_nodes_text: String::new(),
            verb_enabled: HashMap::new(),
            verb_colors: HashMap::new(),
            visible_verbs: Vec::new(),
            tag_enabled: HashMap::new(),
            tag_colors: HashMap::new(),
            layout_dirty: true,
            intro_zoom_target: 1.0,
            intro_animating: false,
            _watcher: None,
            watcher_rx: None,
        };

        if let Some(path) = vault_path {
            app.load_vault(&path);
        }

        app
    }

    fn load_vault(&mut self, path: &std::path::Path) {
        match Vault::load_from_directory(path) {
            Ok(vault) => {
                // Reset state for new vault
                self.vault = vault;
                self.selected_file = None;
                self.editor_text.clear();
                self.editor_dirty = false;
                self.root_node = None;
                self.node_layouts.clear();
                self.pan_offset = Vec2::ZERO;
                self.verb_enabled.clear();
                self.verb_colors.clear();
                self.visible_verbs.clear();
                self.pinned_nodes_text.clear();
                self.tag_enabled.clear();
                self.tag_colors.clear();

                self.rebuild_graph();
                self.start_watcher(path);

                // Start intro zoom: begin zoomed out, animate to normal
                let node_count = self.knowledge_graph.as_ref()
                    .map(|g| g.graph.node_count())
                    .unwrap_or(1) as f32;
                self.zoom = (0.15_f32).max(0.5 / (node_count / 20.0).max(1.0));
                self.intro_zoom_target = 1.0;
                self.intro_animating = true;

                log::info!("Loaded vault from {}", path.display());
            }
            Err(e) => {
                log::error!("Failed to load vault: {}", e);
            }
        }
    }

    fn start_watcher(&mut self, path: &std::path::Path) {
        let (tx, rx) = mpsc::channel();
        let sender = tx.clone();

        let watcher_result = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in event.paths {
                            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                                let _ = sender.send(path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        match watcher_result {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                    log::error!("Failed to watch directory: {}", e);
                    return;
                }
                self._watcher = Some(watcher);
                self.watcher_rx = Some(rx);
                log::info!("File watcher started for {}", path.display());
            }
            Err(e) => {
                log::error!("Failed to create file watcher: {}", e);
            }
        }
    }

    fn poll_watcher(&mut self) {
        let Some(rx) = &self.watcher_rx else { return };

        let mut changed = false;
        while let Ok(path) = rx.try_recv() {
            // Skip if this is the file we're currently editing (avoid fighting the editor)
            if self.selected_file.as_ref() == Some(&path) && self.editor_dirty {
                continue;
            }

            if let Err(e) = self.vault.reload_file(&path) {
                log::warn!("Failed to reload {}: {}", path.display(), e);
            } else {
                // If this is the selected file, refresh the editor text
                if self.selected_file.as_ref() == Some(&path) {
                    if let Some(contents) = self.vault.files.get(&path) {
                        self.editor_text = contents.clone();
                        self.editor_dirty = false;
                    }
                }
                changed = true;
            }
        }

        if changed {
            self.rebuild_graph();
        }
    }

    fn rebuild_graph(&mut self) {
        let mut all_triples = Vec::new();
        let mut file_subjects = Vec::new();
        let mut subject_tags: HashMap<String, Vec<String>> = HashMap::new();

        for (path, contents) in &self.vault.files {
            if let Some(subject) = parser::subject_from_path(path) {
                file_subjects.push(subject.clone());
                let triples = parser::parse_relationships(&subject, contents);
                all_triples.extend(triples);
                let tags = parser::parse_tags(contents);
                if !tags.is_empty() {
                    subject_tags.insert(subject, tags);
                }
            }
        }

        let graph = KnowledgeGraph::build(&all_triples, &file_subjects, &subject_tags);

        // Set up verb colors
        self.verb_colors.clear();
        for (i, key) in graph.verb_keys_sorted().iter().enumerate() {
            self.verb_colors
                .insert(key.clone(), VERB_COLORS[i % VERB_COLORS.len()]);
            self.verb_enabled.entry(key.clone()).or_insert(true);
        }

        // Set up tag colors from all unique tags across nodes
        {
            let mut all_tags = std::collections::BTreeSet::new();
            for node in graph.graph.node_weights() {
                for tag in &node.tags {
                    all_tags.insert(tag.clone());
                }
            }
            for (i, tag) in all_tags.iter().enumerate() {
                self.tag_colors.entry(tag.clone()).or_insert(TAG_COLORS[i % TAG_COLORS.len()]);
                self.tag_enabled.entry(tag.clone()).or_insert(true);
            }
        }

        // Auto-detect root
        if self.root_node.is_none() {
            self.root_node = graph.most_central_node();
        }

        self.knowledge_graph = Some(graph);
        self.layout_dirty = true;
    }

    fn compute_layout(&mut self, canvas_center: Pos2) {
        let graph = match &self.knowledge_graph {
            Some(g) => g,
            None => return,
        };
        let root = match self.root_node {
            Some(r) => r,
            None => return,
        };

        // Node visibility is based on all-verbs-enabled traversal so disabling
        // verbs only hides edges, not nodes.
        let all_enabled: HashMap<String, bool> = self.verb_enabled.keys().map(|k| (k.clone(), true)).collect();
        let mut visible = graph.visible_nodes(root, self.max_depth, &all_enabled);

        // Merge in shortest paths to pinned "must include" nodes
        let pinned_names: Vec<String> = self.pinned_nodes_text
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        for name in &pinned_names {
            // Case-insensitive lookup
            let target_idx = graph.node_indices.get(name).copied()
                .or_else(|| {
                    let lower = name.to_lowercase();
                    graph.node_indices.iter()
                        .find(|(k, _)| k.to_lowercase() == lower)
                        .map(|(_, &idx)| idx)
                });
            if let Some(target_idx) = target_idx {
                if !visible.contains_key(&target_idx) {
                    if let Some(path) = graph.shortest_path(root, target_idx) {
                        for (i, &node) in path.iter().enumerate() {
                            visible.entry(node).or_insert(i);
                        }
                    } else {
                        // No path to root — show as floating orphan
                        let max_d = visible.values().max().copied().unwrap_or(0);
                        visible.insert(target_idx, max_d + 1);
                    }
                }
            }
        }

        if visible.is_empty() {
            self.node_layouts.clear();
            return;
        }

        // Compute the true max depth from an unrestricted unlimited traversal
        let full_visible = graph.visible_nodes(root, usize::MAX, &all_enabled);
        self.max_depth_limit = full_visible.values().max().copied().unwrap_or(1).max(1);

        // Compute which verbs are present among visible nodes (for the controls panel)
        {
            let mut vkeys = std::collections::BTreeSet::new();
            for edge_ref in graph.graph.edge_references() {
                if visible.contains_key(&edge_ref.source()) && visible.contains_key(&edge_ref.target()) {
                    vkeys.insert(edge_ref.weight().verb_key.clone());
                }
            }
            self.visible_verbs = vkeys.into_iter().collect();
        }

        const PHI: f32 = 1.618;
        let base_spacing = 280.0;
        let min_radius = 14.0;
        let max_radius = 52.0;

        // Group visible nodes by depth, sorted deterministically
        let mut by_depth: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
        for (&node, &depth) in &visible {
            by_depth.entry(depth).or_default().push(node);
        }
        for nodes in by_depth.values_mut() {
            nodes.sort_by(|a, b| graph.graph[*a].name.cmp(&graph.graph[*b].name));
        }

        // Build parent map
        let mut parent_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        for (&node, &depth) in &visible {
            if depth == 0 { continue; }
            for neighbor in graph.graph.neighbors_undirected(node) {
                if visible.get(&neighbor).copied() == Some(depth - 1) {
                    parent_map.insert(node, neighbor);
                    break;
                }
            }
        }

        // Group children by parent
        let mut children_of: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        for (&child, &parent) in &parent_map {
            children_of.entry(parent).or_default().push(child);
        }
        for children in children_of.values_mut() {
            children.sort_by(|a, b| graph.graph[*a].name.cmp(&graph.graph[*b].name));
        }

        let mut layouts = HashMap::new();

        // Recursively compute subtree sizes for proportional angle allocation
        fn subtree_size(
            node: NodeIndex,
            children_of: &HashMap<NodeIndex, Vec<NodeIndex>>,
        ) -> usize {
            let children = children_of.get(&node);
            match children {
                None => 1,
                Some(kids) => {
                    let sum: usize = kids.iter().map(|&k| subtree_size(k, children_of)).sum();
                    sum.max(1)
                }
            }
        }

        // Place root at center
        let root_centrality = graph.graph[root].centrality;
        let root_radius = min_radius + root_centrality * (max_radius - min_radius);
        layouts.insert(root, NodeLayout {
            pos: canvas_center,
            radius: root_radius,
            depth: 0,
        });

        // Recursively place children with proportional angle slices
        fn place_children(
            parent: NodeIndex,
            parent_pos: Pos2,
            angle_start: f32,
            angle_span: f32,
            depth: usize,
            base_spacing: f32,
            min_radius: f32,
            max_radius: f32,
            children_of: &HashMap<NodeIndex, Vec<NodeIndex>>,
            graph: &petgraph::graph::DiGraph<crate::graph::KnowledgeNode, crate::graph::KnowledgeEdge>,
            layouts: &mut HashMap<NodeIndex, NodeLayout>,
            canvas_center: Pos2,
        ) {
            let children = match children_of.get(&parent) {
                Some(c) => c,
                None => return,
            };
            if children.is_empty() { return; }

            // Distance from parent — grows with depth but more gently
            let dist = base_spacing * (1.0 + (depth as f32 - 1.0) * 0.35);
            // Minimum angular gap between children (in radians)
            let min_gap = 0.15;

            // Compute subtree sizes for proportional allocation
            let sizes: Vec<usize> = children.iter()
                .map(|&c| subtree_size(c, children_of))
                .collect();
            let total_size: usize = sizes.iter().sum();

            // Ensure minimum span per child
            let needed_span = children.len() as f32 * min_gap;
            let effective_span = angle_span.max(needed_span);

            let mut current_angle = angle_start - effective_span / 2.0;
            for (i, &child) in children.iter().enumerate() {
                let proportion = sizes[i] as f32 / total_size as f32;
                let child_span = effective_span * proportion;
                let child_angle = current_angle + child_span / 2.0;

                let pos = Pos2::new(
                    parent_pos.x + child_angle.cos() * dist,
                    parent_pos.y + child_angle.sin() * dist,
                );

                let centrality = graph[child].centrality;
                let radius = min_radius + centrality * (max_radius - min_radius);

                layouts.insert(child, NodeLayout { pos, radius, depth });

                // Recurse: children inherit a narrower angular slice
                let child_sub_span = child_span * PHI / 2.0;
                place_children(
                    child, pos, child_angle, child_sub_span, depth + 1,
                    base_spacing, min_radius, max_radius,
                    children_of, graph, layouts, canvas_center,
                );

                current_angle += child_span;
            }
        }

        // For depth-1 children of root: distribute across full circle
        if let Some(root_children) = children_of.get(&root) {
            let sizes: Vec<usize> = root_children.iter()
                .map(|&c| subtree_size(c, &children_of))
                .collect();
            let total_size: usize = sizes.iter().sum();

            let mut current_angle: f32 = -std::f32::consts::FRAC_PI_2; // start from top
            for (i, &child) in root_children.iter().enumerate() {
                let proportion = sizes[i] as f32 / total_size as f32;
                let child_span = std::f32::consts::TAU * proportion;
                let child_angle = current_angle + child_span / 2.0;

                let dist = base_spacing;
                let pos = Pos2::new(
                    canvas_center.x + child_angle.cos() * dist,
                    canvas_center.y + child_angle.sin() * dist,
                );

                let centrality = graph.graph[child].centrality;
                let radius = min_radius + centrality * (max_radius - min_radius);
                layouts.insert(child, NodeLayout { pos, radius, depth: 1 });

                let sub_span = child_span * PHI / 2.0;
                place_children(
                    child, pos, child_angle, sub_span, 2,
                    base_spacing, min_radius, max_radius,
                    &children_of, &graph.graph, &mut layouts, canvas_center,
                );

                current_angle += child_span;
            }
        }

        // Place any orphan visible nodes (no parent found) evenly around the edge
        let max_depth_val = by_depth.keys().max().copied().unwrap_or(0);
        for depth in 1..=max_depth_val {
            if let Some(nodes) = by_depth.get(&depth) {
                let orphans: Vec<_> = nodes.iter()
                    .filter(|n| !layouts.contains_key(n))
                    .cloned()
                    .collect();
                if !orphans.is_empty() {
                    let ring_dist = base_spacing * (depth as f32 + 1.0);
                    for (i, node_idx) in orphans.iter().enumerate() {
                        let angle = std::f32::consts::TAU * (i as f32 / orphans.len() as f32);
                        let pos = Pos2::new(
                            canvas_center.x + angle.cos() * ring_dist,
                            canvas_center.y + angle.sin() * ring_dist,
                        );
                        let centrality = graph.graph[*node_idx].centrality;
                        let radius = min_radius + centrality * (max_radius - min_radius);
                        layouts.insert(*node_idx, NodeLayout { pos, radius, depth });
                    }
                }
            }
        }

        self.node_layouts = layouts;
        self.layout_dirty = false;
    }

    fn save_editor(&mut self) {
        if let Some(ref path) = self.selected_file {
            if let Err(e) = std::fs::write(path, &self.editor_text) {
                log::error!("Failed to save file: {}", e);
            } else {
                self.vault.files.insert(path.clone(), self.editor_text.clone());
                self.rebuild_graph();
                self.editor_dirty = false;
            }
        }
    }

    fn select_file(&mut self, path: PathBuf) {
        // Save current if dirty
        if self.editor_dirty {
            self.save_editor();
        }

        if let Some(contents) = self.vault.files.get(&path) {
            self.editor_text = contents.clone();
            self.selected_file = Some(path);
            self.editor_dirty = false;
        }
    }

    fn select_node_by_name(&mut self, name: &str) {
        if let Some(root) = &self.vault.root {
            // Find the file that matches this node name
            let target = self.vault.files.keys().find(|p| {
                parser::subject_from_path(p)
                    .map(|s| s == name)
                    .unwrap_or(false)
            }).cloned();

            if let Some(path) = target {
                self.select_file(path);
            } else {
                // Stub node: create new file
                let new_path = root.join(format!("{}.md", name));
                let initial = format!("# {}\n\n", name);
                if let Err(e) = std::fs::write(&new_path, &initial) {
                    log::error!("Failed to create file: {}", e);
                } else {
                    self.vault.files.insert(new_path.clone(), initial.clone());
                    self.editor_text = initial;
                    self.selected_file = Some(new_path);
                    self.editor_dirty = false;
                    self.rebuild_graph();
                }
            }
        }
    }
}

impl eframe::App for GroveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request periodic repaints so file watcher events are picked up
        // even when there's no direct user interaction.
        if self._watcher.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        // Poll file watcher for external changes
        self.poll_watcher();

        // Intro zoom animation
        if self.intro_animating {
            let speed = 0.02;
            let diff = self.intro_zoom_target - self.zoom;
            if diff.abs() < 0.005 {
                self.zoom = self.intro_zoom_target;
                self.intro_animating = false;
            } else {
                self.zoom += diff * speed;
                self.layout_dirty = true;
            }
            ctx.request_repaint();
        }

        // -- Left panel --
        egui::SidePanel::left("left_panel")
            .default_width(280.0)
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("📁 Vault");
                if self.vault.root.is_some() {
                    ui.horizontal(|ui| {
                        if let Some(ref root) = self.vault.root {
                            ui.label(
                                egui::RichText::new(
                                    root.file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("vault"),
                                )
                                .small()
                                .color(Color32::GRAY),
                            );
                        }
                        if ui.small_button("📂 Switch…").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.load_vault(&path);
                            }
                        }
                    });
                }
                ui.separator();

                // Open vault button
                if self.vault.root.is_none() {
                    if ui.button("Open Vault Directory…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.load_vault(&path);
                        }
                    }
                    return;
                }

                // File tree
                let tree = self.vault.file_tree();
                let mut file_to_select: Option<PathBuf> = None;

                egui::ScrollArea::vertical()
                    .id_salt("file_tree_scroll")
                    .max_height(ui.available_height() * 0.45)
                    .show(ui, |ui| {
                        Self::render_file_tree_node(
                            ui,
                            &tree,
                            &self.selected_file,
                            &mut file_to_select,
                            true,
                        );
                    });

                if let Some(path) = file_to_select {
                    // Set clicked file's node as root
                    if let Some(ref graph) = self.knowledge_graph {
                        if let Some(subject) = parser::subject_from_path(&path) {
                            if let Some(&idx) = graph.node_indices.get(&subject) {
                                self.root_node = Some(idx);
                                self.layout_dirty = true;
                            }
                        }
                    }
                    self.select_file(path);
                }

                ui.separator();

                // Markdown editor
                if self.selected_file.is_some() {
                    ui.horizontal(|ui| {
                        ui.label("📝 Editor");
                        if self.editor_dirty {
                            if ui.small_button("💾 Save").clicked() {
                                self.save_editor();
                            }
                            ui.colored_label(Color32::from_rgb(246, 173, 85), "modified");
                        }
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().id_salt("editor_scroll").show(ui, |ui| {
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut self.editor_text)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .code_editor(),
                        );
                        if response.changed() {
                            self.editor_dirty = true;
                        }
                        // Auto-save on focus loss
                        if response.lost_focus() && self.editor_dirty {
                            self.save_editor();
                        }
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Select a note to edit");
                    });
                }
            });

        // -- Right panel --
        egui::SidePanel::right("right_panel")
            .default_width(220.0)
            .min_width(180.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("⚙ Controls");
                ui.separator();

                // Theme selector
                ui.label("Theme:");
                ui.horizontal(|ui| {
                    if ui.selectable_label(ctx.style().visuals.dark_mode, "🌙 Dark").clicked() {
                        ctx.set_visuals(egui::Visuals::dark());
                    }
                    if ui.selectable_label(!ctx.style().visuals.dark_mode, "☀ Light").clicked() {
                        ctx.set_visuals(egui::Visuals::light());
                    }
                });

                ui.separator();

                // Depth slider
                ui.label("View Depth:");
                let max = self.max_depth_limit.max(1);
                let mut depth = self.max_depth.min(max);
                if ui
                    .add(egui::Slider::new(&mut depth, 0..=max).text("hops"))
                    .changed()
                {
                    self.max_depth = depth;
                    self.layout_dirty = true;
                }

                // Pinned / must-include nodes
                ui.label("Always show (comma-separated):");
                let pinned_response = ui.add(
                    egui::TextEdit::singleline(&mut self.pinned_nodes_text)
                        .desired_width(f32::INFINITY)
                        .hint_text("e.g. ATP, Nucleus, Mitochondria"),
                );
                if pinned_response.changed() || pinned_response.lost_focus() {
                    self.layout_dirty = true;
                }

                ui.separator();

                // Verb filters — only show verbs present in current view
                ui.heading("🏷 Relationships");
                ui.separator();

                let visible_verb_keys = &self.visible_verbs;

                // Select All / Deselect All
                ui.horizontal(|ui| {
                    if ui.small_button("✅ All").clicked() {
                        for key in visible_verb_keys {
                            self.verb_enabled.insert(key.clone(), true);
                        }
                        self.layout_dirty = true;
                    }
                    if ui.small_button("☐ None").clicked() {
                        for key in visible_verb_keys {
                            self.verb_enabled.insert(key.clone(), false);
                        }
                        self.layout_dirty = true;
                    }
                });
                ui.separator();

                let mut changed = false;
                egui::ScrollArea::vertical()
                    .id_salt("verb_filter_scroll")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for key in visible_verb_keys {
                            let color = self
                                .verb_colors
                                .get(key)
                                .copied()
                                .unwrap_or(Color32::GRAY);
                            let display = self
                                .knowledge_graph
                                .as_ref()
                                .map(|g| g.verb_display(key).to_string())
                                .unwrap_or_else(|| key.clone());

                            let enabled = self.verb_enabled.entry(key.clone()).or_insert(true);

                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(12.0, 12.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 2.0, color);

                                if ui.checkbox(enabled, &display).changed() {
                                    changed = true;
                                }
                            });
                        }
                    });
                if changed {
                    self.layout_dirty = true;
                }

                ui.separator();

                // Tag filters
                ui.heading("🏷 Tags");
                ui.separator();

                let visible_tags: Vec<String> = {
                    let mut tags = std::collections::BTreeSet::new();
                    if let Some(ref graph) = self.knowledge_graph {
                        for &node_idx in self.node_layouts.keys() {
                            for tag in &graph.graph[node_idx].tags {
                                tags.insert(tag.clone());
                            }
                        }
                    }
                    tags.into_iter().collect()
                };

                if !visible_tags.is_empty() {
                    ui.horizontal(|ui| {
                        if ui.small_button("✅ All").clicked() {
                            for tag in &visible_tags {
                                self.tag_enabled.insert(tag.clone(), true);
                            }
                            self.layout_dirty = true;
                        }
                        if ui.small_button("☐ None").clicked() {
                            for tag in &visible_tags {
                                self.tag_enabled.insert(tag.clone(), false);
                            }
                            self.layout_dirty = true;
                        }
                    });
                    ui.separator();
                }

                egui::ScrollArea::vertical()
                    .id_salt("tag_filter_scroll")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for tag in &visible_tags {
                            let color = self.tag_colors.get(tag).copied().unwrap_or(Color32::GRAY);
                            let enabled = self.tag_enabled.entry(tag.clone()).or_insert(true);
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(12.0, 12.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(rect.center(), 5.0, color);
                                ui.checkbox(enabled, tag);
                            });
                        }
                    });

                ui.separator();

                // Root node selector
                if let Some(ref graph) = self.knowledge_graph {
                    ui.heading("🌳 Root Node");
                    ui.separator();

                    let current_root_name = self
                        .root_node
                        .map(|idx| graph.graph[idx].name.clone())
                        .unwrap_or_else(|| "(none)".to_string());

                    egui::ComboBox::from_label("Root")
                        .selected_text(&current_root_name)
                        .show_ui(ui, |ui| {
                            let mut names: Vec<_> = graph
                                .node_indices
                                .iter()
                                .map(|(name, &idx)| (name.clone(), idx))
                                .collect();
                            names.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

                            for (name, idx) in names {
                                if ui
                                    .selectable_label(
                                        self.root_node == Some(idx),
                                        &name,
                                    )
                                    .clicked()
                                {
                                    self.root_node = Some(idx);
                                    self.layout_dirty = true;
                                }
                            }
                        });
                }
            });

        // -- Central panel: Mind Map --
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.knowledge_graph.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.heading("Open a vault directory to begin");
                });
                return;
            }

            // Handle pan and zoom
            let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click_and_drag());

            if response.dragged() {
                self.pan_offset += response.drag_delta();
                ctx.request_repaint();
            }

            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let zoom_factor = 1.0 + scroll * 0.002;
                self.zoom = (self.zoom * zoom_factor).clamp(0.2, 5.0);
                self.layout_dirty = true;
                ctx.request_repaint();
            }

            let canvas_rect = response.rect;
            let canvas_center = canvas_rect.center() + self.pan_offset;

            if self.layout_dirty {
                self.compute_layout(canvas_center);
            }

            let painter = ui.painter_at(canvas_rect);

            // Draw edges — group by (source, verb_key) for fan-out, use bezier curves
            if let Some(ref graph) = self.knowledge_graph {
                // Collect edges grouped by (source, verb_key) -> list of targets
                let mut edge_groups: HashMap<(NodeIndex, String), Vec<NodeIndex>> = HashMap::new();
                for edge_ref in graph.graph.edge_references() {
                    let source = edge_ref.source();
                    let target = edge_ref.target();
                    if !self.node_layouts.contains_key(&source) || !self.node_layouts.contains_key(&target) {
                        continue;
                    }
                    let verb_enabled = self
                        .verb_enabled
                        .get(&edge_ref.weight().verb_key)
                        .copied()
                        .unwrap_or(true);
                    if !verb_enabled {
                        continue;
                    }
                    edge_groups
                        .entry((source, edge_ref.weight().verb_key.clone()))
                        .or_default()
                        .push(target);
                }

                // Second pass: draw edges with curves
                for ((source, verb_key), targets) in &edge_groups {
                    let color = self
                        .verb_colors
                        .get(verb_key)
                        .copied()
                        .unwrap_or(Color32::GRAY);
                    let verb_display = graph.verb_display(verb_key);

                    let src_layout = &self.node_layouts[source];
                    let src_pos = Self::apply_zoom(src_layout.pos, canvas_center, self.zoom);

                    if targets.len() == 1 {
                        let tgt_layout = &self.node_layouts[&targets[0]];
                        let tgt_pos = Self::apply_zoom(tgt_layout.pos, canvas_center, self.zoom);

                        draw_curved_arrow(
                            &painter, src_pos, tgt_pos,
                            src_layout.radius * self.zoom,
                            tgt_layout.radius * self.zoom,
                            color, self.zoom,
                        );

                        // Label at curve midpoint (offset from straight midpoint)
                        let mid = bezier_midpoint(src_pos, tgt_pos);
                        let font = egui::FontId::proportional(9.5 * self.zoom);
                        painter.text(mid, egui::Align2::CENTER_CENTER, verb_display, font, color);
                    } else {
                        // Fan-out: stem + branches
                        let avg_dir: Vec2 = targets.iter()
                            .map(|t| {
                                let tp = Self::apply_zoom(self.node_layouts[t].pos, canvas_center, self.zoom);
                                (tp - src_pos).normalized()
                            })
                            .fold(Vec2::ZERO, |acc, d| acc + d)
                            .normalized();
                        let avg_dir = if avg_dir.length() < 0.01 { Vec2::new(1.0, 0.0) } else { avg_dir };

                        let min_dist = targets.iter()
                            .map(|t| {
                                let tp = Self::apply_zoom(self.node_layouts[t].pos, canvas_center, self.zoom);
                                (tp - src_pos).length()
                            })
                            .fold(f32::INFINITY, f32::min);
                        let stem_len = min_dist * 0.35;
                        let fork_point = src_pos + avg_dir * stem_len;

                        // Curved stem
                        let stem_ctrl = bezier_control(src_pos, fork_point);
                        draw_bezier_line(&painter, src_pos, stem_ctrl, fork_point,
                            egui::Stroke::new(2.0 * self.zoom, color));

                        // Label on stem
                        let label_pos = quadratic_bezier_point(src_pos, stem_ctrl, fork_point, 0.5);
                        let font = egui::FontId::proportional(9.5 * self.zoom);
                        let perp = Vec2::new(-avg_dir.y, avg_dir.x) * 10.0 * self.zoom;
                        painter.text(label_pos + perp, egui::Align2::CENTER_CENTER, verb_display, font, color);

                        // Branches
                        for &target in targets {
                            let tgt_layout = &self.node_layouts[&target];
                            let tgt_pos = Self::apply_zoom(tgt_layout.pos, canvas_center, self.zoom);

                            draw_curved_arrow(
                                &painter, fork_point, tgt_pos,
                                0.0, tgt_layout.radius * self.zoom,
                                color, self.zoom,
                            );
                        }
                    }
                }

                // Draw nodes
                let mut clicked_node: Option<String> = None;
                let mut double_clicked_node: Option<NodeIndex> = None;

                for (&node_idx, layout) in &self.node_layouts {
                    let node = &graph.graph[node_idx];
                    let pos = Self::apply_zoom(layout.pos, canvas_center, self.zoom);
                    let radius = layout.radius * self.zoom;

                    // Node fill — color by first tag, concentric rings for additional tags
                    let is_selected = self
                        .selected_file
                        .as_ref()
                        .and_then(|p| parser::subject_from_path(p))
                        .map(|s| s == node.name)
                        .unwrap_or(false);

                    let is_root = self.root_node == Some(node_idx);

                    // Fade nodes at deeper levels for depth-of-field effect
                    let depth_alpha = if self.max_depth_limit > 0 {
                        let t = layout.depth as f32 / self.max_depth_limit as f32;
                        (255.0 * (1.0 - t * 0.4)) as u8
                    } else {
                        255
                    };

                    // Determine fill from first tag (if any and enabled)
                    let tag_fill = node.tags.first()
                        .filter(|t| self.tag_enabled.get(*t).copied().unwrap_or(true))
                        .and_then(|t| self.tag_colors.get(t).copied());

                    let fill = if let Some(c) = tag_fill {
                        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), depth_alpha)
                    } else if is_root {
                        Color32::from_rgba_unmultiplied(99, 179, 237, depth_alpha)
                    } else if !node.has_file {
                        Color32::from_rgba_unmultiplied(80, 80, 90, (180.0 * depth_alpha as f32 / 255.0) as u8)
                    } else {
                        Color32::from_rgba_unmultiplied(45, 55, 72, depth_alpha)
                    };

                    // Draw concentric ring borders for additional tags (outermost = 2nd tag, etc.)
                    let extra_tags: Vec<Color32> = node.tags.iter().skip(1)
                        .filter(|t| self.tag_enabled.get(*t).copied().unwrap_or(true))
                        .filter_map(|t| self.tag_colors.get(t).copied())
                        .collect();
                    let ring_width = 2.5 * self.zoom;
                    for (i, &ring_color) in extra_tags.iter().rev().enumerate() {
                        let ring_r = radius + ring_width * (extra_tags.len() - i) as f32;
                        painter.circle_stroke(pos, ring_r, egui::Stroke::new(ring_width, ring_color));
                    }

                    let stroke = if is_selected {
                        egui::Stroke::new(3.0, Color32::from_rgb(252, 211, 77))
                    } else if !node.has_file {
                        egui::Stroke::new(1.5, Color32::from_rgb(100, 100, 120))
                    } else {
                        egui::Stroke::new(1.5, Color32::from_rgb(120, 140, 170))
                    };

                    painter.circle(pos, radius, fill, stroke);

                    // Node label
                    let font_size = (11.0 + node.centrality * 6.0) * self.zoom;
                    let font = egui::FontId::proportional(font_size);
                    painter.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        &node.name,
                        font.clone(),
                        Color32::WHITE,
                    );

                    // Click detection
                    let node_rect = egui::Rect::from_center_size(
                        pos,
                        egui::vec2(radius * 2.0, radius * 2.0),
                    );
                    let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
                    if let Some(ptr) = pointer_pos {
                        if node_rect.contains(ptr) {
                            // Tooltip: show first sentence from the note
                            let tooltip_text = self.vault.files.iter()
                                .find(|(p, _)| {
                                    parser::subject_from_path(p)
                                        .map(|s| s == node.name)
                                        .unwrap_or(false)
                                })
                                .and_then(|(_, contents)| parser::first_sentence(contents));

                            if let Some(text) = tooltip_text {
                                egui::show_tooltip_at_pointer(ctx, response.layer_id, egui::Id::new("node_tooltip"), |ui| {
                                    ui.label(egui::RichText::new(&node.name).strong());
                                    ui.label(&text);
                                });
                            } else if !node.has_file {
                                egui::show_tooltip_at_pointer(ctx, response.layer_id, egui::Id::new("node_tooltip"), |ui| {
                                    ui.label(egui::RichText::new(&node.name).strong());
                                    ui.colored_label(Color32::GRAY, "(no file yet — click to create)");
                                });
                            }

                            if response.clicked() {
                                clicked_node = Some(node.name.clone());
                            }
                            if response.double_clicked() {
                                double_clicked_node = Some(node_idx);
                            }
                        }
                    }
                }

                // Handle node interactions
                if let Some(name) = clicked_node {
                    self.select_node_by_name(&name);
                }
                if let Some(idx) = double_clicked_node {
                    self.root_node = Some(idx);
                    self.layout_dirty = true;
                }
            }
        });
    }
}

impl GroveApp {
    fn apply_zoom(pos: Pos2, center: Pos2, zoom: f32) -> Pos2 {
        let offset = pos - center;
        center + offset * zoom
    }

    fn render_file_tree_node(
        ui: &mut egui::Ui,
        node: &crate::vault::FileTreeNode,
        selected: &Option<PathBuf>,
        file_to_select: &mut Option<PathBuf>,
        is_root: bool,
    ) {
        if node.is_dir {
            let header = if is_root {
                format!("📂 {}", node.name)
            } else {
                format!("📁 {}", node.name)
            };
            egui::CollapsingHeader::new(header)
                .default_open(is_root)
                .show(ui, |ui| {
                    for child in &node.children {
                        Self::render_file_tree_node(ui, child, selected, file_to_select, false);
                    }
                });
        } else {
            let is_selected = selected.as_ref() == Some(&node.path);
            let label = format!("📄 {}", node.name.trim_end_matches(".md"));
            if ui
                .selectable_label(is_selected, label)
                .clicked()
            {
                *file_to_select = Some(node.path.clone());
            }
        }
    }

}

// ── Bezier curve helpers ──

/// Compute a control point for a gentle curve between two points.
/// Offsets perpendicular to the line for a subtle organic arc.
fn bezier_control(from: Pos2, to: Pos2) -> Pos2 {
    let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    let dir = to - from;
    let perp = Vec2::new(-dir.y, dir.x);
    // Offset proportional to distance — gentle curve
    let offset = perp.normalized() * dir.length() * 0.08;
    mid + offset
}

/// Evaluate a quadratic bezier at parameter t.
fn quadratic_bezier_point(p0: Pos2, p1: Pos2, p2: Pos2, t: f32) -> Pos2 {
    let inv = 1.0 - t;
    Pos2::new(
        inv * inv * p0.x + 2.0 * inv * t * p1.x + t * t * p2.x,
        inv * inv * p0.y + 2.0 * inv * t * p1.y + t * t * p2.y,
    )
}

/// Draw a quadratic bezier curve as a polyline.
fn draw_bezier_line(painter: &egui::Painter, p0: Pos2, ctrl: Pos2, p2: Pos2, stroke: egui::Stroke) {
    let segments = 20;
    let points: Vec<Pos2> = (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            quadratic_bezier_point(p0, ctrl, p2, t)
        })
        .collect();
    for w in points.windows(2) {
        painter.line_segment([w[0], w[1]], stroke);
    }
}

/// Get the visual midpoint of a curved edge (for label placement).
fn bezier_midpoint(from: Pos2, to: Pos2) -> Pos2 {
    let ctrl = bezier_control(from, to);
    let mid = quadratic_bezier_point(from, ctrl, to, 0.5);
    // Offset the label slightly further from the curve to avoid overlap
    let dir = to - from;
    let perp = Vec2::new(-dir.y, dir.x).normalized() * 12.0;
    mid + perp
}

/// Draw a curved arrow from `from` to `to` using a quadratic bezier.
fn draw_curved_arrow(
    painter: &egui::Painter,
    from: Pos2,
    to: Pos2,
    source_radius: f32,
    target_radius: f32,
    color: Color32,
    zoom: f32,
) {
    let dir = (to - from).normalized();
    if dir.length() < 0.01 {
        return;
    }

    let start = from + dir * source_radius;
    let arrow_tip = to - dir * target_radius;
    let ctrl = bezier_control(start, arrow_tip);

    draw_bezier_line(painter, start, ctrl, arrow_tip,
        egui::Stroke::new(1.5 * zoom, color));

    // Arrowhead at tip
    let arrow_size = 7.0 * zoom;
    let tip_dir = (arrow_tip - quadratic_bezier_point(start, ctrl, arrow_tip, 0.92)).normalized();
    let tip_dir = if tip_dir.length() < 0.01 { dir } else { tip_dir };
    let perp = Vec2::new(-tip_dir.y, tip_dir.x);
    let arrow_left = arrow_tip - tip_dir * arrow_size + perp * arrow_size * 0.4;
    let arrow_right = arrow_tip - tip_dir * arrow_size - perp * arrow_size * 0.4;
    painter.add(egui::Shape::convex_polygon(
        vec![arrow_tip, arrow_left, arrow_right],
        color,
        egui::Stroke::NONE,
    ));
}
