// Trait bounds checking for Palladium
// Handles validation of trait constraints on generic types

use crate::ast::{Type, Function, GenericParam};
use crate::errors::{CompileError, Result};
use std::collections::{HashMap, HashSet};

/// Trait bound on a type parameter
#[derive(Debug, Clone)]
pub struct TraitBound {
    pub type_param: String,
    pub trait_names: Vec<String>, // Multiple bounds like T: Display + Debug
}

/// Trait bounds for a generic item
#[derive(Debug, Clone)]
pub struct GenericBounds {
    pub bounds: HashMap<String, Vec<String>>, // Type param -> required traits
}

impl GenericBounds {
    pub fn new() -> Self {
        Self {
            bounds: HashMap::new(),
        }
    }
    
    /// Add a trait bound for a type parameter
    pub fn add_bound(&mut self, type_param: String, trait_name: String) {
        self.bounds
            .entry(type_param)
            .or_insert_with(Vec::new)
            .push(trait_name);
    }
    
    /// Check if a type parameter has a specific trait bound
    pub fn has_bound(&self, type_param: &str, trait_name: &str) -> bool {
        if let Some(bounds) = self.bounds.get(type_param) {
            bounds.contains(&trait_name.to_string())
        } else {
            false
        }
    }
    
    /// Get all bounds for a type parameter
    pub fn get_bounds(&self, type_param: &str) -> Vec<String> {
        self.bounds.get(type_param).cloned().unwrap_or_default()
    }
}

/// Parse trait bounds from function signature
pub fn parse_trait_bounds(func: &Function) -> GenericBounds {
    let mut bounds = GenericBounds::new();
    
    // For now, we'll parse bounds from a special naming convention
    // In the future, this will parse actual syntax like <T: Display>
    
    // Example: If we see a type param "T_Display_Debug", we know T has Display + Debug bounds
    for param in &func.type_params {
        if param.contains('_') {
            let parts: Vec<&str> = param.split('_').collect();
            if parts.len() > 1 {
                let type_param = parts[0].to_string();
                for i in 1..parts.len() {
                    bounds.add_bound(type_param.clone(), parts[i].to_string());
                }
            }
        }
    }
    
    bounds
}

/// Check if a concrete type satisfies trait bounds
pub fn check_bounds_satisfied(
    bounds: &GenericBounds,
    type_args: &HashMap<String, Type>,
    trait_impls: &dyn Fn(&Type, &str) -> bool,
) -> Result<()> {
    for (type_param, required_traits) in &bounds.bounds {
        if let Some(concrete_type) = type_args.get(type_param) {
            for trait_name in required_traits {
                if !trait_impls(concrete_type, trait_name) {
                    return Err(CompileError::Generic(format!(
                        "Type '{}' does not implement trait '{}'",
                        type_to_string(concrete_type),
                        trait_name
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Infer trait bounds from usage in function body
pub fn infer_trait_bounds(func: &Function) -> GenericBounds {
    let mut bounds = GenericBounds::new();
    
    // TODO: Analyze function body to infer required traits
    // For example:
    // - If we see x.fmt(), we know x's type needs Display
    // - If we see x + y, we know the type needs Add
    // - If we see x.clone(), we know the type needs Clone
    
    bounds
}

/// Merge two sets of bounds
pub fn merge_bounds(a: &GenericBounds, b: &GenericBounds) -> GenericBounds {
    let mut result = GenericBounds::new();
    
    // Add all bounds from a
    for (param, traits) in &a.bounds {
        for trait_name in traits {
            result.add_bound(param.clone(), trait_name.clone());
        }
    }
    
    // Add all bounds from b
    for (param, traits) in &b.bounds {
        for trait_name in traits {
            result.add_bound(param.clone(), trait_name.clone());
        }
    }
    
    // Deduplicate
    for bounds in result.bounds.values_mut() {
        bounds.sort();
        bounds.dedup();
    }
    
    result
}

fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::I64 => "i64".to_string(),
        Type::U32 => "u32".to_string(),
        Type::U64 => "u64".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "String".to_string(),
        Type::Unit => "()".to_string(),
        Type::Custom(name) => name.clone(),
        Type::Generic { name, args } => {
            let mut s = name.clone();
            if !args.is_empty() {
                s.push('<');
                // TODO: Format generic args
                s.push('>');
            }
            s
        }
        _ => "<type>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_trait_bounds() {
        let mut bounds = GenericBounds::new();
        bounds.add_bound("T".to_string(), "Display".to_string());
        bounds.add_bound("T".to_string(), "Debug".to_string());
        bounds.add_bound("U".to_string(), "Clone".to_string());
        
        assert!(bounds.has_bound("T", "Display"));
        assert!(bounds.has_bound("T", "Debug"));
        assert!(bounds.has_bound("U", "Clone"));
        assert!(!bounds.has_bound("T", "Clone"));
        
        let t_bounds = bounds.get_bounds("T");
        assert_eq!(t_bounds.len(), 2);
        assert!(t_bounds.contains(&"Display".to_string()));
        assert!(t_bounds.contains(&"Debug".to_string()));
    }
}