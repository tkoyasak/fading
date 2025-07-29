use crop::Rope;
use tower_lsp_server::lsp_types::*;

fn position_to_index(rope: &Rope, position: &Position) -> usize {
    let line = position.line as usize;
    let char = position.character as usize;
    rope.byte_of_line(line) + rope.line(line).byte_of_utf16_code_unit(char)
}

pub fn lsp_range_to_rope_range(rope: &Rope, range: &Range) -> std::ops::Range<usize> {
    let start = position_to_index(rope, &range.start);
    let end = position_to_index(rope, &range.end);
    start..end
}
