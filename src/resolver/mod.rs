// Module resolver for Palladium
// "Finding legends across realms"

use crate::ast::{Import, Program};
use crate::errors::{CompileError, Result};
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// Information about a resolved module
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub path: PathBuf,
    pub ast: Program,
    pub exports: HashSet<String>, // Names of exported items
}

/// Module resolver handles finding and loading modules
pub struct ModuleResolver {
    /// Search paths for modules (like PYTHONPATH)
    search_paths: Vec<PathBuf>,
    /// Cache of already loaded modules
    loaded_modules: HashMap<String, ModuleInfo>,
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleResolver {
    pub fn new() -> Self {
        let mut search_paths = vec![
            PathBuf::from("."),        // Current directory
            PathBuf::from("examples"), // Examples directory (temporary for testing)
        ];

        // Add standard library path if it exists
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                let std_lib = parent.join("std");
                if std_lib.exists() {
                    search_paths.push(std_lib.clone());
                }
            }
        }

        // Check for PALLADIUM_PATH environment variable
        if let Ok(pd_path) = std::env::var("PALLADIUM_PATH") {
            for path in pd_path.split(':') {
                search_paths.push(PathBuf::from(path));
            }
        }

        Self {
            search_paths,
            loaded_modules: HashMap::new(),
        }
    }

    /// Add a search path
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Resolve all imports in a program
    pub fn resolve_program(&mut self, program: &Program) -> Result<HashMap<String, ModuleInfo>> {
        let mut resolved = HashMap::new();

        for import in &program.imports {
            let module_name = self.import_to_module_name(import);
            if !self.loaded_modules.contains_key(&module_name) {
                self.load_module(&module_name)?;
            }

            if let Some(module_info) = self.loaded_modules.get(&module_name) {
                // Filter module info based on imported items if specified
                let filtered_info = if let Some(items) = &import.items {
                    self.filter_module_info(module_info, items)
                } else {
                    module_info.clone()
                };

                // Use alias if provided, otherwise use the full module name
                let key = if let Some(alias) = &import.alias {
                    alias.clone()
                } else {
                    module_name
                };
                resolved.insert(key, filtered_info);
            }
        }

        Ok(resolved)
    }

    /// Convert import path to module name
    fn import_to_module_name(&self, import: &Import) -> String {
        import.path.join("::")
    }

    /// Filter module info to only include specified items
    fn filter_module_info(&self, module_info: &ModuleInfo, items: &[String]) -> ModuleInfo {
        let item_set: HashSet<_> = items.iter().cloned().collect();

        ModuleInfo {
            path: module_info.path.clone(),
            ast: module_info.ast.clone(),
            exports: module_info
                .exports
                .iter()
                .filter(|export_name| item_set.contains(*export_name))
                .cloned()
                .collect(),
        }
    }

    /// Load a module by name
    fn load_module(&mut self, module_name: &str) -> Result<()> {
        // Convert module name to file path (e.g., "std::io" -> "std/io.pd")
        let module_path = module_name.replace("::", "/");
        let file_name = format!("{}.pd", module_path);

        // Try to find the module file
        let mut module_file = None;
        for search_path in &self.search_paths {
            let full_path = search_path.join(&file_name);
            if full_path.exists() {
                module_file = Some(full_path);
                break;
            }
        }

        let module_file = module_file
            .ok_or_else(|| CompileError::Generic(format!("Module '{}' not found", module_name)))?;

        // Read and parse the module
        let source = fs::read_to_string(&module_file).map_err(|e| {
            CompileError::Generic(format!("Failed to read module '{}': {}", module_name, e))
        })?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.collect_tokens()?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse()?;

        // Extract exported items (those marked as 'pub')
        let mut exports = HashSet::new();
        for item in &ast.items {
            match item {
                crate::ast::Item::Function(func) => {
                    if matches!(func.visibility, crate::ast::Visibility::Public) {
                        exports.insert(func.name.clone());
                    }
                }
                crate::ast::Item::Struct(struct_def) => {
                    if matches!(struct_def.visibility, crate::ast::Visibility::Public) {
                        exports.insert(struct_def.name.clone());
                    }
                }
                crate::ast::Item::Enum(enum_def) => {
                    // Note: EnumDef doesn't have a visibility field in the current AST
                    // This would need to be added to the AST to support private enums
                    // For now, all enums are treated as public
                    exports.insert(enum_def.name.clone());
                }
            }
        }

        // Recursively resolve imports in the loaded module
        let _sub_modules = self.resolve_program(&ast)?;

        // Store the loaded module
        let module_info = ModuleInfo {
            path: module_file,
            ast,
            exports,
        };

        self.loaded_modules
            .insert(module_name.to_string(), module_info);

        Ok(())
    }

    /// Get all loaded modules
    pub fn get_loaded_modules(&self) -> &HashMap<String, ModuleInfo> {
        &self.loaded_modules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_name_conversion() {
        let resolver = ModuleResolver::new();
        let import = Import {
            path: vec!["std".to_string(), "io".to_string()],
            items: None,
            alias: None,
            span: crate::errors::Span::dummy(),
        };

        assert_eq!(resolver.import_to_module_name(&import), "std::io");
    }
}
