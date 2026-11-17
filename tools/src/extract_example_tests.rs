use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};

pub fn extract_examples() -> HashMap<String, String> {
    let directory_path = "../tests/examples";
    let paths = fs::read_dir(directory_path).expect("Unable to read directory");

    let mut example_map: HashMap<String, String> = HashMap::new();

    for path in paths {
        let path = path.expect("Error reading path").path();
        if path.is_file() {
            let file = fs::File::open(&path).expect("Unable to open file");
            let reader = BufReader::new(file);

            let mut recording = false;
            let mut example_code = Vec::new();
            let mut example_id = String::new();

            for line in reader.lines() {
                let line = line.expect("Error reading line");
                if line.contains("// @example-start:") {
                    recording = true;
                    // Extract the ID or name from the line
                    example_id = line.split(':').nth(1).unwrap_or("").trim().to_string();
                    example_code.clear();
                } else if line.contains("// @example-end") {
                    if recording {
                        recording = false;
                        // Insert the collected code into the map with the example ID as the key
                        example_map.insert(example_id.clone(), example_code.join("\n"));
                    }
                } else if recording {
                    example_code.push(line);
                }
            }
        }
    }

    example_map
}
