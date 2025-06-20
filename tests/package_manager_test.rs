use palladium::package::{PackageManager, PackageManifest};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_package_init() {
    let temp_dir = TempDir::new().unwrap();
    let package_name = "test_package";
    
    // Initialize package
    PackageManager::init(package_name, temp_dir.path()).unwrap();
    
    // Check that files were created
    assert!(temp_dir.path().join("package.pd").exists());
    assert!(temp_dir.path().join("src").exists());
    assert!(temp_dir.path().join("src/main.pd").exists());
    
    // Check manifest content
    let manifest = PackageManager::load_manifest(&temp_dir.path().join("package.pd")).unwrap();
    assert_eq!(manifest.name, package_name);
    assert_eq!(manifest.version, "0.1.0");
}

#[test]
fn test_manifest_parsing() {
    let manifest_content = r#"
name = "test_package"
version = "1.0.0"
description = "A test package"
authors = ["Test Author <test@example.com>"]
license = "MIT"

[dependencies]
http = "1.0"
json = "2.0"

[dev-dependencies]
test_framework = "0.1"
"#;
    
    let temp_dir = TempDir::new().unwrap();
    let manifest_path = temp_dir.path().join("package.pd");
    fs::write(&manifest_path, manifest_content).unwrap();
    
    let manifest = PackageManager::load_manifest(&manifest_path).unwrap();
    
    assert_eq!(manifest.name, "test_package");
    assert_eq!(manifest.version, "1.0.0");
    assert_eq!(manifest.description, Some("A test package".to_string()));
    assert_eq!(manifest.authors.len(), 1);
    assert_eq!(manifest.license, Some("MIT".to_string()));
    assert_eq!(manifest.dependencies.len(), 2);
    assert_eq!(manifest.dev_dependencies.len(), 1);
}

#[test]
fn test_add_dependency() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create initial manifest
    let manifest = PackageManifest {
        name: "test_package".to_string(),
        version: "0.1.0".to_string(),
        description: None,
        authors: vec![],
        license: None,
        dependencies: Default::default(),
        dev_dependencies: Default::default(),
        build_dependencies: Default::default(),
        main: None,
        lib: None,
        bin: vec![],
        examples: vec![],
        tests: vec![],
    };
    
    let manifest_path = temp_dir.path().join("package.pd");
    let content = PackageManager::manifest_to_string(&manifest);
    fs::write(&manifest_path, content).unwrap();
    
    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Add dependency
    let mut pm = PackageManager::new().unwrap();
    pm.add_dependency("http", "1.0", false).unwrap();
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
    
    // Check that dependency was added
    let updated_manifest = PackageManager::load_manifest(&manifest_path).unwrap();
    assert!(updated_manifest.dependencies.contains_key("http"));
}