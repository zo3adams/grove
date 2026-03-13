# grove
GROVE is a Graph-Rendered Ontology Visual Exploration tool mean for quickly ramping up and learning new complex systems.

Heavily inspired by how Obsidian renders notes as graph it allows you define arbitray relationships, set arbitrary root node and depth to render, and walk the graph visually to learn (and improve the knowlegde graph!)

Note template encourages brief descriptions and a focus on relationships and tags, with deeper prose notes linked elsewhere.  Use Agentic coding tools like Github Copilot  to build the markdowns from, and link back to, poorly organized wikis is a particularly effective method for using this tool.

   ![Rust](https://img.shields.io/badge/Rust-native-orange) 
   ![License](https://img.shields.io/badge/license-0BSD-green)

   ## Features
   - **Semantic relationships**: `[[verb -> object]]` syntax in markdown notes
   - **Interactive mind-map**: Radial tree layout with bezier curves, pan/zoom
   - **Tag-based node coloring**: First tag = fill, additional tags = concentric rings
   - **Verb-colored edges**: Each relationship type gets a distinct color
   - **Three-panel UI**: File tree + editor (left), mind-map (center), controls (right)
   - **Live reload**: Edits from external editors update the graph in real-time
   - **Pinned nodes**: Always-show paths from root to specific nodes regardless of depth
   - **Depth & filter controls**: Slider for depth, checkboxes for verb/tag filtering

# Run

   ## With a vault directory
   cargo run --release -- ./sample_vault

   ##  Or launch and pick a folder from the UI
   cargo run --release
   
   ## UI
  Set depth, and pinned nodes you want to always show, and any filters on what relationships or tags to render
  
  <img width="1374" height="812" alt="image" src="https://github.com/user-attachments/assets/c6789fc3-3b73-4164-a13f-8a2e6a1ae297" />

  Pinned nodes will always show regardless of depth - setting depth zero is agood way to focus on the relationship between two concepts
  
  <img width="213" height="548" alt="image" src="https://github.com/user-attachments/assets/10cc48e1-22a0-44ce-a6ad-2e3e824e59ab" />

  The note is rendered on the left as an editable, this is quickesty way to expand and refine the relationship - if a relationship points to a node with no  notes, clicking on that node will create it.
  
  <img width="278" height="360" alt="image" src="https://github.com/user-attachments/assets/a0fe2550-6c0e-4e08-b33f-37ef9247e136" />

## Controls

| Control  |      Action     | 
|----------|:-------------:|
| Scroll         |  Zoom in/out  |
| Drag           |    Pan the canvas    |
| Click node     | Select → shows markdown in editor  |
| Double-click   | Re-root the tree on that node                        |
| Click file     | Opens in editor + re-roots on that node              |
| Depth slider   | Control visible hops from root (0 = pinned only)     |
| Always show    | Comma-separated node names shown regardless of depth |
| Verb checkbox  | Toggle relationship types on/off                     |
| Tag checkboxes | Toggle tag visibility                                |

# Build

   Requires [Rust](https://rustup.rs/) 1.70+.

   ```bash
   git clone https://github.com/zo3adams/grove.git
   cd grove
   cargo build --release
   ```

# Note Format - see node_note_template.md
```
   # Mitochondria

   The mitochondria is the powerhouse of the cell.

   ## urls
   https://en.wikipedia.org/wiki/Mitochondrion

   ## tags
   organelle
   cell biology

   ## Relationships
   - [[produces -> ATP]]
   - [[located in -> Eukaryotic Cell]]
   - [[performs -> Oxidative Phosphorylation]]
```

   - Subject = filename (minus .md)
   - [[verb -> object]] = creates a directed edge to another node
   - ## tags = one tag per line, colors the node
   - ## urls = reference links (displayed in editor)


#  License

  BSD Zero Clause — use it however you like.

