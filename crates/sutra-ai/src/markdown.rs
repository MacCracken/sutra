//! Markdown playbook parser — extracts structured intent from Markdown,
//! sends to hoosh for TOML generation.

use comrak::{parse_document, Arena, Options};

/// Extract structured sections from a Markdown playbook.
/// Returns (name, targets, tasks) as raw strings for hoosh translation.
pub fn extract_sections(markdown: &str) -> MarkdownPlaybook {
    let arena = Arena::new();
    let options = Options::default();
    let root = parse_document(&arena, markdown, &options);

    let mut result = MarkdownPlaybook {
        name: String::new(),
        sections: Vec::new(),
    };

    let mut current_section = String::new();
    let mut current_items: Vec<String> = Vec::new();

    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            comrak::nodes::NodeValue::Heading(heading) => {
                // Save previous section
                if !current_section.is_empty() {
                    result.sections.push(MarkdownSection {
                        heading: current_section.clone(),
                        items: current_items.clone(),
                    });
                    current_items.clear();
                }

                if heading.level == 1 {
                    // Extract heading text from children
                    let text = collect_text(node);
                    result.name = text;
                    current_section.clear();
                } else {
                    let text = collect_text(node);
                    current_section = text;
                }
            }
            comrak::nodes::NodeValue::Item(_) => {
                let text = collect_text(node);
                if !text.is_empty() {
                    current_items.push(text);
                }
            }
            _ => {}
        }
    }

    // Save last section
    if !current_section.is_empty() {
        result.sections.push(MarkdownSection {
            heading: current_section,
            items: current_items,
        });
    }

    result
}

fn collect_text<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    let mut text = String::new();
    for child in node.descendants() {
        let data = child.data.borrow();
        if let comrak::nodes::NodeValue::Text(ref t) = data.value {
            text.push_str(t);
        }
        if let comrak::nodes::NodeValue::Code(ref c) = data.value {
            text.push_str(&c.literal);
        }
    }
    text.trim().to_string()
}

#[derive(Debug, Clone)]
pub struct MarkdownPlaybook {
    pub name: String,
    pub sections: Vec<MarkdownSection>,
}

#[derive(Debug, Clone)]
pub struct MarkdownSection {
    pub heading: String,
    pub items: Vec<String>,
}

impl MarkdownPlaybook {
    /// Convert to a prompt string for hoosh NL→TOML translation.
    pub fn to_prompt(&self) -> String {
        let mut prompt = format!("Generate a Sutra TOML playbook for: {}\n\n", self.name);

        for section in &self.sections {
            prompt.push_str(&format!("{}:\n", section.heading));
            for item in &section.items {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sections() {
        let md = r#"# Deploy tarang to edge fleet

## Target
- role: edge
- arch: aarch64

## Tasks
- Install `tarang` version `2026.3.18` via ark
- Enable `tarang.service` via argonaut
- Verify port 8070 is listening
"#;

        let result = extract_sections(md);
        assert_eq!(result.name, "Deploy tarang to edge fleet");
        assert_eq!(result.sections.len(), 2);
        assert_eq!(result.sections[0].heading, "Target");
        assert_eq!(result.sections[0].items.len(), 2);
        assert_eq!(result.sections[1].heading, "Tasks");
        assert_eq!(result.sections[1].items.len(), 3);
    }

    #[test]
    fn test_to_prompt() {
        let pb = MarkdownPlaybook {
            name: "Deploy tarang".to_string(),
            sections: vec![MarkdownSection {
                heading: "Tasks".to_string(),
                items: vec!["Install tarang".to_string()],
            }],
        };

        let prompt = pb.to_prompt();
        assert!(prompt.contains("Deploy tarang"));
        assert!(prompt.contains("Install tarang"));
    }

    #[test]
    fn test_empty_markdown() {
        let result = extract_sections("");
        assert!(result.name.is_empty());
        assert!(result.sections.is_empty());
    }
}
