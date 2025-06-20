// Dependency resolution for Palladium package manager
// "Solving the legendary dependency puzzle"

use crate::errors::{CompileError, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Semantic version for dependency resolution
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: String,
}

impl Version {
    /// Parse a version string
    pub fn parse(version: &str) -> Result<Self> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 3 {
            return Err(CompileError::Generic(format!(
                "Invalid version format: {}",
                version
            )));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| CompileError::Generic(format!("Invalid major version: {}", parts[0])))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| CompileError::Generic(format!("Invalid minor version: {}", parts[1])))?;

        // Handle patch version with pre-release
        let (patch, pre_release) = if let Some(dash_pos) = parts[2].find('-') {
            let patch_str = &parts[2][..dash_pos];
            let pre = &parts[2][dash_pos + 1..];
            let patch = patch_str.parse::<u32>().map_err(|_| {
                CompileError::Generic(format!("Invalid patch version: {}", patch_str))
            })?;
            (patch, pre.to_string())
        } else {
            let patch = parts[2].parse::<u32>().map_err(|_| {
                CompileError::Generic(format!("Invalid patch version: {}", parts[2]))
            })?;
            (patch, String::new())
        };

        Ok(Version {
            major,
            minor,
            patch,
            pre_release,
        })
    }

    /// Check if this version satisfies a version requirement
    pub fn satisfies(&self, requirement: &VersionRequirement) -> bool {
        match requirement {
            VersionRequirement::Exact(v) => self == v,
            VersionRequirement::GreaterThan(v) => self > v,
            VersionRequirement::GreaterThanOrEqual(v) => self >= v,
            VersionRequirement::LessThan(v) => self < v,
            VersionRequirement::LessThanOrEqual(v) => self <= v,
            VersionRequirement::Caret(v) => {
                // Caret: compatible with version (same major)
                self.major == v.major && self >= v
            }
            VersionRequirement::Tilde(v) => {
                // Tilde: compatible with version (same major.minor)
                self.major == v.major && self.minor == v.minor && self >= v
            }
            VersionRequirement::Wildcard => true,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pre_release.is_empty() {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else {
            write!(
                f,
                "{}.{}.{}-{}",
                self.major, self.minor, self.patch, self.pre_release
            )
        }
    }
}

/// Version requirement specification
#[derive(Debug, Clone, PartialEq)]
pub enum VersionRequirement {
    Exact(Version),
    GreaterThan(Version),
    GreaterThanOrEqual(Version),
    LessThan(Version),
    LessThanOrEqual(Version),
    Caret(Version),
    Tilde(Version),
    Wildcard,
}

impl VersionRequirement {
    /// Parse a version requirement string
    pub fn parse(requirement: &str) -> Result<Self> {
        let requirement = requirement.trim();

        if requirement == "*" {
            return Ok(VersionRequirement::Wildcard);
        }

        if let Some(version) = requirement.strip_prefix('^') {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::Caret(v));
        }

        if let Some(version) = requirement.strip_prefix('~') {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::Tilde(v));
        }

        if let Some(version) = requirement.strip_prefix(">=") {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::GreaterThanOrEqual(v));
        }

        if let Some(version) = requirement.strip_prefix('>') {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::GreaterThan(v));
        }

        if let Some(version) = requirement.strip_prefix("<=") {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::LessThanOrEqual(v));
        }

        if let Some(version) = requirement.strip_prefix('<') {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::LessThan(v));
        }

        if let Some(version) = requirement.strip_prefix('=') {
            let v = Version::parse(version)?;
            return Ok(VersionRequirement::Exact(v));
        }

        // Default to exact version
        let v = Version::parse(requirement)?;
        Ok(VersionRequirement::Exact(v))
    }
}

/// Package information for dependency resolution
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub dependencies: HashMap<String, VersionRequirement>,
}

/// Resolved dependency graph
#[derive(Debug)]
pub struct ResolvedDependencies {
    /// Map of package name to resolved version
    pub packages: HashMap<String, Version>,
    /// Dependency order (for installation)
    pub order: Vec<String>,
}

/// Dependency resolver using a simple SAT-like approach
pub struct DependencyResolver {
    /// Available packages in the registry
    available_packages: HashMap<String, Vec<Package>>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            available_packages: HashMap::new(),
        }
    }

    /// Add a package to the available packages
    pub fn add_available_package(&mut self, package: Package) {
        self.available_packages
            .entry(package.name.clone())
            .or_default()
            .push(package);
    }

    /// Resolve dependencies for a root package
    pub fn resolve(&self, root_package: &Package) -> Result<ResolvedDependencies> {
        let mut resolved = HashMap::new();
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();

        // Start with root package
        resolved.insert(root_package.name.clone(), root_package.version.clone());
        stack.push_back(root_package.clone());

        while let Some(current) = stack.pop_front() {
            if visited.contains(&current.name) {
                continue;
            }
            visited.insert(current.name.clone());

            // Process each dependency
            for (dep_name, dep_req) in &current.dependencies {
                // Find compatible versions
                let available = self
                    .available_packages
                    .get(dep_name)
                    .ok_or_else(|| {
                        CompileError::Generic(format!("Package '{}' not found in registry", dep_name))
                    })?;

                let compatible: Vec<&Package> = available
                    .iter()
                    .filter(|p| p.version.satisfies(dep_req))
                    .collect();

                if compatible.is_empty() {
                    return Err(CompileError::Generic(format!(
                        "No compatible version found for {} (required: {:?})",
                        dep_name, dep_req
                    )));
                }

                // Choose the latest compatible version
                let chosen = compatible
                    .into_iter()
                    .max_by_key(|p| &p.version)
                    .unwrap();

                // Check for version conflicts
                if let Some(existing_version) = resolved.get(dep_name) {
                    if existing_version != &chosen.version {
                        return Err(CompileError::Generic(format!(
                            "Version conflict for {}: {} vs {}",
                            dep_name, existing_version, chosen.version
                        )));
                    }
                } else {
                    resolved.insert(dep_name.clone(), chosen.version.clone());
                    stack.push_back(chosen.clone());
                }
            }
        }

        // Topological sort for installation order
        let order = self.topological_sort(&resolved)?;

        Ok(ResolvedDependencies {
            packages: resolved,
            order,
        })
    }

    /// Perform topological sort on the dependency graph
    fn topological_sort(&self, resolved: &HashMap<String, Version>) -> Result<Vec<String>> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Build the graph
        for package_name in resolved.keys() {
            in_degree.insert(package_name.clone(), 0);
            graph.insert(package_name.clone(), Vec::new());
        }

        // Fill the graph with dependencies
        for package_name in resolved.keys() {
            if let Some(packages) = self.available_packages.get(package_name) {
                if let Some(package) = packages.iter().find(|p| &p.version == &resolved[package_name])
                {
                    for dep_name in package.dependencies.keys() {
                        if resolved.contains_key(dep_name) {
                            graph.get_mut(dep_name).unwrap().push(package_name.clone());
                            *in_degree.get_mut(package_name).unwrap() += 1;
                        }
                    }
                }
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Find all nodes with in-degree 0
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node.clone());
            }
        }

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        if result.len() != resolved.len() {
            return Err(CompileError::Generic(
                "Circular dependency detected".to_string(),
            ));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre_release, "");

        let v = Version::parse("2.0.0-alpha").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert_eq!(v.pre_release, "alpha");
    }

    #[test]
    fn test_version_requirement_parsing() {
        let req = VersionRequirement::parse("^1.2.3").unwrap();
        match req {
            VersionRequirement::Caret(v) => {
                assert_eq!(v.major, 1);
                assert_eq!(v.minor, 2);
                assert_eq!(v.patch, 3);
            }
            _ => panic!("Expected caret requirement"),
        }

        let req = VersionRequirement::parse(">=1.0.0").unwrap();
        match req {
            VersionRequirement::GreaterThanOrEqual(v) => {
                assert_eq!(v.major, 1);
                assert_eq!(v.minor, 0);
                assert_eq!(v.patch, 0);
            }
            _ => panic!("Expected >= requirement"),
        }
    }

    #[test]
    fn test_version_satisfies() {
        let v1 = Version::parse("1.2.3").unwrap();
        let v2 = Version::parse("1.3.0").unwrap();
        let v3 = Version::parse("2.0.0").unwrap();

        let req = VersionRequirement::parse("^1.2.0").unwrap();
        assert!(v1.satisfies(&req));
        assert!(v2.satisfies(&req));
        assert!(!v3.satisfies(&req)); // Different major version

        let req = VersionRequirement::parse("~1.2.0").unwrap();
        assert!(v1.satisfies(&req));
        assert!(!v2.satisfies(&req)); // Different minor version
    }

    #[test]
    fn test_dependency_resolution() {
        let mut resolver = DependencyResolver::new();

        // Add available packages
        resolver.add_available_package(Package {
            name: "http".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            dependencies: HashMap::new(),
        });

        resolver.add_available_package(Package {
            name: "http".to_string(),
            version: Version::parse("1.1.0").unwrap(),
            dependencies: HashMap::new(),
        });

        resolver.add_available_package(Package {
            name: "json".to_string(),
            version: Version::parse("2.0.0").unwrap(),
            dependencies: HashMap::new(),
        });

        // Create root package with dependencies
        let mut deps = HashMap::new();
        deps.insert(
            "http".to_string(),
            VersionRequirement::parse("^1.0.0").unwrap(),
        );
        deps.insert(
            "json".to_string(),
            VersionRequirement::parse("2.0.0").unwrap(),
        );

        let root = Package {
            name: "myapp".to_string(),
            version: Version::parse("0.1.0").unwrap(),
            dependencies: deps,
        };

        let resolved = resolver.resolve(&root).unwrap();
        assert_eq!(resolved.packages.len(), 3);
        assert_eq!(resolved.packages["http"], Version::parse("1.1.0").unwrap());
        assert_eq!(resolved.packages["json"], Version::parse("2.0.0").unwrap());
    }
}