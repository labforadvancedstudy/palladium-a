// Lockfile management for Palladium package manager
// "Locking down the legendary dependencies"

use crate::errors::{CompileError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Lockfile format for reproducible builds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version
    pub version: u32,
    
    /// Root package information
    pub root: RootPackage,
    
    /// Resolved packages
    pub packages: Vec<LockedPackage>,
    
    /// Metadata
    pub metadata: LockfileMetadata,
}

/// Root package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootPackage {
    pub name: String,
    pub version: String,
}

/// Locked package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: PackageSource,
    pub dependencies: Vec<String>,
    pub checksum: String,
}

/// Package source information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PackageSource {
    Registry {
        url: String,
    },
    Git {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rev: Option<String>,
    },
    Path {
        path: String,
    },
    Local {
        path: String,
    },
}

/// Lockfile metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockfileMetadata {
    /// Timestamp of lockfile creation
    pub created_at: String,
    
    /// Palladium version used to create the lockfile
    pub palladium_version: String,
    
    /// Platform information
    pub platform: PlatformInfo,
}

/// Platform information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
}

impl Lockfile {
    /// Current lockfile format version
    const CURRENT_VERSION: u32 = 1;
    
    /// Create a new lockfile
    pub fn new(root_name: &str, root_version: &str) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            root: RootPackage {
                name: root_name.to_string(),
                version: root_version.to_string(),
            },
            packages: Vec::new(),
            metadata: LockfileMetadata {
                created_at: chrono::Utc::now().to_rfc3339(),
                palladium_version: env!("CARGO_PKG_VERSION").to_string(),
                platform: PlatformInfo {
                    os: std::env::consts::OS.to_string(),
                    arch: std::env::consts::ARCH.to_string(),
                },
            },
        }
    }
    
    /// Load lockfile from disk
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(CompileError::IoError)?;
        
        // Parse as TOML
        let lockfile: Self = toml::from_str(&content)
            .map_err(|e| CompileError::Generic(format!("Failed to parse lockfile: {}", e)))?;
        
        // Validate version
        if lockfile.version > Self::CURRENT_VERSION {
            return Err(CompileError::Generic(format!(
                "Lockfile version {} is newer than supported version {}",
                lockfile.version,
                Self::CURRENT_VERSION
            )));
        }
        
        Ok(lockfile)
    }
    
    /// Save lockfile to disk
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| CompileError::Generic(format!("Failed to serialize lockfile: {}", e)))?;
        
        fs::write(path, content).map_err(CompileError::IoError)?;
        Ok(())
    }
    
    /// Add a locked package
    pub fn add_package(&mut self, package: LockedPackage) {
        // Remove existing version if any
        self.packages.retain(|p| p.name != package.name);
        self.packages.push(package);
        
        // Sort packages for deterministic output
        self.packages.sort_by(|a, b| a.name.cmp(&b.name));
    }
    
    /// Get a locked package by name
    pub fn get_package(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }
    
    /// Check if lockfile is up to date with manifest
    pub fn is_up_to_date(&self, manifest: &super::PackageManifest) -> bool {
        // Check root package
        if self.root.name != manifest.name || self.root.version != manifest.version {
            return false;
        }
        
        // Check if all manifest dependencies are in lockfile
        for (dep_name, _dep_spec) in &manifest.dependencies {
            if let Some(locked) = self.get_package(dep_name) {
                // TODO: Check if locked version satisfies dependency spec
                let _ = locked; // Placeholder
            } else {
                return false;
            }
        }
        
        true
    }
    
    /// Generate a dependency tree for display
    pub fn dependency_tree(&self) -> String {
        let mut tree = String::new();
        tree.push_str(&format!("{} v{}\n", self.root.name, self.root.version));
        
        // Build dependency map
        let mut dep_map: HashMap<String, Vec<String>> = HashMap::new();
        for package in &self.packages {
            for dep in &package.dependencies {
                dep_map.entry(self.root.name.clone()).or_default().push(dep.clone());
            }
        }
        
        // Print tree
        self.print_deps(&self.root.name, &dep_map, &mut tree, "", true);
        
        tree
    }
    
    /// Recursively print dependencies
    fn print_deps(
        &self,
        package: &str,
        dep_map: &HashMap<String, Vec<String>>,
        output: &mut String,
        prefix: &str,
        _is_last: bool,
    ) {
        if let Some(deps) = dep_map.get(package) {
            for (i, dep) in deps.iter().enumerate() {
                let is_last_dep = i == deps.len() - 1;
                let connector = if is_last_dep { "└── " } else { "├── " };
                let extension = if is_last_dep { "    " } else { "│   " };
                
                if let Some(locked) = self.get_package(dep) {
                    output.push_str(&format!(
                        "{}{}{} v{}\n",
                        prefix, connector, locked.name, locked.version
                    ));
                    
                    let new_prefix = format!("{}{}", prefix, extension);
                    self.print_deps(&locked.name, dep_map, output, &new_prefix, is_last_dep);
                }
            }
        }
    }
    
    /// Verify checksums of all packages
    pub fn verify_checksums(&self, cache_dir: &Path) -> Result<()> {
        for package in &self.packages {
            let package_dir = cache_dir.join(&package.name).join(&package.version);
            
            if !package_dir.exists() {
                return Err(CompileError::Generic(format!(
                    "Package {} v{} not found in cache",
                    package.name, package.version
                )));
            }
            
            // TODO: Actually compute and verify checksum
            // For now, just check that the directory exists
        }
        
        Ok(())
    }
}

/// Lockfile diff for showing what changed
pub struct LockfileDiff {
    pub added: Vec<LockedPackage>,
    pub removed: Vec<LockedPackage>,
    pub updated: Vec<(LockedPackage, LockedPackage)>, // (old, new)
}

impl LockfileDiff {
    /// Compute diff between two lockfiles
    pub fn compute(old: &Lockfile, new: &Lockfile) -> Self {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut updated = Vec::new();
        
        // Find added and updated packages
        for new_pkg in &new.packages {
            if let Some(old_pkg) = old.packages.iter().find(|p| p.name == new_pkg.name) {
                if old_pkg.version != new_pkg.version {
                    updated.push((old_pkg.clone(), new_pkg.clone()));
                }
            } else {
                added.push(new_pkg.clone());
            }
        }
        
        // Find removed packages
        for old_pkg in &old.packages {
            if !new.packages.iter().any(|p| p.name == old_pkg.name) {
                removed.push(old_pkg.clone());
            }
        }
        
        Self {
            added,
            removed,
            updated,
        }
    }
    
    /// Display the diff in a human-readable format
    pub fn display(&self) -> String {
        let mut output = String::new();
        
        if !self.added.is_empty() {
            output.push_str("Added:\n");
            for pkg in &self.added {
                output.push_str(&format!("  + {} v{}\n", pkg.name, pkg.version));
            }
        }
        
        if !self.removed.is_empty() {
            output.push_str("Removed:\n");
            for pkg in &self.removed {
                output.push_str(&format!("  - {} v{}\n", pkg.name, pkg.version));
            }
        }
        
        if !self.updated.is_empty() {
            output.push_str("Updated:\n");
            for (old, new) in &self.updated {
                output.push_str(&format!(
                    "  ~ {} v{} -> v{}\n",
                    old.name, old.version, new.version
                ));
            }
        }
        
        if output.is_empty() {
            output.push_str("No changes\n");
        }
        
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_lockfile_creation() {
        let lockfile = Lockfile::new("test_package", "1.0.0");
        assert_eq!(lockfile.version, Lockfile::CURRENT_VERSION);
        assert_eq!(lockfile.root.name, "test_package");
        assert_eq!(lockfile.root.version, "1.0.0");
        assert!(lockfile.packages.is_empty());
    }
    
    #[test]
    fn test_lockfile_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let lockfile_path = temp_dir.path().join("package.lock");
        
        let mut lockfile = Lockfile::new("test_package", "1.0.0");
        lockfile.add_package(LockedPackage {
            name: "http".to_string(),
            version: "1.1.0".to_string(),
            source: PackageSource::Registry {
                url: "https://packages.palladium-lang.org".to_string(),
            },
            dependencies: vec![],
            checksum: "abc123".to_string(),
        });
        
        // Save
        lockfile.save(&lockfile_path).unwrap();
        assert!(lockfile_path.exists());
        
        // Load
        let loaded = Lockfile::load(&lockfile_path).unwrap();
        assert_eq!(loaded.root.name, "test_package");
        assert_eq!(loaded.packages.len(), 1);
        assert_eq!(loaded.packages[0].name, "http");
        assert_eq!(loaded.packages[0].version, "1.1.0");
    }
    
    #[test]
    fn test_lockfile_diff() {
        let mut old = Lockfile::new("test", "1.0.0");
        old.add_package(LockedPackage {
            name: "http".to_string(),
            version: "1.0.0".to_string(),
            source: PackageSource::Registry {
                url: "https://packages.palladium-lang.org".to_string(),
            },
            dependencies: vec![],
            checksum: "old".to_string(),
        });
        old.add_package(LockedPackage {
            name: "json".to_string(),
            version: "1.0.0".to_string(),
            source: PackageSource::Registry {
                url: "https://packages.palladium-lang.org".to_string(),
            },
            dependencies: vec![],
            checksum: "old".to_string(),
        });
        
        let mut new = Lockfile::new("test", "1.0.0");
        new.add_package(LockedPackage {
            name: "http".to_string(),
            version: "1.1.0".to_string(), // Updated
            source: PackageSource::Registry {
                url: "https://packages.palladium-lang.org".to_string(),
            },
            dependencies: vec![],
            checksum: "new".to_string(),
        });
        new.add_package(LockedPackage {
            name: "xml".to_string(), // Added
            version: "2.0.0".to_string(),
            source: PackageSource::Registry {
                url: "https://packages.palladium-lang.org".to_string(),
            },
            dependencies: vec![],
            checksum: "new".to_string(),
        });
        // json removed
        
        let diff = LockfileDiff::compute(&old, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "xml");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "json");
        assert_eq!(diff.updated.len(), 1);
        assert_eq!(diff.updated[0].0.name, "http");
        assert_eq!(diff.updated[0].0.version, "1.0.0");
        assert_eq!(diff.updated[0].1.version, "1.1.0");
    }
}