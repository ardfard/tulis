//! The last frame's layout: wrapped lines per paragraph, used for hit-testing and vertical moves.

use gpui::{Pixels, Point, WrappedLine, point, px};
use tulis_core::DocPos;

pub(crate) fn clamp_px(value: Pixels, low: Pixels, high: Pixels) -> Pixels {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

pub struct ParaLayout {
    /// Offset of the paragraph's top from the element's top.
    pub top: Pixels,
    pub line: WrappedLine,
    /// Byte offset at which each visual line starts.
    pub line_starts: Vec<usize>,
    /// Byte length of the paragraph text this was shaped from (to detect a stale layout).
    pub text_len: usize,
}

impl ParaLayout {
    pub fn new(top: Pixels, line: WrappedLine, text_len: usize) -> Self {
        let mut line_starts = vec![0];
        for boundary in line.wrap_boundaries() {
            let run = &line.unwrapped_layout.runs[boundary.run_ix];
            line_starts.push(run.glyphs[boundary.glyph_ix].index);
        }
        Self {
            top,
            line,
            line_starts,
            text_len,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn height(&self, line_height: Pixels) -> Pixels {
        line_height * self.line_count() as f32
    }

    pub fn visual_line_of(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    pub fn line_range(&self, k: usize) -> (usize, usize) {
        let end = self
            .line_starts
            .get(k + 1)
            .copied()
            .unwrap_or(self.line.len());
        (self.line_starts[k], end)
    }

    /// x of `offset` measured from the start of visual line `k`.
    pub fn x_in_line(&self, k: usize, offset: usize) -> Pixels {
        let layout = &self.line.unwrapped_layout;
        layout.x_for_index(offset) - layout.x_for_index(self.line_starts[k])
    }

    /// Position of `offset` relative to the paragraph's top-left corner.
    pub fn point_for(&self, offset: usize, line_height: Pixels) -> Point<Pixels> {
        let k = self.visual_line_of(offset);
        point(self.x_in_line(k, offset), line_height * k as f32)
    }

    pub fn index_for(&self, local: Point<Pixels>, line_height: Pixels) -> usize {
        match self.line.closest_index_for_position(local, line_height) {
            Ok(ix) | Err(ix) => ix,
        }
    }
}

pub struct LayoutCache {
    /// Window position of the element's top-left corner.
    pub origin: Point<Pixels>,
    pub width: Pixels,
    pub line_height: Pixels,
    pub paras: Vec<ParaLayout>,
}

impl LayoutCache {
    pub fn para_gap(&self) -> Pixels {
        self.line_height * 0.6
    }

    pub fn total_height(&self) -> Pixels {
        self.paras
            .last()
            .map_or(self.line_height, |p| p.top + p.height(self.line_height))
    }

    /// Window position of the top of the caret at `pos`.
    pub fn window_point(&self, pos: DocPos) -> Option<Point<Pixels>> {
        let para = self.paras.get(pos.para)?;
        let local = para.point_for(pos.offset.min(para.line.len()), self.line_height);
        Some(point(
            self.origin.x + local.x,
            self.origin.y + para.top + local.y,
        ))
    }

    pub fn pos_for_window_point(&self, window_point: Point<Pixels>) -> DocPos {
        let x = window_point.x - self.origin.x;
        let y = window_point.y - self.origin.y;
        let gap = self.para_gap();
        let ix = self
            .paras
            .iter()
            .position(|p| y < p.top + p.height(self.line_height) + gap / 2.)
            .unwrap_or(self.paras.len().saturating_sub(1));
        let Some(para) = self.paras.get(ix) else {
            return DocPos::default();
        };
        let local_y = clamp_px(y - para.top, px(0.), para.height(self.line_height) - px(1.));
        let local_x = clamp_px(x, px(0.), self.width);
        DocPos::new(
            ix,
            para.index_for(point(local_x, local_y), self.line_height),
        )
    }

    /// Move `lines` visual lines (negative = up) from `pos`, aiming for x = `goal_x`.
    pub fn vertical(&self, pos: DocPos, lines: isize, goal_x: Pixels) -> DocPos {
        if self.paras.is_empty() {
            return pos;
        }
        let mut para = pos.para.min(self.paras.len() - 1);
        let mut k = self.paras[para].visual_line_of(pos.offset) as isize + lines;
        while k < 0 {
            if para == 0 {
                return DocPos::new(0, 0);
            }
            para -= 1;
            k += self.paras[para].line_count() as isize;
        }
        while k >= self.paras[para].line_count() as isize {
            if para + 1 >= self.paras.len() {
                return DocPos::new(para, self.paras[para].line.len());
            }
            k -= self.paras[para].line_count() as isize;
            para += 1;
        }
        let y = self.line_height * k as f32 + self.line_height / 2.;
        DocPos::new(
            para,
            self.paras[para].index_for(point(goal_x, y), self.line_height),
        )
    }

    /// Start and end of the visual line containing `pos`; a wrapped line's end excludes trailing spaces.
    pub fn line_bounds(&self, pos: DocPos, text: &str) -> (usize, usize) {
        let Some(para) = self.paras.get(pos.para) else {
            return (0, text.len());
        };
        let k = para.visual_line_of(pos.offset);
        let (start, end) = para.line_range(k);
        // A stale layout may point past the text or into a character.
        let start = text.floor_char_boundary(start);
        let mut end = text.floor_char_boundary(end).max(start);
        if k + 1 < para.line_count() {
            while end > start && text[..end].ends_with(char::is_whitespace) {
                end -= text[..end].chars().next_back().map_or(1, char::len_utf8);
            }
        }
        (start, end)
    }

    pub fn x_for(&self, pos: DocPos) -> Pixels {
        let Some(para) = self.paras.get(pos.para) else {
            return px(0.);
        };
        para.x_in_line(
            para.visual_line_of(pos.offset),
            pos.offset.min(para.line.len()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(line_starts: Vec<usize>, text_len: usize) -> LayoutCache {
        LayoutCache {
            origin: point(px(0.), px(0.)),
            width: px(100.),
            line_height: px(20.),
            paras: vec![ParaLayout {
                top: px(0.),
                line: WrappedLine::default(),
                line_starts,
                text_len,
            }],
        }
    }

    #[test]
    fn line_bounds_stay_on_char_boundaries_with_stale_offsets() {
        let text = "héllo wörld"; // é is bytes 1..3
        let layout = cache(vec![0, 2, 50], 40);
        assert_eq!(layout.line_bounds(DocPos::new(0, 1), text), (0, 1));
        assert_eq!(layout.line_bounds(DocPos::new(0, 5), text), (1, text.len()));
        assert_eq!(
            layout.line_bounds(DocPos::new(0, 60), text),
            (text.len(), text.len())
        );
    }
}
