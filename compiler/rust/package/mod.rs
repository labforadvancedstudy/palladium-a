// Package manager for Palladium
// "Managing legends, one package at a time"

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use crate::errors::{CompileError, Result};

/// Package manifest structure (package.pd)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    /// Package name
    pub name: String,
    
    /// Package version (semver)
    pub version: String,
    
    /// Package description
    pub description: Option<String>,
    
    /// Package authors
    pub authors: Vec<String>,
    
    /// Package license
    pub license: Option<String>,
    
    /// Package dependencies
    pub dependencies: HashMap<String, Dependency>,
    
    /// Dev dependencies (for tests/examples)
    pub dev_dependencies: HashMap<String, Dependency>,
    
    /// Build dependencies (for build scripts)
    pub build_dependencies: HashMap<String, Dependency>,
    
    /// Entry point (defaults to src/main.pd)
    pub main: Option<String>,
    
    /// Library entry point (defaults to src/lib.pd)
    pub lib: Option<String>,
    
    /// Binary targets
    pub bin: Vec<BinaryTarget>,
    
    /// Example targets
    pub examples: Vec<ExampleTarget>,
    
    /// Test targets
    pub tests: Vec<TestTarget>,
}

/// Dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    /// Simple version string
    Version(String),
    
    /// Detailed dependency
    Detailed {
        version: Option<String>,
        path: Option<String>,
        git: Option<String>,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        features: Vec<String>,
        optional: bool,
    },
}

/// Binary target specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryTarget {
    pub name: String,
    pub path: String,
}

/// Example target specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleTarget {
    pub name: String,
    pub path: String,
}

/// Test target specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTarget {
    pub name: String,
    pub path: String,
}

/// Package manager
pub struct PackageManager {
    /// Cache directory for downloaded packages
    cache_dir: PathBuf,
    
    /// Registry URL
    registry_url: String,
    
    /// Loaded package manifests
    manifests: HashMap<String, PackageManifest>,
}

impl PackageManager {
    /// Create a new package manager
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| CompileError::Generic("Could not find home directory".to_string()))?;
        
        let cache_dir = home_dir.join(".palladium").join("cache");
        
        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .map_err(|e| CompileError::IoError(e))?;
        }
        
        Ok(Self {
            cache_dir,
            registry_url: "https://packages.palladium-lang.org".to_string(),
            manifests: HashMap::new(),
        })
    }
    
    /// Load package manifest from a file
    pub fn load_manifest(path: &Path) -> Result<PackageManifest> {
        let content = fs::read_to_string(path)
            .map_err(|e| CompileError::IoError(e))?;
        
        // For now, we'll parse a simple format
        // In the future, this could be TOML or a custom format
        Self::parse_manifest(&content)
    }
    
    /// Parse manifest from string
    fn parse_manifest(content: &str) -> Result<PackageManifest> {
        // Simple parser for package.pd format
        // Format example:
        // name = "my_package"
        // version = "0.1.0"
        // description = "A cool package"
        // authors = ["John Doe <john@example.com>"]
        // 
        // [dependencies]
        // std = "1.0"
        // http = { version = "0.2", features = ["client"] }
        
        let mut manifest = PackageManifest {
            name: String::new(),
            version: String::new(),
            description: None,
            authors: Vec::new(),
            license: None,
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
            build_dependencies: HashMap::new(),
            main: None,
            lib: None,
            bin: Vec::new(),
            examples: Vec::new(),
            tests: Vec::new(),
        };
        
        let mut current_section = "";
        
        for line in content.lines() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
                continue;
            }
            
            // Check for section headers
            if line.starts_with('[') && line.ends_with(']') {
                current_section = &line[1..line.len()-1];
                continue;
            }
            
            // Parse key-value pairs
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos+1..].trim();
                
                match current_section {
                    "" => {
                        // Top-level fields
                        match key {
                            "name" => manifest.name = Self::parse_string(value)?,
                            "version" => manifest.version = Self::parse_string(value)?,
                            "description" => manifest.description = Some(Self::parse_string(value)?),
                            "license" => manifest.license = Some(Self::parse_string(value)?),
                            "main" => manifest.main = Some(Self::parse_string(value)?),
                            "lib" => manifest.lib = Some(Self::parse_string(value)?),
                            "authors" => manifest.authors = Self::parse_string_array(value)?,
                            _ => {} // Ignore unknown fields
                        }
                    }
                    "dependencies" => {
                        let dep = Self::parse_dependency(value)?;
                        manifest.dependencies.insert(key.to_string(), dep);
                    }
                    "dev-dependencies" => {
                        let dep = Self::parse_dependency(value)?;
                        manifest.dev_dependencies.insert(key.to_string(), dep);
                    }
                    "build-dependencies" => {
                        let dep = Self::parse_dependency(value)?;
                        manifest.build_dependencies.insert(key.to_string(), dep);
                    }
                    _ => {} // Ignore unknown sections
                }
            }
        }
        
        // Validate required fields
        if manifest.name.is_empty() {
            return Err(CompileError::Generic("Package name is required".to_string()));
        }
        if manifest.version.is_empty() {
            return Err(CompileError::Generic("Package version is required".to_string()));
        }
        
        Ok(manifest)
    }
    
    /// Parse a quoted string
    fn parse_string(value: &str) -> Result<String> {
        if value.starts_with('"') && value.ends_with('"') {
            Ok(value[1..value.len()-1].to_string())
        } else {
            Err(CompileError::Generic(format!("Expected quoted string, got: {}", value)))
        }
    }
    
    /// Parse an array of strings
    fn parse_string_array(value: &str) -> Result<Vec<String>> {
        if value.starts_with('[') && value.ends_with(']') {
            let inner = &value[1..value.len()-1];
            let mut result = Vec::new();
            
            for item in inner.split(',') {
                let item = item.trim();
                if !item.is_empty() {
                    result.push(Self::parse_string(item)?);
                }
            }
            
            Ok(result)
        } else {
            Err(CompileError::Generic(format!("Expected array, got: {}", value)))
        }
    }
    
    /// Parse a dependency specification
    fn parse_dependency(value: &str) -> Result<Dependency> {
        if value.starts_with('"') && value.ends_with('"') {
            // Simple version string
            Ok(Dependency::Version(value[1..value.len()-1].to_string()))
        } else if value.starts_with('{') && value.ends_with('}') {
            // Detailed dependency
            // For now, just return a simple version
            // TODO: Implement proper parsing
            Ok(Dependency::Version("*".to_string()))
        } else {
            Err(CompileError::Generic(format!("Invalid dependency format: {}", value)))
        }
    }
    
    /// Initialize a new package in the current directory
    pub fn init(name: &str, path: &Path) -> Result<()> {
        // Create directory structure
        let src_dir = path.join("src");
        if !src_dir.exists() {
            fs::create_dir_all(&src_dir)
                .map_err(|e| CompileError::IoError(e))?;
        }
        
        // Create default package.pd
        let manifest = PackageManifest {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: Some(format!("A new Palladium package")),
            authors: vec![Self::get_default_author()],
            license: Some("MIT".to_string()),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
            build_dependencies: HashMap::new(),
            main: None,
            lib: None,
            bin: Vec::new(),
            examples: Vec::new(),
            tests: Vec::new(),
        };
        
        let manifest_path = path.join("package.pd");
        let manifest_content = Self::manifest_to_string(&manifest);
        fs::write(&manifest_path, manifest_content)
            .map_err(|e| CompileError::IoError(e))?;
        
        // Create default main.pd
        let main_path = src_dir.join("main.pd");
        let main_content = r#"// Entry point for the package

fn main() {
    print("Hello from {}!\n");
}
"#.replace("{}", name);
        
        fs::write(&main_path, main_content)
            .map_err(|e| CompileError::IoError(e))?;
        
        println!("✅ Created package '{}' at {}", name, path.display());
        
        Ok(())
    }
    
    /// Get default author from git config or environment
    fn get_default_author() -> String {
        // Try to get from git config
        if let Ok(output) = std::process::Command::new("git")
            .args(&["config", "--global", "user.name"])
            .output()
        {
            if output.status.success() {
                if let Ok(name) = String::from_utf8(output.stdout) {
                    let name = name.trim();
                    
                    // Also try to get email
                    if let Ok(email_output) = std::process::Command::new("git")
                        .args(&["config", "--global", "user.email"])
                        .output()
                    {
                        if email_output.status.success() {
                            if let Ok(email) = String::from_utf8(email_output.stdout) {
                                let email = email.trim();
                                return format!("{} <{}>", name, email);
                            }
                        }
                    }
                    
                    return name.to_string();
                }
            }
        }
        
        // Fall back to environment
        if let Ok(user) = std::env::var("USER") {
            return user;
        }
        
        "Unknown Author".to_string()
    }
    
    /// Convert manifest to string format
    fn manifest_to_string(manifest: &PackageManifest) -> String {
        let mut result = String::new();
        
        // Basic fields
        result.push_str(&format!("name = \"{}\"\n", manifest.name));
        result.push_str(&format!("version = \"{}\"\n", manifest.version));
        
        if let Some(desc) = &manifest.description {
            result.push_str(&format!("description = \"{}\"\n", desc));
        }
        
        if !manifest.authors.is_empty() {
            result.push_str("authors = [");
            for (i, author) in manifest.authors.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format!("\"{}\"", author));
            }
            result.push_str("]\n");
        }
        
        if let Some(license) = &manifest.license {
            result.push_str(&format!("license = \"{}\"\n", license));
        }
        
        // Dependencies
        if !manifest.dependencies.is_empty() {
            result.push_str("\n[dependencies]\n");
            for (name, dep) in &manifest.dependencies {
                match dep {
                    Dependency::Version(v) => {
                        result.push_str(&format!("{} = \"{}\"\n", name, v));
                    }
                    Dependency::Detailed { .. } => {
                        // TODO: Implement detailed format
                        result.push_str(&format!("{} = \"*\"\n", name));
                    }
                }
            }
        }
        
        // Dev dependencies
        if !manifest.dev_dependencies.is_empty() {
            result.push_str("\n[dev-dependencies]\n");
            for (name, dep) in &manifest.dev_dependencies {
                match dep {
                    Dependency::Version(v) => {
                        result.push_str(&format!("{} = \"{}\"\n", name, v));
                    }
                    Dependency::Detailed { .. } => {
                        result.push_str(&format!("{} = \"*\"\n", name));
                    }
                }
            }
        }
        
        result
    }
    
    /// Add a dependency to the current package
    pub fn add_dependency(&mut self, name: &str, version: &str, dev: bool) -> Result<()> {
        // Load current manifest
        let manifest_path = Path::new("package.pd");
        let mut manifest = Self::load_manifest(&manifest_path)?;
        
        // Add dependency
        let dep = Dependency::Version(version.to_string());
        if dev {
            manifest.dev_dependencies.insert(name.to_string(), dep);
            println!("➕ Added dev dependency: {} = \"{}\"", name, version);
        } else {
            manifest.dependencies.insert(name.to_string(), dep);
            println!("➕ Added dependency: {} = \"{}\"", name, version);
        }
        
        // Save manifest
        let content = Self::manifest_to_string(&manifest);
        fs::write(&manifest_path, content)
            .map_err(|e| CompileError::IoError(e))?;
        
        Ok(())
    }
    
    /// Build the current package
    pub fn build(&self, release: bool) -> Result<()> {
        // Load manifest
        let manifest_path = Path::new("package.pd");
        let manifest = Self::load_manifest(&manifest_path)?;
        
        println!("🔨 Building package '{}'...", manifest.name);
        
        // Determine entry point
        let entry = manifest.main.as_deref().unwrap_or("src/main.pd");
        let entry_path = Path::new(entry);
        
        if !entry_path.exists() {
            return Err(CompileError::Generic(format!(
                "Entry point '{}' not found", entry
            )));
        }
        
        // Use the driver to compile
        let driver = if release {
            crate::Driver::new() // TODO: Add optimization flags
        } else {
            crate::Driver::new()
        };
        
        // Create build directory
        let build_dir = Path::new("target").join(if release { "release" } else { "debug" });
        if !build_dir.exists() {
            fs::create_dir_all(&build_dir)
                .map_err(|e| CompileError::IoError(e))?;
        }
        
        // Compile the package
        let output = driver.compile_file(&entry_path)?;
        
        // Move output to target directory
        let target_name = format!("{}.c", manifest.name);
        let target_path = build_dir.join(&target_name);
        fs::rename(&output, &target_path)
            .map_err(|e| CompileError::IoError(e))?;
        
        println!("✅ Build complete: {}", target_path.display());
        
        Ok(())
    }
    
    /// Run the current package
    pub fn run(&self, args: Vec<String>, release: bool) -> Result<()> {
        // First build
        self.build(release)?;
        
        // Load manifest to get package name
        let manifest = Self::load_manifest(Path::new("package.pd"))?;
        
        // Find the built executable
        let build_dir = Path::new("target").join(if release { "release" } else { "debug" });
        let c_file = build_dir.join(format!("{}.c", manifest.name));
        let exe_file = build_dir.join(&manifest.name);
        
        // Compile C to executable
        println!("🔗 Linking executable...");
        let gcc_output = std::process::Command::new("gcc")
            .arg(&c_file)
            .arg("-o")
            .arg(&exe_file)
            .output()
            .map_err(|e| CompileError::Generic(format!("Failed to run gcc: {}", e)))?;
        
        if !gcc_output.status.success() {
            let stderr = String::from_utf8_lossy(&gcc_output.stderr);
            return Err(CompileError::Generic(format!(
                "gcc compilation failed:\n{}", stderr
            )));
        }
        
        // Run the executable
        println!("🚀 Running '{}'...", manifest.name);
        println!("─────────────────────────────────────");
        
        let mut cmd = std::process::Command::new(&exe_file);
        cmd.args(&args);
        
        let status = cmd.status()
            .map_err(|e| CompileError::Generic(format!("Failed to run program: {}", e)))?;
        
        println!("─────────────────────────────────────");
        
        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);
            println!("⚠️  Program exited with code: {}", exit_code);
        } else {
            println!("✅ Program completed successfully");
        }
        
        Ok(())
    }
}

impl Default for PackageManager {
    fn default() -> Self {
        Self::new().expect("Failed to create package manager")
    }
}