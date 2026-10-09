use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_FLOAT_FUNCTION_NAMES: [&str; 20] = [
    "ln", "ln_1p", "log", "log2", "log10", "exp", "exp2", "exp_m1", "powf", "powi", "sin", "cos", "tan", "sin_cos",
    "atan", "atan2", "atanh", "hypot", "cbrt", "mul_add",
];
const FORBIDDEN_COLLECTION_NAMES: [&str; 2] = ["HashMap", "HashSet"];
const DETERMINISM_EXEMPT_DIRECTORY_NAMES: [&str; 2] = ["render", "play"];
const BITCODE_CRATE_NAME: &str = "bitcode";
const BITCODE_PERMITTED_DIRECTORY_NAME: &str = "protocol";

#[derive(Debug)]
struct SourceFile {
    relative_path: PathBuf,
    contents: String,
}

impl SourceFile {
    fn is_inside_directory(&self, directory_name: &str) -> bool {
        self.relative_path.components().any(|component| component.as_os_str() == directory_name)
    }
}

#[test]
fn shared_source_uses_no_forbidden_operations() {
    let source_root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source_files: Vec<SourceFile> = read_source_files(&source_root, &source_root);
    let violations: Vec<String> = source_files.iter().flat_map(find_violations).collect();

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn find_violations_reports_forbidden_operations_outside_permitted_directories() {
    let source_file: SourceFile = create_source_file(
        "simulation/growth.rs",
        "let a: f64 = x.ln();\nlet b: fn(f64) -> f64 = f64::sin;\nlet c: HashMap<u32, u32> = HashMap::new();\nuse bitcode::Encode;\nlet d: f64 = libm::log(x);\n",
    );

    let violations: Vec<String> = find_violations(&source_file);

    assert_eq!(
        violations,
        vec![
            String::from("simulation/growth.rs:1: .ln("),
            String::from("simulation/growth.rs:2: f64::sin"),
            String::from("simulation/growth.rs:3: HashMap"),
            String::from("simulation/growth.rs:4: bitcode"),
        ],
    );
}

#[test]
fn find_violations_permits_exempt_directories() {
    let source_files: Vec<SourceFile> = vec![
        create_source_file("render/sprite.rs", "let a: f64 = x.ln();\n"),
        create_source_file("play/input.rs", "let b: fn(f64) -> f64 = f64::sin;\n"),
        create_source_file("protocol/message.rs", "use bitcode::Encode;\n"),
    ];

    let violations: Vec<String> = source_files.iter().flat_map(find_violations).collect();

    assert_eq!(violations, Vec::<String>::new());
}

fn create_source_file(relative_path: &str, contents: &str) -> SourceFile {
    SourceFile {
        relative_path: PathBuf::from(relative_path),
        contents: String::from(contents),
    }
}

fn read_source_files(source_root: &Path, directory: &Path) -> Vec<SourceFile> {
    let mut source_files: Vec<SourceFile> = Vec::new();
    let mut entry_paths: Vec<PathBuf> = fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().path()).collect();
    entry_paths.sort();

    for entry_path in entry_paths {
        let is_directory: bool = entry_path.is_dir();
        if is_directory {
            source_files.extend(read_source_files(source_root, &entry_path));
            continue;
        }

        let is_rust_file: bool = entry_path.extension().is_some_and(|extension| extension == "rs");
        if !is_rust_file {
            continue;
        }

        source_files.push(SourceFile {
            relative_path: entry_path.strip_prefix(source_root).unwrap().to_path_buf(),
            contents: fs::read_to_string(&entry_path).unwrap(),
        });
    }

    source_files
}

fn find_violations(source_file: &SourceFile) -> Vec<String> {
    let mut violations: Vec<String> = Vec::new();
    let is_determinism_exempt: bool = DETERMINISM_EXEMPT_DIRECTORY_NAMES
        .iter()
        .any(|directory_name| source_file.is_inside_directory(directory_name));

    if !is_determinism_exempt {
        violations.extend(find_pattern_violations(source_file, &get_determinism_patterns()));
    }

    if !source_file.is_inside_directory(BITCODE_PERMITTED_DIRECTORY_NAME) {
        violations.extend(find_pattern_violations(
            source_file,
            &[String::from(BITCODE_CRATE_NAME)],
        ));
    }

    violations
}

fn get_determinism_patterns() -> Vec<String> {
    let mut patterns: Vec<String> = Vec::new();

    for function_name in FORBIDDEN_FLOAT_FUNCTION_NAMES {
        patterns.push(format!(".{function_name}("));
        patterns.push(format!("f64::{function_name}"));
        patterns.push(format!("f32::{function_name}"));
    }

    for collection_name in FORBIDDEN_COLLECTION_NAMES {
        patterns.push(String::from(collection_name));
    }

    patterns
}

fn find_pattern_violations(source_file: &SourceFile, patterns: &[String]) -> Vec<String> {
    let mut violations: Vec<String> = Vec::new();

    for (line_index, line) in source_file.contents.lines().enumerate() {
        for pattern in patterns {
            if line.contains(pattern.as_str()) {
                violations.push(format!(
                    "{}:{}: {}",
                    source_file.relative_path.display(),
                    line_index + 1,
                    pattern,
                ));
            }
        }
    }

    violations
}
