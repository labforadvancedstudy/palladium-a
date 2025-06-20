// Trait resolution for Palladium type checker
// Handles trait implementations and method resolution

use crate::ast::{ImplBlock, TraitDef, Type};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;

/// Information about a trait
#[derive(Debug, Clone)]
pub struct TraitInfo {
    #[allow(dead_code)]
    pub name: String,
    pub methods: HashMap<String, TraitMethodInfo>,
    #[allow(dead_code)]
    pub type_params: Vec<String>,
}

/// Information about a trait method
#[derive(Debug, Clone)]
pub struct TraitMethodInfo {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub params: Vec<Type>,
    #[allow(dead_code)]
    pub return_type: Option<Type>,
    pub has_default: bool,
}

/// Implementation information
#[derive(Debug, Clone)]
pub struct ImplInfo {
    #[allow(dead_code)]
    pub trait_name: Option<String>, // None for inherent impl
    #[allow(dead_code)]
    pub for_type: Type,
    #[allow(dead_code)]
    pub methods: HashMap<String, MethodImpl>,
}

/// Method implementation details
#[derive(Debug, Clone)]
pub struct MethodImpl {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub type_params: Vec<String>,
    #[allow(dead_code)]
    pub is_generic: bool,
}

/// Trait resolver manages trait definitions and implementations
pub struct TraitResolver {
    /// All trait definitions
    traits: HashMap<String, TraitInfo>,
    /// All implementations
    impls: Vec<ImplInfo>,
    /// Type to implementations mapping for fast lookup
    type_impls: HashMap<String, Vec<usize>>, // Type name -> impl indices
}

impl TraitResolver {
    pub fn new() -> Self {
        Self {
            traits: HashMap::new(),
            impls: Vec::new(),
            type_impls: HashMap::new(),
        }
    }

    /// Register a trait definition
    pub fn register_trait(&mut self, trait_def: &TraitDef) -> Result<()> {
        if self.traits.contains_key(&trait_def.name) {
            return Err(CompileError::Generic(format!(
                "Trait '{}' already defined",
                trait_def.name
            )));
        }

        let mut methods = HashMap::new();
        for method in &trait_def.methods {
            let method_info = TraitMethodInfo {
                name: method.name.clone(),
                params: method.params.iter().map(|p| p.ty.clone()).collect(),
                return_type: method.return_type.clone(),
                has_default: method.has_body,
            };
            methods.insert(method.name.clone(), method_info);
        }

        let trait_info = TraitInfo {
            name: trait_def.name.clone(),
            methods,
            type_params: trait_def.type_params.clone(),
        };

        self.traits.insert(trait_def.name.clone(), trait_info);
        Ok(())
    }

    /// Register an implementation
    pub fn register_impl(&mut self, impl_block: &ImplBlock) -> Result<()> {
        let trait_name = if let Some(trait_type) = &impl_block.trait_type {
            match trait_type {
                Type::Custom(name) => Some(name.clone()),
                _ => return Err(CompileError::Generic("Invalid trait type".to_string())),
            }
        } else {
            None
        };

        // Verify trait exists if this is a trait impl
        if let Some(ref tname) = trait_name {
            if !self.traits.contains_key(tname) {
                return Err(CompileError::Generic(format!(
                    "Trait '{}' not found",
                    tname
                )));
            }
        }

        let mut methods = HashMap::new();
        for method in &impl_block.methods {
            let method_impl = MethodImpl {
                name: method.name.clone(),
                type_params: method.type_params.clone(),
                is_generic: !method.type_params.is_empty(),
            };
            methods.insert(method.name.clone(), method_impl);
        }

        let impl_info = ImplInfo {
            trait_name,
            for_type: impl_block.for_type.clone(),
            methods,
        };

        let impl_index = self.impls.len();
        self.impls.push(impl_info);

        // Update type_impls mapping
        if let Some(type_name) = self.get_type_name(&impl_block.for_type) {
            self.type_impls
                .entry(type_name)
                .or_default()
                .push(impl_index);
        }

        Ok(())
    }

    /// Check if a type implements a trait
    #[allow(dead_code)]
    pub fn type_implements_trait(&self, ty: &Type, trait_name: &str) -> bool {
        if let Some(type_name) = self.get_type_name(ty) {
            if let Some(impl_indices) = self.type_impls.get(&type_name) {
                for &idx in impl_indices {
                    let impl_info = &self.impls[idx];
                    if impl_info.trait_name.as_ref() == Some(&trait_name.to_string()) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Find method implementation for a type
    #[allow(dead_code)]
    pub fn find_method(&self, ty: &Type, method_name: &str) -> Option<MethodResolution> {
        if let Some(type_name) = self.get_type_name(ty) {
            if let Some(impl_indices) = self.type_impls.get(&type_name) {
                // First check inherent methods
                for &idx in impl_indices {
                    let impl_info = &self.impls[idx];
                    if impl_info.trait_name.is_none() {
                        if let Some(method) = impl_info.methods.get(method_name) {
                            return Some(MethodResolution {
                                trait_name: None,
                                method_name: method.name.clone(),
                                is_generic: method.is_generic,
                            });
                        }
                    }
                }

                // Then check trait methods
                for &idx in impl_indices {
                    let impl_info = &self.impls[idx];
                    if let Some(ref trait_name) = impl_info.trait_name {
                        if let Some(method) = impl_info.methods.get(method_name) {
                            return Some(MethodResolution {
                                trait_name: Some(trait_name.clone()),
                                method_name: method.name.clone(),
                                is_generic: method.is_generic,
                            });
                        }
                    }
                }
            }
        }
        None
    }

    /// Get all traits implemented by a type
    #[allow(dead_code)]
    pub fn get_implemented_traits(&self, ty: &Type) -> Vec<String> {
        let mut traits = Vec::new();

        if let Some(type_name) = self.get_type_name(ty) {
            if let Some(impl_indices) = self.type_impls.get(&type_name) {
                for &idx in impl_indices {
                    let impl_info = &self.impls[idx];
                    if let Some(ref trait_name) = impl_info.trait_name {
                        traits.push(trait_name.clone());
                    }
                }
            }
        }

        traits
    }

    /// Check if all required trait methods are implemented
    pub fn check_trait_impl_complete(
        &self,
        impl_block: &ImplBlock,
        trait_name: &str,
    ) -> Result<()> {
        let trait_info = self
            .traits
            .get(trait_name)
            .ok_or_else(|| CompileError::Generic(format!("Trait '{}' not found", trait_name)))?;

        let impl_methods: HashMap<_, _> = impl_block
            .methods
            .iter()
            .map(|m| (m.name.clone(), m))
            .collect();

        // Check all required methods are implemented
        for (method_name, method_info) in &trait_info.methods {
            if !method_info.has_default && !impl_methods.contains_key(method_name) {
                return Err(CompileError::Generic(format!(
                    "Missing implementation for trait method '{}'",
                    method_name
                )));
            }
        }

        // Check no extra methods
        for method in &impl_block.methods {
            if !trait_info.methods.contains_key(&method.name) {
                return Err(CompileError::Generic(format!(
                    "Method '{}' is not a member of trait '{}'",
                    method.name, trait_name
                )));
            }
        }

        Ok(())
    }

    /// Get type name for lookup
    fn get_type_name(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::I32 => Some("i32".to_string()),
            Type::I64 => Some("i64".to_string()),
            Type::U32 => Some("u32".to_string()),
            Type::U64 => Some("u64".to_string()),
            Type::Bool => Some("bool".to_string()),
            Type::String => Some("String".to_string()),
            Type::Custom(name) => Some(name.clone()),
            Type::Generic { name, .. } => Some(name.clone()),
            _ => None,
        }
    }
}

/// Result of method resolution
#[derive(Debug, Clone)]
pub struct MethodResolution {
    #[allow(dead_code)]
    pub trait_name: Option<String>,
    #[allow(dead_code)]
    pub method_name: String,
    #[allow(dead_code)]
    pub is_generic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{TraitMethod, Visibility};
    use crate::errors::Span;

    #[test]
    fn test_trait_registration() {
        let mut resolver = TraitResolver::new();

        let trait_def = TraitDef {
            visibility: Visibility::Public,
            name: "Display".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            methods: vec![TraitMethod {
                name: "fmt".to_string(),
                lifetime_params: vec![],
                type_params: vec![],
                params: vec![],
                return_type: Some(Type::String),
                has_body: false,
                body: None,
                span: Span::dummy(),
            }],
            span: Span::dummy(),
        };

        assert!(resolver.register_trait(&trait_def).is_ok());
        assert!(resolver.traits.contains_key("Display"));
    }
}
