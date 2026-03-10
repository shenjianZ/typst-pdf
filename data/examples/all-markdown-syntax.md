# All Markdown Syntax

Paragraph with **bold**, *italic*, ***both***, ~~strike~~, `inline code`, ~subscript~, ^superscript^, and escaped chars `#`, `$`, `[`, `]`.

Line with a hard break at the end.  
Next line after the hard break.

---

## Links

Inline link to [OpenAI](https://openai.com/) and automatic URL <https://example.com>.

## Lists

- unordered item
- nested container
  - child item
  - second child with `code`

1. ordered item
2. second ordered item
   - mixed nested bullet
   - [x] completed task
   - [ ] pending task

## Quote

> Blockquote level 1
>
> > Nested quote level 2
>
> Back to level 1.

## Code

```rust
fn main() {
    println!("fenced code block");
}
```

    indented code block
    still part of the same block

## Table

| Name | Value | Notes |
| --- | ---: | :--- |
| alpha | 1 | left aligned |
| beta | `code` | another cell |

## HTML

<span data-demo="inline-html">inline html fragment</span>

<div>
block html fragment
</div>

## Media

Image example: ![diagram](diagram.svg)

## Math

Inline math: $a^2 + b^2 = c^2$.

Display math:

$$
sum_(i=1)^n i = n (n + 1) / 2
$$

## Footnotes

Footnote reference[^note] in a paragraph.

[^note]: Footnote definition text.

### Final Section

Plain text after all constructs.
