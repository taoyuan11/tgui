use ropey::Rope;

#[derive(Clone, Debug)]
pub(crate) struct RopeBuffer {
    rope: Rope,
    snapshot: String,
}

impl RopeBuffer {
    pub(crate) fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            snapshot: text.to_string(),
        }
    }

    pub(crate) fn from_parts(rope: Rope, snapshot: String) -> Self {
        Self { rope, snapshot }
    }

    pub(crate) fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub(crate) fn clamp_byte_boundary(&self, byte_index: usize) -> usize {
        let byte_index = byte_index.min(self.len_bytes());
        self.char_to_byte(self.byte_to_char(byte_index))
    }

    pub(crate) fn byte_to_char(&self, byte_index: usize) -> usize {
        self.rope.byte_to_char(byte_index.min(self.len_bytes()))
    }

    pub(crate) fn char_to_byte(&self, char_index: usize) -> usize {
        self.rope
            .char_to_byte(char_index.min(self.rope.len_chars()))
    }

    pub(crate) fn prev_char_boundary_byte(&self, byte_index: usize) -> usize {
        let byte_index = self.clamp_byte_boundary(byte_index);
        if byte_index == 0 {
            return 0;
        }

        let char_index = self.byte_to_char(byte_index);
        if char_index == 0 {
            0
        } else {
            self.char_to_byte(char_index - 1)
        }
    }

    pub(crate) fn next_char_boundary_byte(&self, byte_index: usize) -> usize {
        let byte_index = self.clamp_byte_boundary(byte_index);
        if byte_index >= self.len_bytes() {
            return self.len_bytes();
        }

        let char_index = self.byte_to_char(byte_index);
        self.char_to_byte((char_index + 1).min(self.rope.len_chars()))
    }

    pub(crate) fn replace_byte_range(
        &mut self,
        start_byte: usize,
        end_byte: usize,
        replacement: &str,
    ) {
        let start_byte = self.clamp_byte_boundary(start_byte);
        let end_byte = self.clamp_byte_boundary(end_byte);
        let start_char = self.byte_to_char(start_byte);
        let end_char = self.byte_to_char(end_byte.max(start_byte));

        self.rope.remove(start_char..end_char);
        if !replacement.is_empty() {
            self.rope.insert(start_char, replacement);
        }
        self.snapshot
            .replace_range(start_byte..end_byte, replacement);
    }

    pub(crate) fn slice_byte_range_to_string(&self, start_byte: usize, end_byte: usize) -> String {
        let start_byte = self.clamp_byte_boundary(start_byte);
        let end_byte = self.clamp_byte_boundary(end_byte.max(start_byte));
        let start_char = self.byte_to_char(start_byte);
        let end_char = self.byte_to_char(end_byte);
        self.rope.slice(start_char..end_char).to_string()
    }

    #[cfg(test)]
    pub(crate) fn materialize_string(&self) -> String {
        self.snapshot.clone()
    }

    pub(crate) fn into_parts(self) -> (Rope, String) {
        (self.rope, self.snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::RopeBuffer;

    #[test]
    fn clamp_byte_boundary_moves_to_previous_utf8_boundary() {
        let buffer = RopeBuffer::from_str("a中🙂");

        assert_eq!(buffer.clamp_byte_boundary(0), 0);
        assert_eq!(buffer.clamp_byte_boundary(1), 1);
        assert_eq!(buffer.clamp_byte_boundary(2), 1);
        assert_eq!(buffer.clamp_byte_boundary(3), 1);
        assert_eq!(buffer.clamp_byte_boundary(4), 4);
        assert_eq!(buffer.clamp_byte_boundary(7), 4);
        assert_eq!(buffer.clamp_byte_boundary(8), 8);
    }

    #[test]
    fn char_boundaries_round_trip_through_byte_offsets() {
        let buffer = RopeBuffer::from_str("a中🙂b");

        assert_eq!(buffer.byte_to_char(0), 0);
        assert_eq!(buffer.byte_to_char(1), 1);
        assert_eq!(buffer.byte_to_char(4), 2);
        assert_eq!(buffer.byte_to_char(8), 3);
        assert_eq!(buffer.char_to_byte(0), 0);
        assert_eq!(buffer.char_to_byte(1), 1);
        assert_eq!(buffer.char_to_byte(2), 4);
        assert_eq!(buffer.char_to_byte(3), 8);
        assert_eq!(buffer.char_to_byte(4), 9);
    }

    #[test]
    fn replace_byte_range_updates_buffer_and_snapshot() {
        let mut buffer = RopeBuffer::from_str("hello🙂world");
        let start = "hello".len();
        let end = start + "🙂".len();
        buffer.replace_byte_range(start, end, "中");

        assert_eq!(buffer.materialize_string(), "hello中world");
        assert_eq!(
            buffer.prev_char_boundary_byte("hello中".len()),
            "hello".len()
        );
        assert_eq!(
            buffer.next_char_boundary_byte("hello".len()),
            "hello中".len()
        );
    }

    #[test]
    fn slice_byte_range_returns_expected_text() {
        let buffer = RopeBuffer::from_str("ab中cd🙂ef");
        let start = "ab".len();
        let end = "ab中cd🙂".len();

        assert_eq!(buffer.slice_byte_range_to_string(start, end), "中cd🙂");
    }
}
