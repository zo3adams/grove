# GROVE — Product Requirements
## Graph-Rendered Ontology for Visual Exploration

## Vision
A WYSIWYG mind-map tool for rapid learning of complex systems (business processes, codebases, physical processes). Users author plain markdown notes with semantic relationship annotations, and the app renders them as a beautiful, interactive tree-of-knowledge visualization.

## Core Concepts

### Notes as Knowledge Nodes
- Each `.md` file in a **vault** (user-chosen directory) represents a **subject/concept**.
- The filename (minus `.md`) is the subject name (e.g., `Mitochondria.md` → "Mitochondria").
- Notes contain standard markdown plus **relationship annotations**.

### Relationship Syntax
```
[[verb -> object]]
```
- **verb**: An arbitrary relationship label the user supplies (e.g., "produces", "contains", "depends on").
- **object**: The name of another concept (corresponds to another note's filename).
- The **subject** is always the file the annotation appears in.
- Annotations can appear anywhere in the markdown — inline, in lists, in headings, etc.

#### Examples
```markdown
# Mitochondria

The mitochondria is the powerhouse of the cell.

- [[produces -> ATP]]
- [[located in -> Eukaryotic Cell]]
- [[has membrane -> Inner Membrane]]

Energy production via [[performs -> Oxidative Phosphorylation]].
```
**Parsed triples:**
- (Mitochondria, produces, ATP)
- (Mitochondria, located in, Eukaryotic Cell)
- (Mitochondria, has membrane, Inner Membrane)
- (Mitochondria, performs, Oxidative Phosphorylation)

### Stub Nodes
- If `[[verb -> Foo]]` references `Foo` but `Foo.md` doesn't exist, "Foo" still appears in the graph as a **stub node** (visually distinct — e.g., dashed border or dimmed).
- Users can click a stub to create the corresponding `.md` file.

## UI Layout

### Three-Panel Design
```
┌──────────────┬─────────────────────────────┬──────────────┐
│  File Tree   │                             │  Controls    │
│  (vault)     │     Mind-Map / Tree of      │  - Depth     │
│              │     Knowledge Canvas        │  - Verb      │
│──────────────│     (central, dominant)      │    filters   │
│  Markdown    │                             │  - Legend    │
│  Editor      │                             │    (colors)  │
│  (selected)  │                             │              │
└──────────────┴─────────────────────────────┴──────────────┘
```

### Left Panel
- **Top**: Collapsible file/folder tree showing the vault's `.md` files.
- **Bottom**: Editable markdown of the currently selected note. Changes auto-save (debounced) and trigger re-parse.

### Center Panel (The Star)
- **Mind-map / tree-of-knowledge visualization**.
- Root node at center; branches radiate outward.
- **Node size** proportional to centrality (more connections = larger).
- **Edges** labeled with verbs, color-coded by verb type.
- Pan (drag) and zoom (scroll) controls.
- Click a node to select it (shows its markdown in the editor).
- Double-click a node to re-root the tree view on it.

### Right Panel
- **Depth slider**: Control how many hops from root are rendered (1, 2, 3, … all).
- **Verb/relationship filters**: Checkboxes for each unique verb. Uncheck to hide those edges (and resulting orphan nodes).
- **Color legend**: Auto-generated mapping of verbs → colors. Shows which color corresponds to which relationship type.

## Root Node Selection
- **Auto-detect**: On vault load, the node with the highest degree centrality becomes the default root.
- **Manual override**: User can double-click any node, or select from a dropdown, to set it as root.
- The mind-map re-layouts around the new root.

## Visualization Requirements
- **Layout**: Radial tree / force-directed hybrid.
  - Root at center.
  - Direct children in the first ring, grandchildren in the second ring, etc.
  - Avoid overlapping nodes; spread evenly.
- **Node appearance**:
  - Circles (or rounded rects) with the concept name as label.
  - Radius proportional to centrality.
  - Stub nodes visually distinct (dashed outline, lower opacity).
  - Selected node highlighted (glow or thicker border).
- **Edge appearance**:
  - Curved lines (bezier) connecting nodes.
  - Labeled with the verb at the midpoint.
  - Color-coded by verb (consistent across the visualization).
  - Arrow indicating direction (subject → object).
- **Aesthetics**: Clean, modern, engaging. Dark and light theme support.

## Data & Storage
- **Plain `.md` files** on the local filesystem.
- No database — the graph is built in-memory from file contents on each load.
- Vault directory is user-chosen (file dialog or CLI argument).
- Git-friendly: users can version their vault with git.

## File Watching
- The app watches the vault directory for changes (create, modify, delete).
- On external change, affected files are re-parsed and the graph is updated incrementally.
- The visualization updates in near-real-time.

## Verbs & Grouping
- Verbs are **case-insensitive** for grouping purposes (e.g., "Produces" and "produces" are the same verb group).
- Display uses the casing from the first occurrence found.
- Each unique verb group gets a consistent color from a palette.

## Non-Goals (for v1)
- No cloud sync or collaboration features.
- No WYSIWYG markdown rendering in the editor (plain text editing is fine for v1).
- No export to other formats (PDF, image) — stretch goal.
- No plugin system.
- No mobile support.

## Technical Constraints
- All Rust, native app (eframe/egui).
- Must run on macOS (primary), Linux, and Windows.
- Should handle vaults with hundreds of notes smoothly.
- Layout computation should be fast enough for interactive re-rooting.
