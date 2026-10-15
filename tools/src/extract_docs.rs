mod extract_example_tests;

use serde_json::{json, Map, Value};
use std::fs;
use syn::spanned::Spanned;
use syn::{ImplItem, ImplItemMethod, Item, Type};

#[derive(Default)]
struct Documentation {
    docs: Map<String, Value>,
    code_examples: Map<String, Value>,
    groups: Vec<Value>,
}

fn main() {
    let file_content = fs::read_to_string("../src/api/spec.rs").expect("Unable to read file");
    let syntax_tree = syn::parse_file(&file_content).expect("Unable to parse file");
    let lines: Vec<_> = file_content.lines().collect();
    let mut when = Documentation::default();
    let mut then = Documentation::default();

    for item in &syntax_tree.items {
        if let Item::Impl(item) = item {
            if let Type::Path(type_path) = &*item.self_ty {
                let ident = &type_path.path.segments.last().unwrap().ident;
                let documentation = if ident == "When" {
                    &mut when
                } else if ident == "Then" {
                    &mut then
                } else {
                    continue;
                };
                for item in &item.items {
                    if let ImplItem::Method(method) = item {
                        documentation.extract_method(method, &lines);
                    }
                }
            }
        }
    }

    fs::create_dir_all("target/generated").expect("Unable to create output directory");
    write_json("docs.json", json!({"when": when.docs, "then": then.docs}));
    write_json(
        "code_examples.json",
        json!({"when": when.code_examples, "then": then.code_examples}),
    );
    write_json("groups.json", json!({"when": when.groups, "then": then.groups}));
    write_json("example_tests.json", json!(extract_example_tests::extract_examples()));
}

impl Documentation {
    fn extract_method(&mut self, method: &ImplItemMethod, lines: &[&str]) {
        let method_name = method.sig.ident.to_string();
        let mut docs = String::new();
        let mut example = String::new();
        let mut in_code_block = false;

        for line in method.attrs.iter().filter_map(doc_line) {
            docs.push_str(line.strip_prefix(' ').unwrap_or(&line));
            docs.push('\n');

            if line.trim().starts_with("```rust") {
                example.push_str("```rust\n");
                in_code_block = true;
            } else if line.trim().starts_with("```") && in_code_block {
                example.push_str("```\n");
                in_code_block = false;
            } else if in_code_block {
                example.push_str(&line);
                example.push('\n');
            }
        }

        self.docs.insert(method_name.clone(), json!(docs));
        if !example.is_empty() {
            self.code_examples.insert(method_name.clone(), json!(example));
        }
        // Spans use one-based line numbers, so this indexes the following line.
        let group = lines
            .get(method.span().end().line)
            .and_then(|line| line.trim().strip_prefix("// @docs-group:"))
            .map(str::trim)
            .unwrap_or("No group");
        self.groups.push(json!({"method": method_name, "group": group}));
    }
}

fn doc_line(attr: &syn::Attribute) -> Option<String> {
    if attr.path.is_ident("doc") {
        if let Ok(syn::Meta::NameValue(value)) = attr.parse_meta() {
            if let syn::Lit::Str(value) = value.lit {
                return Some(value.value());
            }
        }
    }
    None
}

fn write_json(filename: &str, value: Value) {
    let output = serde_json::to_string_pretty(&value).expect("Unable to serialize JSON");
    fs::write(format!("target/generated/{}", filename), output).expect("Unable to write file");
}
