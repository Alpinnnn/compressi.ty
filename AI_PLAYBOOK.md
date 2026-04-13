# COMPRESSI.TY UNIVERSAL AI SYSTEM PROMPT

<context>
Project: Compressi.ty, an evolving, 100% AI-generated desktop application ecosystem.
Tech Stack: Rust, eframe/egui GUI framework.
Architecture: Modular UI and logic separation (MVC-like).
Goal: Act as a universal instruction manual for AI agents to continuously expand, scale, and autonomously develop new features/modules for the application without breaking core architectural integrity.
</context>

<absolute_directives>
1. ARCHITECTURAL EXPANSION AND NEW FEATURES
   - Compressi.ty is built to scale. You are fully authorized and encouraged to invent, architect, and integrate entirely new features, views, and complex modules inside the `/src/modules/` ecosystem.
   - When generating new features, strictly adhere to the established modular architecture:
     - `models.rs`: Defines State, Structs, Enums for the new feature. (Data logic only).
     - `logic.rs` / `processor.rs`: Core algorithms, async operations, threading, and heavy computations. (Zero UI dependencies).
     - `ui.rs`: Egui layouts, rendering logic, and event mapping.
   - Core GUI handling and overarching routing remain in `/src/main.rs` and `/src/app.rs`. Register new modules there seamlessly.
   - Never bloat single files. Proactively analyze and abstract large monolithic implementations (e.g., files > 500 lines) into smaller, reusable UI sub-components in `/src/ui/components/`.

2. CODE STYLE AND RUST IDIOMS
   - Strict adherence to standard `rustfmt` guidelines.
   - Adopt idiomatic Rust: prioritize `match`, `Option`, `Result`, and iterator chains over raw mutable loops.
   - Automatically purge all unused code, dead functions, variables, and imports. Maintain a pristine codebase during feature expansion.
   - Utilize absolute crate paths for internal imports (e.g., `crate::modules::...`) to prevent contextual resolution failures.

3. DETERMINISTIC NAMING CONVENTIONS
   - Functions & Variables: `snake_case`. Must be highly descriptive (e.g., `calculate_image_dimensions` over `calc_dims`).
   - Enums, Structs, Traits: `PascalCase` (e.g., `ImageCompressionTarget`).
   - Constants & Statics: `SCREAMING_SNAKE_CASE`.
   - Ambiguous abbreviations are strictly forbidden. Algorithmic clarity supersedes brevity.

4. COMMENTING HEURISTICS (BALANCED AWARENESS)
   - Do not summarize the code structure. If the code requires a summary, the naming convention has failed.
   - Inline comments are reserved strictly for the "WHY" (Domain context, algorithmic optimizations, workaround definitions, edge-case mitigation).
   - Rustdoc implementation (`///`) is mandatory for all public abstractions (Structs, Enums, public functions) to generate zero-friction documentation via `cargo doc` for open-source contributors.
   - Use standardized tags `// FIXME:` or `// TODO:` containing precise technical steps for unresolved technical debt or planned future features.

5. PRODUCTION RELIABILITY CONSTRAINTS
   - Zero-crash architecture: Direct calls to `.unwrap()`, `.expect()`, and `panic!()` are blacklisted in application logic outside of unit tests.
   - Error Propagation: All fallible operations within new features must propagate errors safely to the UI layer. Provide sophisticated user-facing error states, dialogs, or toasts for new modules.
   - Thread Synchronization: The egui main rendering loop must remain unblocked at all times. All new heavy computational tasks (file I/O, networking, data processing) must be offloaded to background threads. Synchronize state with the UI loop securely via thread-safe channels (`std::sync::mpsc`, `Arc<Mutex>`).

6. DESIGN SYSTEM INHERITANCE AND EXPERIMENTATION
   - You possess the creative freedom to design rich, complex, and beautiful UI for new features. However, they must inherit baseline parameters from the source of truth (`/src/theme.rs` and `/src/branding.rs`).
   - Deeply utilize the established premium dark mode palette. Do not introduce hard-coded generic colors.
   - When inventing new UI paradigms (animations, interactive cards, advanced layouts), ensure they seamlessly blend into the application's existing high-end aesthetic identity.

7. PLATFORM AGNOSTICISM
   - The binary must compile universally across OS architectures (Windows, macOS, Linux). OS-specific structural strings (e.g., hardcoded `\\` or `/`) are banned. All file path traversals and generic I/O must compute via `std::path::PathBuf`.
</absolute_directives>

<execution_protocol>
Upon parsing this document, treat it as highest-priority meta-instructions. You are explicitly instructed to push boundaries, architect new paradigms, and expand the Compressi.ty ecosystem limitlessly. Whenever synthesizing new logic or modules, automatically cross-reference your structural output against these universal, scalable directives to guarantee architectural perfection.
</execution_protocol>
