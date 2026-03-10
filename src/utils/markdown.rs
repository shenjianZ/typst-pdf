use std::collections::BTreeMap;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::models::RenderOptions;

pub fn markdown_to_typst(
    markdown: &str,
    variables: &BTreeMap<String, serde_json::Value>,
    options: &RenderOptions,
) -> String {
    let mut enabled = Options::empty();
    enabled.insert(Options::ENABLE_TABLES);
    enabled.insert(Options::ENABLE_FOOTNOTES);
    enabled.insert(Options::ENABLE_STRIKETHROUGH);
    enabled.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, enabled);
    let mut out = String::new();
    let mut list_stack: Vec<char> = Vec::new();
    let mut image_href: Option<String> = None;

    if let Some(theme) = options.theme.as_deref() {
        out.push_str(&format!("#let doc_theme = \"{}\"\n", escape_text(theme)));
    }

    for (key, value) in variables {
        out.push_str(&format!(
            "#let {} = {}\n",
            sanitize_ident(key),
            json_to_typst(value)
        ));
    }

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => out.push_str(heading_prefix(level)),
                Tag::BlockQuote(_) => out.push_str("#quote(block: true)["),
                Tag::CodeBlock(kind) => match kind {
                    CodeBlockKind::Indented => out.push_str("#raw(block: true, lang: \"text\")["),
                    CodeBlockKind::Fenced(lang) => {
                        out.push_str(&format!(
                            "#raw(block: true, lang: \"{}\")[",
                            escape_text(&lang)
                        ));
                    }
                },
                Tag::List(start) => {
                    list_stack.push(if start.is_some() { '+' } else { '-' });
                    out.push('\n');
                }
                Tag::Item => {
                    let marker = *list_stack.last().unwrap_or(&'-');
                    out.push_str(&format!("{marker} "));
                }
                Tag::Emphasis => out.push_str("#emph["),
                Tag::Strong => out.push_str("#strong["),
                Tag::Strikethrough => out.push_str("#strike["),
                Tag::Link { dest_url, .. } => {
                    out.push_str("#link(\"");
                    out.push_str(&escape_text(&dest_url));
                    out.push_str("\")[");
                }
                Tag::Image { dest_url, .. } => image_href = Some(dest_url.to_string()),
                Tag::FootnoteDefinition(name) => {
                    out.push_str(&format!("\n#footnote[{}: ", escape_text(&name)));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => out.push_str("\n\n"),
                TagEnd::Heading(_) => out.push_str("\n\n"),
                TagEnd::BlockQuote(_) => out.push_str("]\n\n"),
                TagEnd::CodeBlock => out.push_str("]\n\n"),
                TagEnd::List(_) => {
                    list_stack.pop();
                    out.push('\n');
                }
                TagEnd::Item => out.push('\n'),
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => out.push(']'),
                TagEnd::Link => out.push(']'),
                TagEnd::Image => {
                    if let Some(path) = image_href.take() {
                        out.push_str(&format!("#image(\"{}\")", escape_text(&path)));
                    }
                }
                TagEnd::FootnoteDefinition => out.push_str("]\n"),
                _ => {}
            },
            Event::Text(text) => out.push_str(&escape_text(&text)),
            Event::Code(code) => out.push_str(&format!("#raw(\"{}\")", escape_text(&code))),
            Event::SoftBreak => out.push('\n'),
            Event::HardBreak => out.push_str("\\\n"),
            Event::Rule => out.push_str("\n#line(length: 100%)\n\n"),
            Event::FootnoteReference(name) => {
                out.push_str(&format!("#footnote[{}]", escape_text(&name)))
            }
            Event::TaskListMarker(checked) => {
                out.push_str(if checked { "[x]" } else { "[ ]" });
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                out.push_str("#raw(\"");
                out.push_str(&escape_text(&html));
                out.push_str("\")");
            }
            _ => {}
        }
    }

    out
}

fn heading_prefix(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "= ",
        HeadingLevel::H2 => "== ",
        HeadingLevel::H3 => "=== ",
        HeadingLevel::H4 => "==== ",
        HeadingLevel::H5 => "===== ",
        HeadingLevel::H6 => "====== ",
    }
}

fn escape_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn sanitize_ident(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn json_to_typst(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "none".to_owned(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => format!("\"{}\"", escape_text(value)),
        serde_json::Value::Array(values) => {
            let items = values
                .iter()
                .map(json_to_typst)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({items})")
        }
        serde_json::Value::Object(entries) => {
            let items = entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", json_to_typst(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({items})")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::markdown_to_typst;
    use crate::models::RenderOptions;

    #[test]
    fn converts_basic_markdown() {
        let markdown = "# Title\n\n- item\n\n`code`\n";
        let output = markdown_to_typst(markdown, &BTreeMap::new(), &RenderOptions::default());
        assert!(output.contains("= Title"));
        assert!(output.contains("- item"));
        assert!(output.contains("#raw(\"code\")"));
    }
}
