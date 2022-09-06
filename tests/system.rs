use std::fs;
use std::process::Command;

// The datatest crate could be used to make the output of running this test far superior,
// but that crate relies on functionality that is currently not stable. :-(

#[test]
fn run_all_tests() {
    let dirs = fs::read_dir("tests").expect("Failed to read directory");
    for maybe_entry in dirs {
        let entry = maybe_entry.expect("Failed to read entry");
        let name = entry
            .path()
            .to_str()
            .expect("Failed to convert entry to string")
            .to_string();
        if name.ends_with(".crush") {
            let output = Command::new("./target/debug/crush")
                .args(&[name.as_str()])
                .output()
                .expect("failed to execute process");
            let output_name = name.clone() + ".output";
            let expected_output = fs::read_to_string(output_name.as_str())
                .expect(format!("failed to read output file {}", output_name).as_str());
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                expected_output,
                "\n\nError while running file {}",
                name.as_str()
            );
        }
    }
}
