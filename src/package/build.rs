// Build system for Palladium
// "Forging packages into legendary artifacts"

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use crate::errors::{CompileError, Result};
use crate::driver::Driver;
use super::{PackageManifest, PackageManager};

/// Build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Release mode (optimized)
    pub release: bool,
    /// Target directory
    pub target_dir: PathBuf,
    /// Enable LLVM backend
    pub use_llvm: bool,
    /// Verbose output
    pub verbose: bool,
    /// Number of parallel jobs
    pub jobs: usize,
    /// Features to enable
    pub features: HashSet<String>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            release: false,
            target_dir: PathBuf::from("target"),
            use_llvm: false,
            verbose: false,
            jobs: num_cpus::get(),
            features: HashSet::new(),
        }
    }
}

/// Build context for tracking dependencies and artifacts
pub struct BuildContext {
    /// Build configuration
    config: BuildConfig,
    /// Package manifests by name
    packages: HashMap<String, PackageManifest>,
    /// Build graph (package -> dependencies)
    dependency_graph: HashMap<String, Vec<String>>,
    /// Artifact cache (file -> last modified time)
    artifact_cache: HashMap<PathBuf, SystemTime>,
}

impl BuildContext {
    pub fn new(config: BuildConfig) -> Self {
        Self {
            config,
            packages: HashMap::new(),
            dependency_graph: HashMap::new(),
            artifact_cache: HashMap::new(),
        }
    }
    
    /// Load a package and its dependencies
    pub fn load_package(&mut self, manifest_path: &Path) -> Result<String> {
        let manifest = PackageManager::load_manifest(manifest_path)?;
        let name = manifest.name.clone();
        
        // Add to packages
        self.packages.insert(name.clone(), manifest.clone());
        
        // Build dependency list
        let mut deps = Vec::new();
        for (dep_name, _dep_spec) in &manifest.dependencies {
            deps.push(dep_name.clone());
            // TODO: Resolve and load dependency packages
        }
        
        self.dependency_graph.insert(name.clone(), deps);
        
        Ok(name)
    }
    
    /// Get build order using topological sort
    pub fn get_build_order(&self) -> Result<Vec<String>> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        
        for package in self.packages.keys() {
            if !visited.contains(package) {
                self.visit_package(package, &mut visited, &mut visiting, &mut order)?;
            }
        }
        
        Ok(order)
    }
    
    /// DFS visit for topological sort
    fn visit_package(
        &self,
        package: &str,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if visiting.contains(package) {
            return Err(CompileError::Generic(format!("Circular dependency detected: {}", package)));
        }
        
        if visited.contains(package) {
            return Ok(());
        }
        
        visiting.insert(package.to_string());
        
        if let Some(deps) = self.dependency_graph.get(package) {
            for dep in deps {
                self.visit_package(dep, visited, visiting, order)?;
            }
        }
        
        visiting.remove(package);
        visited.insert(package.to_string());
        order.push(package.to_string());
        
        Ok(())
    }
    
    /// Check if a file needs rebuilding
    pub fn needs_rebuild(&self, source: &Path, target: &Path) -> bool {
        if !target.exists() {
            return true;
        }
        
        let source_modified = fs::metadata(source)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
            
        let target_modified = fs::metadata(target)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
            
        source_modified > target_modified
    }
    
    /// Build a single package
    pub fn build_package(&mut self, package_name: &str) -> Result<PathBuf> {
        let manifest = self.packages.get(package_name)
            .ok_or_else(|| CompileError::Generic(format!("Package '{}' not found", package_name)))?
            .clone();
        
        if self.config.verbose {
            println!("📦 Building package '{}'", package_name);
        }
        
        // Create output directory
        let output_dir = self.config.target_dir
            .join(if self.config.release { "release" } else { "debug" })
            .join("deps");
        
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir)?;
        }
        
        // Determine what to build
        let mut built_artifacts = Vec::new();
        
        // Build library if present
        if let Some(lib_path) = &manifest.lib {
            let artifact = self.build_library(&manifest, lib_path, &output_dir)?;
            built_artifacts.push(artifact);
        }
        
        // Build binaries
        for binary in &manifest.bin {
            let artifact = self.build_binary(&manifest, &binary.path, &binary.name, &output_dir)?;
            built_artifacts.push(artifact);
        }
        
        // Build main if present and no explicit binaries
        if manifest.bin.is_empty() {
            if let Some(main_path) = &manifest.main {
                let artifact = self.build_binary(&manifest, main_path, &manifest.name, &output_dir)?;
                built_artifacts.push(artifact);
            } else if Path::new("src/main.pd").exists() {
                let artifact = self.build_binary(&manifest, "src/main.pd", &manifest.name, &output_dir)?;
                built_artifacts.push(artifact);
            }
        }
        
        if built_artifacts.is_empty() {
            return Err(CompileError::Generic(format!("No build targets found for package '{}'", package_name)));
        }
        
        Ok(built_artifacts[0].clone())
    }
    
    /// Build a library
    fn build_library(&mut self, manifest: &PackageManifest, lib_path: &str, output_dir: &Path) -> Result<PathBuf> {
        let source_path = Path::new(lib_path);
        let output_name = format!("lib{}", manifest.name);
        
        self.compile_file(source_path, &output_name, output_dir, true)
    }
    
    /// Build a binary
    fn build_binary(&mut self, manifest: &PackageManifest, bin_path: &str, name: &str, output_dir: &Path) -> Result<PathBuf> {
        let source_path = Path::new(bin_path);
        
        self.compile_file(source_path, name, output_dir, false)
    }
    
    /// Compile a single file
    fn compile_file(&mut self, source_path: &Path, output_name: &str, output_dir: &Path, is_lib: bool) -> Result<PathBuf> {
        let output_path = if self.config.use_llvm {
            output_dir.join(format!("{}.ll", output_name))
        } else {
            output_dir.join(format!("{}.c", output_name))
        };
        
        // Check if rebuild is needed
        if !self.needs_rebuild(source_path, &output_path) {
            if self.config.verbose {
                println!("   ⏭️  {} is up to date", source_path.display());
            }
            return Ok(output_path);
        }
        
        if self.config.verbose {
            println!("   🔨 Compiling {}", source_path.display());
        }
        
        // Create driver with appropriate settings
        let mut driver = Driver::new();
        if self.config.use_llvm {
            driver = driver.with_llvm();
        }
        
        // Compile the file
        let temp_output = driver.compile_file(source_path)?;
        
        // Move to final location
        fs::rename(&temp_output, &output_path)?;
        
        // Update cache
        if let Ok(metadata) = fs::metadata(&output_path) {
            if let Ok(modified) = metadata.modified() {
                self.artifact_cache.insert(output_path.clone(), modified);
            }
        }
        
        Ok(output_path)
    }
}

/// Build system entry point
pub struct BuildSystem {
    context: BuildContext,
}

impl BuildSystem {
    pub fn new(config: BuildConfig) -> Self {
        Self {
            context: BuildContext::new(config),
        }
    }
    
    /// Build the current project
    pub fn build(&mut self) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Load root package
        let root_package = self.context.load_package(Path::new("package.pd"))?;
        
        // Get build order
        let build_order = self.context.get_build_order()?;
        
        println!("🏗️  Building {} package(s)", build_order.len());
        
        // Build packages in order
        for package in build_order {
            self.context.build_package(&package)?;
        }
        
        let elapsed = start_time.elapsed();
        println!("✅ Build completed in {:.2}s", elapsed.as_secs_f64());
        
        Ok(())
    }
    
    /// Clean build artifacts
    pub fn clean(&self) -> Result<()> {
        let target_dir = &self.context.config.target_dir;
        
        if target_dir.exists() {
            println!("🧹 Cleaning {}", target_dir.display());
            fs::remove_dir_all(target_dir)?;
        }
        
        println!("✅ Clean complete");
        Ok(())
    }
    
    /// Run the built executable
    pub fn run(&mut self, args: Vec<String>) -> Result<()> {
        // First build
        self.build()?;
        
        // Find the main executable
        let manifest = PackageManager::load_manifest(Path::new("package.pd"))?;
        let exe_name = &manifest.name;
        
        let exe_dir = self.context.config.target_dir
            .join(if self.context.config.release { "release" } else { "debug" })
            .join("deps");
        
        let c_file = exe_dir.join(format!("{}.c", exe_name));
        let exe_file = exe_dir.join(exe_name);
        
        // Compile C to executable if needed
        if self.needs_executable_rebuild(&c_file, &exe_file) {
            println!("🔗 Linking {}", exe_name);
            
            let mut gcc_cmd = std::process::Command::new("gcc");
            gcc_cmd.arg(&c_file)
                .arg("-o")
                .arg(&exe_file);
            
            if self.context.config.release {
                gcc_cmd.arg("-O3");
            }
            
            let output = gcc_cmd.output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CompileError::Generic(format!("Linking failed:\n{}", stderr)));
            }
        }
        
        // Run the executable
        println!("🚀 Running {}", exe_name);
        println!("─────────────────────────────────────");
        
        let mut cmd = std::process::Command::new(&exe_file);
        cmd.args(&args);
        
        let status = cmd.status()?;
        
        println!("─────────────────────────────────────");
        
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        
        Ok(())
    }
    
    /// Check if executable needs rebuilding
    fn needs_executable_rebuild(&self, source: &Path, target: &Path) -> bool {
        self.context.needs_rebuild(source, target)
    }
    
    /// Run tests
    pub fn test(&mut self, filter: Option<&str>) -> Result<()> {
        println!("🧪 Running tests...");
        
        let manifest = PackageManager::load_manifest(Path::new("package.pd"))?;
        
        // Find test files
        let mut test_files = Vec::new();
        
        // Explicit test targets
        for test in &manifest.tests {
            test_files.push((test.name.clone(), PathBuf::from(&test.path)));
        }
        
        // Auto-discover tests in tests/ directory
        let tests_dir = Path::new("tests");
        if tests_dir.exists() {
            for entry in fs::read_dir(tests_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "pd") {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    if let Some(filter) = filter {
                        if !name.contains(filter) {
                            continue;
                        }
                    }
                    
                    test_files.push((name, path));
                }
            }
        }
        
        if test_files.is_empty() {
            println!("No tests found");
            return Ok(());
        }
        
        println!("Found {} test(s)", test_files.len());
        
        let mut passed = 0;
        let mut failed = 0;
        
        for (name, path) in test_files {
            print!("test {} ... ", name);
            
            match self.run_test(&path) {
                Ok(()) => {
                    println!("✅ ok");
                    passed += 1;
                }
                Err(e) => {
                    println!("❌ FAILED");
                    println!("  Error: {}", e);
                    failed += 1;
                }
            }
        }
        
        println!("\nTest results: {} passed, {} failed", passed, failed);
        
        if failed > 0 {
            Err(CompileError::Generic(format!("{} test(s) failed", failed)))
        } else {
            Ok(())
        }
    }
    
    /// Run a single test file
    fn run_test(&mut self, test_path: &Path) -> Result<()> {
        // Compile the test
        let output_dir = self.context.config.target_dir.join("debug").join("tests");
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir)?;
        }
        
        let test_name = test_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("test");
        
        let output_path = self.context.compile_file(test_path, test_name, &output_dir, false)?;
        
        // Link to executable
        let exe_path = output_dir.join(test_name);
        
        let gcc_output = std::process::Command::new("gcc")
            .arg(&output_path)
            .arg("-o")
            .arg(&exe_path)
            .output()?;
        
        if !gcc_output.status.success() {
            let stderr = String::from_utf8_lossy(&gcc_output.stderr);
            return Err(CompileError::Generic(format!("Test compilation failed:\n{}", stderr)));
        }
        
        // Run the test
        let output = std::process::Command::new(&exe_path).output()?;
        
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(CompileError::Generic(format!(
                "Test failed with exit code {}\nstdout:\n{}\nstderr:\n{}", 
                output.status.code().unwrap_or(-1),
                stdout,
                stderr
            )))
        }
    }
}

// Re-export num_cpus functionality
pub mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }
}