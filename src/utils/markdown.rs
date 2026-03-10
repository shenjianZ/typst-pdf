use std::collections::BTreeMap;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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
    enabled.insert(Options::ENABLE_MATH);
    enabled.insert(Options::ENABLE_SUPERSCRIPT);
    enabled.insert(Options::ENABLE_SUBSCRIPT);
    enabled.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(markdown, enabled);
    let mut context = MarkdownContext::default();

    if let Some(theme) = options.theme.as_deref() {
        context
            .out
            .push_str(&format!("#let doc_theme = \"{}\"\n", escape_text(theme)));
    }

    context.out.push_str(
        "#let md_quote(depth, body) = block(\n  width: 100%,\n  inset: (x: 14pt, y: 11pt),\n  fill: rgb(\"#f8fafc\"),\n  stroke: (left: 2pt + rgb(\"#d5dce5\")),\n  radius: 4pt,\n  above: 0.85em,\n  below: 0.95em,\n)[\n  #set text(fill: rgb(\"#4b5563\"), style: \"italic\")\n  #body\n]\n",
    );
    context.out.push_str(
        "#let md_figure(path, caption: none) = block(\n  width: 100%,\n  above: 1em,\n  below: 1.05em,\n)[\n  #image(path, width: 100%)\n]\n",
    );
    context.out.push_str(
        "#let md_codeblock(lang, body) = block(\n  width: 100%,\n  inset: (x: 13pt, y: 11pt),\n  radius: 5pt,\n  fill: rgb(\"#f5f7fb\"),\n  stroke: 0.7pt + rgb(\"#dde3ea\"),\n  above: 0.85em,\n  below: 1.05em,\n)[\n  #if lang != \"\" and lang != \"text\" [\n    #set text(size: 7.8pt, weight: \"medium\", fill: rgb(\"#6b7280\"))\n    #upper(lang)\n    #v(0.55em)\n  ]\n  #set par(leading: 0.72em)\n  #set text(size: 9.6pt, fill: rgb(\"#334155\"))\n  #raw(body, block: true, lang: lang)\n]\n",
    );
    context.out.push_str(
        "#let md_task(checked) = box(\n  inset: 0pt,\n  baseline: 35%,\n)[\n  #box(\n    width: 0.92em,\n    height: 0.92em,\n    radius: 0.24em,\n    fill: if checked { rgb(\"#8b5cf6\") } else { white },\n    stroke: 0.8pt + if checked { rgb(\"#8b5cf6\") } else { rgb(\"#cbd5e1\") },\n    inset: 0pt,\n  )[\n    #if checked {\n      align(center + horizon)[#text(size: 0.62em, weight: \"bold\", fill: white)[✓]]\n    }\n  ]\n]\n",
    );

    for (key, value) in variables {
        context.out.push_str(&format!(
            "#let {} = {}\n",
            sanitize_ident(key),
            json_to_typst(value)
        ));
    }

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => context.push_str(heading_prefix(level)),
                Tag::BlockQuote(_) => {
                    context.quote_depth += 1;
                    context.push_str(&format!("#md_quote({})[", context.quote_depth));
                }
                Tag::CodeBlock(kind) => match kind {
                    CodeBlockKind::Indented => context.start_code_block("text"),
                    CodeBlockKind::Fenced(lang) => context.start_code_block(&lang),
                },
                Tag::List(start) => {
                    context
                        .list_stack
                        .push(if start.is_some() { '+' } else { '-' });
                    context.ensure_blank_line();
                }
                Tag::Item => {
                    let marker = *context.list_stack.last().unwrap_or(&'-');
                    let depth = context.list_stack.len().saturating_sub(1);
                    context.push_str(&format!("{}{} ", "  ".repeat(depth), marker));
                }
                Tag::Emphasis => context.push_str("#emph["),
                Tag::Strong => context.push_str("#strong["),
                Tag::Strikethrough => context.push_str("#strike["),
                Tag::Superscript => context.push_str("#super["),
                Tag::Subscript => context.push_str("#sub["),
                Tag::Link { dest_url, .. } => {
                    context.push_str("#link(\"");
                    context.push_str(&escape_text(&dest_url));
                    context.push_str("\")[");
                }
                Tag::Image { dest_url, .. } => {
                    context.image_href = Some(dest_url.to_string());
                    context.in_image = true;
                    context.image_alt = Some(String::new());
                }
                Tag::FootnoteDefinition(name) => context.push_str(&format!("\n[^{name}]: ")),
                Tag::Table(alignments) => context.start_table(alignments),
                Tag::TableHead => context.mark_table_head(true),
                Tag::TableRow => context.start_table_row(),
                Tag::TableCell => context.start_table_cell(),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => context.push_str("\n\n"),
                TagEnd::Heading(_) => context.push_str("\n\n"),
                TagEnd::BlockQuote(_) => {
                    context.push_str("]\n\n");
                    context.quote_depth = context.quote_depth.saturating_sub(1);
                }
                TagEnd::CodeBlock => context.finish_code_block(),
                TagEnd::List(_) => {
                    context.list_stack.pop();
                    context.push_str("\n");
                }
                TagEnd::Item => context.push_str("\n"),
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Superscript
                | TagEnd::Subscript => context.push_char(']'),
                TagEnd::Link => context.push_char(']'),
                TagEnd::Image => {
                    if let Some(path) = context.image_href.take() {
                        let caption = context
                            .image_alt
                            .take()
                            .unwrap_or_default()
                            .trim()
                            .to_owned();
                        if caption.is_empty() {
                            context.push_str(&format!("#md_figure(\"{}\")", escape_text(&path)));
                        } else {
                            context.push_str(&format!(
                                "#md_figure(\"{}\", caption: [{}])",
                                escape_text(&path),
                                escape_text(&caption)
                            ));
                        }
                    }
                    context.in_image = false;
                }
                TagEnd::FootnoteDefinition => context.push_str("\n"),
                TagEnd::Table => context.finish_table(),
                TagEnd::TableHead => {
                    context.finish_table_row();
                    context.mark_table_head(false);
                }
                TagEnd::TableRow => context.finish_table_row(),
                TagEnd::TableCell => context.finish_table_cell(),
                _ => {}
            },
            Event::Text(text) => {
                if context.in_image {
                    if let Some(alt) = context.image_alt.as_mut() {
                        alt.push_str(&text);
                    }
                } else {
                    context.push_str(&escape_text(&text));
                }
            }
            Event::Code(code) => context.push_str(&format!("#raw(\"{}\")", escape_text(&code))),
            Event::SoftBreak => context.push_char('\n'),
            Event::HardBreak => context.push_str("\\\n"),
            Event::Rule => {
                context.push_str("\n#line(length: 100%, stroke: 1pt + rgb(\"#d7dee7\"))\n\n")
            }
            Event::FootnoteReference(name) => {
                context.push_str(&format!("[^{}]", escape_text(&name)))
            }
            Event::TaskListMarker(checked) => {
                context.push_str(&format!("#md_task({checked}) "));
            }
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::InlineMath(expr) => context.push_str(&format!(
                "${}$",
                escape_text(&normalize_math_expr(&expr))
            )),
            Event::DisplayMath(expr) => {
                context.push_str(&format!(
                    "\n${}$\n\n",
                    escape_text(&normalize_math_expr(&expr))
                ))
            }
        }
    }

    context.finish_table();
    context.out
}

#[derive(Default)]
struct MarkdownContext {
    out: String,
    list_stack: Vec<char>,
    image_href: Option<String>,
    in_image: bool,
    image_alt: Option<String>,
    quote_depth: usize,
    code_block: Option<CodeBlockState>,
    table: Option<TableState>,
}

impl MarkdownContext {
    fn push_str(&mut self, value: &str) {
        if let Some(code_block) = self.code_block.as_mut() {
            code_block.content.push_str(value);
            return;
        }
        if let Some(table) = self.table.as_mut() {
            table.push_str(value);
        } else {
            self.out.push_str(value);
        }
    }

    fn push_char(&mut self, value: char) {
        if let Some(code_block) = self.code_block.as_mut() {
            code_block.content.push(value);
            return;
        }
        if let Some(table) = self.table.as_mut() {
            table.push_char(value);
        } else {
            self.out.push(value);
        }
    }

    fn ensure_blank_line(&mut self) {
        if !self.out.ends_with("\n\n") {
            if !self.out.ends_with('\n') && !self.out.is_empty() {
                self.out.push('\n');
            }
            self.out.push('\n');
        }
    }

    fn start_table(&mut self, alignments: Vec<Alignment>) {
        self.finish_table();
        self.table = Some(TableState {
            alignments,
            ..TableState::default()
        });
    }

    fn start_code_block(&mut self, lang: &str) {
        self.code_block = Some(CodeBlockState {
            lang: lang.to_owned(),
            content: String::new(),
        });
    }

    fn finish_code_block(&mut self) {
        let Some(code_block) = self.code_block.take() else {
            return;
        };
        self.out.push_str(&format!(
            "#md_codeblock(\"{}\", \"{}\")\n\n",
            escape_text(&code_block.lang),
            escape_text(code_block.content.trim_end_matches('\n'))
        ));
    }

    fn mark_table_head(&mut self, in_head: bool) {
        if let Some(table) = self.table.as_mut() {
            table.in_head = in_head;
        }
    }

    fn start_table_row(&mut self) {
        if let Some(table) = self.table.as_mut() {
            table.current_row.clear();
        }
    }

    fn finish_table_row(&mut self) {
        if let Some(table) = self.table.as_mut() {
            table.finish_row();
        }
    }

    fn start_table_cell(&mut self) {
        if let Some(table) = self.table.as_mut() {
            table.current_cell.clear();
        }
    }

    fn finish_table_cell(&mut self) {
        if let Some(table) = self.table.as_mut() {
            table.finish_cell();
        }
    }

    fn finish_table(&mut self) {
        let Some(table) = self.table.take() else {
            return;
        };
        if table.rows.is_empty() {
            return;
        }

        self.ensure_blank_line();
        let column_count = table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0)
            .max(1);

        self.out.push_str(&format!(
            "#table(\n  columns: {column_count},\n  inset: (x: 10pt, y: 8pt),\n  align: (x, y) => if y == 0 {{ center }} else {{ left }},\n  stroke: (x, y) => if y == 0 {{ 0.9pt + rgb(\"#d9dee7\") }} else {{ 0.7pt + rgb(\"#e3e8ee\") }},\n  fill: (x, y) => if y == 0 {{ rgb(\"#f3f4f6\") }} else {{ white }},\n"
        ));

        if let Some(header) = table.rows.iter().find(|row| row.is_header) {
            self.out.push_str("  table.header(\n");
            for index in 0..column_count {
                self.out.push_str("    ");
                self.out.push_str(&table_cell(
                    header.cells.get(index).map(String::as_str).unwrap_or(""),
                    table.alignments.get(index),
                    true,
                ));
                self.out.push_str(",\n");
            }
            self.out.push_str("  ),\n");
        }

        for row in table.rows.iter().filter(|row| !row.is_header) {
            for index in 0..column_count {
                self.out.push_str("  ");
                self.out.push_str(&table_cell(
                    row.cells.get(index).map(String::as_str).unwrap_or(""),
                    table.alignments.get(index),
                    false,
                ));
                self.out.push_str(",\n");
            }
        }

        self.out.push_str(")\n\n");
    }
}

#[derive(Default)]
struct TableState {
    alignments: Vec<Alignment>,
    rows: Vec<TableRow>,
    current_row: Vec<String>,
    current_cell: String,
    in_head: bool,
}

impl TableState {
    fn push_str(&mut self, value: &str) {
        self.current_cell.push_str(value);
    }

    fn push_char(&mut self, value: char) {
        self.current_cell.push(value);
    }

    fn finish_cell(&mut self) {
        self.current_row.push(self.current_cell.trim().to_owned());
        self.current_cell.clear();
    }

    fn finish_row(&mut self) {
        if !self.current_cell.is_empty() {
            self.finish_cell();
        }
        if self.current_row.is_empty() {
            return;
        }
        self.rows.push(TableRow {
            cells: std::mem::take(&mut self.current_row),
            is_header: self.in_head,
        });
    }
}

struct TableRow {
    cells: Vec<String>,
    is_header: bool,
}

struct CodeBlockState {
    lang: String,
    content: String,
}

fn table_cell(content: &str, alignment: Option<&Alignment>, is_header: bool) -> String {
    let body = if is_header {
        format!("#strong[{}]", cell_body(content))
    } else {
        cell_body(content)
    };

    match alignment.copied().unwrap_or(Alignment::None) {
        Alignment::Left | Alignment::None => format!("[{}]", body),
        Alignment::Center => format!("[#align(center)[{}]]", body),
        Alignment::Right => format!("[#align(right)[{}]]", body),
    }
}

fn cell_body(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        " ".to_owned()
    } else {
        trimmed.to_owned()
    }
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
        .replace('#', "\\#")
        .replace('$', "\\$")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn normalize_math_expr(input: &str) -> String {
    let mut normalized = String::new();
    let mut chars = input.trim().chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let mut command = String::new();
            while let Some(next) = chars.peek() {
                if next.is_ascii_alphabetic() {
                    command.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }

            if command.is_empty() {
                if let Some(next) = chars.next() {
                    normalized.push(next);
                }
                continue;
            }

            normalized.push_str(match command.as_str() {
                "int" => "integral",
                "sqrt" => "sqrt",
                "infty" => "oo",
                "pi" => "pi",
                "alpha" => "alpha",
                "beta" => "beta",
                "gamma" => "gamma",
                "theta" => "theta",
                "lambda" => "lambda",
                "mu" => "mu",
                "sigma" => "sigma",
                "phi" => "phi",
                "omega" => "omega",
                _ => command.as_str(),
            });
            continue;
        }

        normalized.push(match ch {
            '{' => '(',
            '}' => ')',
            _ => ch,
        });
    }

    split_identifier_runs(&normalized)
}

fn split_identifier_runs(input: &str) -> String {
    let keywords = [
        "sqrt", "sin", "cos", "tan", "log", "ln", "exp", "lim", "sum", "prod", "integral",
        "oo", "pi", "alpha", "beta", "gamma", "theta", "lambda", "mu", "sigma", "phi",
        "omega",
    ];
    let mut output = String::new();
    let mut run = String::new();

    let flush_run = |run: &mut String, output: &mut String| {
        if run.is_empty() {
            return;
        }
        if run.len() > 1 && !keywords.contains(&run.as_str()) {
            output.push_str(&run.chars().map(|ch| ch.to_string()).collect::<Vec<_>>().join(" "));
        } else {
            output.push_str(run);
        }
        run.clear();
    };

    for ch in input.chars() {
        if ch.is_ascii_alphabetic() {
            run.push(ch);
        } else {
            flush_run(&mut run, &mut output);
            output.push(ch);
        }
    }
    flush_run(&mut run, &mut output);
    output
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

    #[test]
    fn converts_complex_markdown_structures() {
        let markdown = r#"# Title

Paragraph with #hash and $cash.

1. ordered
   - nested
   - [x] done

> quote

| Name | Value |
| --- | --- |
| A | `code` |

Inline footnote[^one] and image ![alt](image.png).

[^one]: Footnote text
"#;
        let output = markdown_to_typst(markdown, &BTreeMap::new(), &RenderOptions::default());

        assert!(output.contains("\\#hash"));
        assert!(output.contains("\\$cash"));
        assert!(output.contains("+ ordered"));
        assert!(output.contains("  - nested"));
        assert!(output.contains("#md_task(true) done"));
        assert!(output.contains("#md_quote(1)["));
        assert!(output.contains("#table("));
        assert!(output.contains("#strong["));
        assert!(output.contains("Name"));
        assert!(output.contains("Value"));
        assert!(output.contains("[^one]"));
        assert!(output.contains("#md_figure(\"image.png\""));
    }

    #[test]
    fn converts_all_markdown_fixture() {
        let markdown = include_str!("../../data/examples/all-markdown-syntax.md");
        let output = markdown_to_typst(markdown, &BTreeMap::new(), &RenderOptions::default());

        assert!(output.contains("= All Markdown Syntax"));
        assert!(output.contains("#strong[bold]"));
        assert!(output.contains("#emph[italic]"));
        assert!(output.contains("#strike[strike]"));
        assert!(output.contains("#raw(\"inline code\")"));
        assert!(output.contains("#sub[subscript]"));
        assert!(output.contains("#super[superscript]"));
        assert!(output.contains("#link(\"https://openai.com/\")"));
        assert!(output.contains("#md_quote(1)["));
        assert!(output.contains("#md_codeblock(\"rust\", \"fn main() {"));
        assert!(output.contains("#table("));
        assert!(!output.contains("<span data-demo"));
        assert!(output.contains("inline html fragment"));
        assert!(output.contains("#md_figure(\"diagram.svg\""));
        assert!(output.contains("$a^2 + b^2 = c^2$"));
        assert!(output.contains("[^note]"));
    }

    #[test]
    fn normalizes_latex_style_math_expressions() {
        let markdown = "Inline: $E = mc^2$\n\n$$\n\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}\n$$\n";
        let output = markdown_to_typst(markdown, &BTreeMap::new(), &RenderOptions::default());

        assert!(output.contains("$E = m c^2$"));
        assert!(output.contains("integral_(-oo)^(oo) e^(-x^2) d x = sqrt(pi)"));
    }
}
