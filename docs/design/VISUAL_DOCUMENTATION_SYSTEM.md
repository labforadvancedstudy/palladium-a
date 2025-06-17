# Palladium Visual Documentation System Implementation Plan

## Current State Analysis
The current docs/ directory is cluttered with ~20 bootstrap-related files at the root level, making navigation difficult. We need to reorganize according to the visual documentation system spec.

## New Documentation Structure

```
docs/
├── README.md                      # Main documentation entry point with visual progress
├── features/                      # Feature specifications and status
│   ├── status.yaml               # Central progress tracking
│   ├── core-language/            # Core language features
│   │   ├── implicit-lifetimes.md
│   │   ├── borrow-checker.md
│   │   ├── type-inference.md
│   │   └── ...
│   ├── async-system/             # Async features
│   │   ├── async-as-effect.md
│   │   └── ...
│   └── advanced/                 # Advanced features
│       ├── totality-checking.md
│       └── ...
├── guides/                       # User guides
│   ├── getting-started.md
│   ├── migration-from-rust.md
│   └── ...
├── reference/                    # Language reference
│   ├── syntax.md
│   ├── types.md
│   └── ...
├── bootstrap/                    # Bootstrap documentation (moved from root)
│   ├── README.md
│   ├── status.md
│   ├── tutorial.md
│   └── ...
├── internals/                    # Compiler internals
│   ├── architecture.md
│   ├── generics-design.md
│   └── ...
└── tools/                        # Tooling documentation
    ├── pdc.md
    ├── formatter.md
    └── ...
```

## Migration Plan

### Phase 1: Create Directory Structure
1. Create new directories: features/, guides/, reference/, bootstrap/, internals/, tools/
2. Create subdirectories in features/ matching status.yaml structure

### Phase 2: Move Existing Files
- Bootstrap files → docs/bootstrap/
- Design docs → docs/internals/
- Getting started → docs/guides/
- Module/Generics design → docs/internals/

### Phase 3: Create Feature Documentation
- Copy status.yaml from visual_docs
- Create feature documentation templates
- Generate initial feature docs from status.yaml

### Phase 4: Update README.md
- Add visual progress indicators
- Create feature tree visualization
- Add quick links to all sections

### Phase 5: Generate Dashboard
- Create HTML dashboard from status.yaml
- Add to README as preview
- Set up auto-generation script

## Visual Progress Indicators

### In README.md:
```markdown
## 🚀 Palladium Implementation Progress

### Core Language [████████░░] 82%
- ✅ Type System (90%)
- ✅ Borrow Checker (95%)  
- ⏳ Implicit Lifetimes (80%)
- ⏳ Traits (70%)

### Bootstrap Progress [██████░░░░] 60%
- ✅ Tiny Compiler v16 (100%)
- ⏳ Self-hosting Capability (60%)
- 🔲 Full Compiler Port (0%)

### Advanced Features [███░░░░░░░] 30%
- ⏳ Async as Effect (40%)
- ⏳ Totality Checking (30%)
- 🔲 Proof Generation (0%)
```

## Feature Documentation Template

Each feature will have:
1. Status badge and percentage
2. Code comparison (Rust vs Go vs Palladium)
3. Why this feature exists
4. Implementation notes
5. Roadmap/TODOs

## Benefits
- Clean, navigable structure
- Visual progress at a glance
- Consistent documentation format
- Easy to update and maintain
- Single source of truth for project status