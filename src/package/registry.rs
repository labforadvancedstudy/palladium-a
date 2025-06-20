// Package registry for Palladium
// "The legendary package archive"

use crate::errors::{CompileError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::dependency::{Package, Version};
use super::PackageManifest;

/// Registry metadata for a package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPackage {
    pub name: String,
    pub versions: Vec<VersionInfo>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

/// Information about a specific version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub checksum: String,
    pub dependencies: HashMap<String, String>,
    pub yanked: bool,
    pub published_at: String,
    pub download_url: String,
}

/// Package registry client
pub struct RegistryClient {
    /// Registry base URL
    base_url: String,
    /// Local cache directory
    cache_dir: PathBuf,
    /// HTTP client (simulated for now)
    _client: HttpClient,
}

impl RegistryClient {
    pub fn new(base_url: String, cache_dir: PathBuf) -> Result<Self> {
        // Ensure cache directory exists
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(CompileError::IoError)?;
        }

        Ok(Self {
            base_url,
            cache_dir,
            _client: HttpClient::new(),
        })
    }

    /// Search for packages by name
    pub fn search(&self, query: &str) -> Result<Vec<RegistryPackage>> {
        let _url = format!("{}/api/v1/search?q={}", self.base_url, query);
        
        // For now, return mock data
        // TODO: Implement actual HTTP request
        Ok(vec![
            RegistryPackage {
                name: "http".to_string(),
                versions: vec![
                    VersionInfo {
                        version: "1.0.0".to_string(),
                        checksum: "abc123".to_string(),
                        dependencies: HashMap::new(),
                        yanked: false,
                        published_at: "2024-01-01T00:00:00Z".to_string(),
                        download_url: format!("{}/packages/http/1.0.0.tar.gz", self.base_url),
                    },
                    VersionInfo {
                        version: "1.1.0".to_string(),
                        checksum: "def456".to_string(),
                        dependencies: HashMap::new(),
                        yanked: false,
                        published_at: "2024-02-01T00:00:00Z".to_string(),
                        download_url: format!("{}/packages/http/1.1.0.tar.gz", self.base_url),
                    },
                ],
                description: Some("HTTP client and server library".to_string()),
                homepage: Some("https://github.com/palladium/http".to_string()),
                repository: Some("https://github.com/palladium/http".to_string()),
                keywords: vec!["http".to_string(), "network".to_string()],
                categories: vec!["network".to_string()],
            },
        ])
    }

    /// Get package information
    pub fn get_package(&self, name: &str) -> Result<RegistryPackage> {
        let _url = format!("{}/api/v1/packages/{}", self.base_url, name);
        
        // Check cache first
        let cache_file = self.cache_dir.join(format!("{}.json", name));
        if cache_file.exists() {
            let content = fs::read_to_string(&cache_file).map_err(CompileError::IoError)?;
            let package: RegistryPackage = serde_json::from_str(&content)
                .map_err(|e| CompileError::Generic(format!("Failed to parse cache: {}", e)))?;
            return Ok(package);
        }

        // TODO: Implement actual HTTP request
        Err(CompileError::Generic(format!(
            "Package '{}' not found in registry",
            name
        )))
    }

    /// Download a specific version of a package
    pub fn download_package(&self, name: &str, version: &str) -> Result<PathBuf> {
        let package = self.get_package(name)?;
        
        let version_info = package
            .versions
            .iter()
            .find(|v| v.version == version)
            .ok_or_else(|| {
                CompileError::Generic(format!("Version {} not found for package {}", version, name))
            })?;

        if version_info.yanked {
            return Err(CompileError::Generic(format!(
                "Version {} of package {} has been yanked",
                version, name
            )));
        }

        // Download to cache
        let package_dir = self.cache_dir.join(name).join(version);
        if package_dir.exists() {
            // Already downloaded
            return Ok(package_dir);
        }

        // Create directory
        fs::create_dir_all(&package_dir).map_err(CompileError::IoError)?;

        // TODO: Actually download and extract the package
        // For now, create a mock package.pd file
        let manifest = PackageManifest {
            name: name.to_string(),
            version: version.to_string(),
            description: package.description.clone(),
            authors: vec!["Registry Author".to_string()],
            license: Some("MIT".to_string()),
            dependencies: version_info
                .dependencies
                .iter()
                .map(|(k, v)| (k.clone(), super::Dependency::Version(v.clone())))
                .collect(),
            dev_dependencies: HashMap::new(),
            build_dependencies: HashMap::new(),
            main: Some("src/main.pd".to_string()),
            lib: None,
            bin: vec![],
            examples: vec![],
            tests: vec![],
        };

        let manifest_path = package_dir.join("package.pd");
        let manifest_content = super::PackageManager::manifest_to_string(&manifest);
        fs::write(&manifest_path, manifest_content).map_err(CompileError::IoError)?;

        // Create source files
        let src_dir = package_dir.join("src");
        fs::create_dir_all(&src_dir).map_err(CompileError::IoError)?;
        
        let main_content = format!(
            "// {} v{}\n// {}\n\nfn main() {{\n    print(\"Hello from {}!\");\n}}\n",
            name,
            version,
            package.description.as_deref().unwrap_or(""),
            name
        );
        fs::write(src_dir.join("main.pd"), main_content).map_err(CompileError::IoError)?;

        Ok(package_dir)
    }

    /// Publish a package to the registry
    pub fn publish(&self, manifest_path: &Path) -> Result<()> {
        let manifest = super::PackageManager::load_manifest(manifest_path)?;
        
        println!("📦 Publishing {} v{}", manifest.name, manifest.version);
        
        // Validate package
        self.validate_package(&manifest)?;
        
        // Create package archive
        let archive_path = self.create_package_archive(&manifest, manifest_path.parent().unwrap())?;
        
        // Upload to registry
        let url = format!("{}/api/v1/publish", self.base_url);
        
        // TODO: Implement actual upload
        println!("📤 Uploading to {}...", url);
        println!("✅ Package published successfully!");
        
        // Clean up
        fs::remove_file(&archive_path).ok();
        
        Ok(())
    }

    /// Validate a package before publishing
    fn validate_package(&self, manifest: &PackageManifest) -> Result<()> {
        // Check required fields
        if manifest.name.is_empty() {
            return Err(CompileError::Generic("Package name is required".to_string()));
        }
        
        if manifest.version.is_empty() {
            return Err(CompileError::Generic("Package version is required".to_string()));
        }
        
        // Validate version format
        Version::parse(&manifest.version)?;
        
        // Check for reserved names
        let reserved_names = ["std", "core", "builtin", "palladium"];
        if reserved_names.contains(&manifest.name.as_str()) {
            return Err(CompileError::Generic(format!(
                "Package name '{}' is reserved",
                manifest.name
            )));
        }
        
        // Validate package name format (lowercase, alphanumeric, hyphens)
        if !manifest.name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(CompileError::Generic(
                "Package name must contain only alphanumeric characters, hyphens, and underscores".to_string()
            ));
        }
        
        Ok(())
    }

    /// Create a package archive for publishing
    fn create_package_archive(&self, manifest: &PackageManifest, _source_dir: &Path) -> Result<PathBuf> {
        let archive_name = format!("{}-{}.tar.gz", manifest.name, manifest.version);
        let archive_path = self.cache_dir.join(&archive_name);
        
        // TODO: Actually create tar.gz archive
        // For now, just create an empty file
        fs::write(&archive_path, b"mock archive").map_err(CompileError::IoError)?;
        
        Ok(archive_path)
    }

    /// Get all available packages (for dependency resolution)
    pub fn get_all_packages(&self) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        
        // TODO: Implement actual registry query
        // For now, return some mock packages
        packages.push(Package {
            name: "http".to_string(),
            version: Version::parse("1.0.0")?,
            dependencies: HashMap::new(),
        });
        
        packages.push(Package {
            name: "http".to_string(),
            version: Version::parse("1.1.0")?,
            dependencies: HashMap::new(),
        });
        
        packages.push(Package {
            name: "json".to_string(),
            version: Version::parse("2.0.0")?,
            dependencies: HashMap::new(),
        });
        
        packages.push(Package {
            name: "xml".to_string(),
            version: Version::parse("1.0.0")?,
            dependencies: {
                let mut deps = HashMap::new();
                deps.insert(
                    "http".to_string(),
                    super::dependency::VersionRequirement::parse("^1.0.0")?,
                );
                deps
            },
        });
        
        Ok(packages)
    }
}

/// Mock HTTP client (will be replaced with actual implementation)
struct HttpClient;

impl HttpClient {
    fn new() -> Self {
        Self
    }
    
    // TODO: Implement actual HTTP methods
    // fn get(&self, url: &str) -> Result<String>
    // fn post(&self, url: &str, body: &[u8]) -> Result<String>
}

/// Local registry for offline development
pub struct LocalRegistry {
    root_dir: PathBuf,
    packages: HashMap<String, RegistryPackage>,
}

impl LocalRegistry {
    pub fn new(root_dir: PathBuf) -> Result<Self> {
        if !root_dir.exists() {
            fs::create_dir_all(&root_dir).map_err(CompileError::IoError)?;
        }
        
        let mut registry = Self {
            root_dir,
            packages: HashMap::new(),
        };
        
        registry.load_packages()?;
        Ok(registry)
    }
    
    /// Load all packages from the local registry
    fn load_packages(&mut self) -> Result<()> {
        let index_path = self.root_dir.join("index.json");
        if index_path.exists() {
            let content = fs::read_to_string(&index_path).map_err(CompileError::IoError)?;
            self.packages = serde_json::from_str(&content)
                .map_err(|e| CompileError::Generic(format!("Failed to parse index: {}", e)))?;
        }
        Ok(())
    }
    
    /// Save the package index
    fn save_index(&self) -> Result<()> {
        let index_path = self.root_dir.join("index.json");
        let content = serde_json::to_string_pretty(&self.packages)
            .map_err(|e| CompileError::Generic(format!("Failed to serialize index: {}", e)))?;
        fs::write(&index_path, content).map_err(CompileError::IoError)?;
        Ok(())
    }
    
    /// Add a package to the local registry
    pub fn add_package(&mut self, manifest_path: &Path) -> Result<()> {
        let manifest = super::PackageManager::load_manifest(manifest_path)?;
        let _source_dir = manifest_path.parent().unwrap();
        
        // Create package directory
        let package_dir = self
            .root_dir
            .join(&manifest.name)
            .join(&manifest.version);
        
        if package_dir.exists() {
            return Err(CompileError::Generic(format!(
                "Package {} v{} already exists in local registry",
                manifest.name, manifest.version
            )));
        }
        
        // Copy package files
        fs::create_dir_all(&package_dir).map_err(CompileError::IoError)?;
        
        // TODO: Actually copy all package files
        // For now, just copy the manifest
        let dest_manifest = package_dir.join("package.pd");
        fs::copy(manifest_path, &dest_manifest).map_err(CompileError::IoError)?;
        
        // Update index
        let version_info = VersionInfo {
            version: manifest.version.clone(),
            checksum: "local".to_string(),
            dependencies: manifest
                .dependencies
                .iter()
                .map(|(k, v)| match v {
                    super::Dependency::Version(s) => (k.clone(), s.clone()),
                    _ => (k.clone(), "*".to_string()),
                })
                .collect(),
            yanked: false,
            published_at: chrono::Utc::now().to_rfc3339(),
            download_url: format!("file://{}", package_dir.display()),
        };
        
        self.packages
            .entry(manifest.name.clone())
            .or_insert_with(|| RegistryPackage {
                name: manifest.name.clone(),
                versions: vec![],
                description: manifest.description.clone(),
                homepage: None,
                repository: None,
                keywords: vec![],
                categories: vec![],
            })
            .versions
            .push(version_info);
        
        self.save_index()?;
        
        println!(
            "✅ Added {} v{} to local registry",
            manifest.name, manifest.version
        );
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_local_registry() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path().to_path_buf()).unwrap();
        
        // Create a test manifest
        let manifest_dir = temp_dir.path().join("test_package");
        fs::create_dir(&manifest_dir).unwrap();
        
        let manifest = PackageManifest {
            name: "test_package".to_string(),
            version: "0.1.0".to_string(),
            description: Some("Test package".to_string()),
            authors: vec!["Test Author".to_string()],
            license: Some("MIT".to_string()),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
            build_dependencies: HashMap::new(),
            main: None,
            lib: None,
            bin: vec![],
            examples: vec![],
            tests: vec![],
        };
        
        let manifest_path = manifest_dir.join("package.pd");
        let content = super::super::PackageManager::manifest_to_string(&manifest);
        fs::write(&manifest_path, content).unwrap();
        
        // Add to registry
        registry.add_package(&manifest_path).unwrap();
        
        // Check that it was added
        assert!(registry.packages.contains_key("test_package"));
        assert_eq!(registry.packages["test_package"].versions.len(), 1);
        assert_eq!(
            registry.packages["test_package"].versions[0].version,
            "0.1.0"
        );
    }
}