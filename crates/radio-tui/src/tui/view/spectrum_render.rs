use crate::tui::model::SpectrumStyle;

pub const BAR_GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub glyph: char,
    pub level: f32,
}

const EMPTY: Cell = Cell {
    glyph: ' ',
    level: 0.0,
};

pub fn render_grid(
    levels: &[f32],
    width: usize,
    height: usize,
    style: SpectrumStyle,
) -> Vec<Vec<Cell>> {
    let mut grid = vec![vec![EMPTY; width]; height.max(1)];
    if width == 0 || height == 0 || levels.is_empty() {
        return grid;
    }
    for col in 0..width {
        let level = sample_level(levels, col, width).clamp(0.0, 1.0);
        match style {
            SpectrumStyle::Bars => fill_bar(&mut grid, col, level, 0, height),
            SpectrumStyle::Mirror => fill_mirror(&mut grid, col, level, height),
            SpectrumStyle::Dots => fill_dots(&mut grid, col, level, height),
            SpectrumStyle::Wave => fill_wave(&mut grid, col, level, height),
        }
    }
    grid
}

fn sample_level(levels: &[f32], col: usize, width: usize) -> f32 {
    if width <= 1 {
        return levels[0];
    }
    let idx = col * levels.len() / width;
    levels[idx.min(levels.len() - 1)]
}

fn fill_bar(grid: &mut [Vec<Cell>], col: usize, level: f32, top: usize, bottom: usize) {
    let span = (bottom - top) as f32;
    let units = level * span * 8.0;
    for (row, cell) in grid.iter_mut().enumerate().take(bottom).skip(top) {
        let from_bottom = (bottom - 1 - row) as f32;
        let cell_units = units - from_bottom * 8.0;
        if cell_units <= 0.0 {
            continue;
        }
        let g = ((cell_units.min(8.0).ceil() as usize).max(1) - 1).min(7);
        cell[col] = Cell {
            glyph: BAR_GLYPHS[g],
            level,
        };
    }
}

fn fill_mirror(grid: &mut [Vec<Cell>], col: usize, level: f32, height: usize) {
    let center = (height - 1) as f32 / 2.0;
    let reach = level * (center + 0.5);
    for (row, cell) in grid.iter_mut().enumerate() {
        let dist = (row as f32 - center).abs();
        if dist <= reach {
            cell[col] = Cell {
                glyph: '█', level
            };
        }
    }
    if level > 0.0 {
        let c = (center.round() as usize).min(height - 1);
        grid[c][col] = Cell {
            glyph: '━', level
        };
    }
}

fn fill_dots(grid: &mut [Vec<Cell>], col: usize, level: f32, height: usize) {
    if level <= 0.0 {
        return;
    }
    let from_bottom = (level * (height - 1) as f32).round() as usize;
    let row = height - 1 - from_bottom.min(height - 1);
    let glyph = match level {
        x if x > 0.66 => '●',
        x if x > 0.33 => '•',
        _ => '·',
    };
    grid[row][col] = Cell { glyph, level };
}

fn fill_wave(grid: &mut [Vec<Cell>], col: usize, level: f32, height: usize) {
    let pos = level * (height - 1) as f32;
    let row = height - 1 - (pos.round() as usize).min(height - 1);
    let frac = pos - pos.floor();
    let glyph = match frac {
        x if x < 0.34 => '▁',
        x if x < 0.67 => '▄',
        _ => '▀',
    };
    grid[row][col] = Cell { glyph, level };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occupied(grid: &[Vec<Cell>], col: usize) -> usize {
        grid.iter().filter(|row| row[col].glyph != ' ').count()
    }

    #[test]
    fn empty_levels_yield_blank_grid() {
        let g = render_grid(&[], 4, 3, SpectrumStyle::Bars);
        assert_eq!(g.len(), 3);
        assert!(g.iter().all(|r| r.iter().all(|c| c.glyph == ' ')));
    }

    #[test]
    fn bars_taller_level_fills_more_rows() {
        let g = render_grid(&[1.0, 0.1], 2, 4, SpectrumStyle::Bars);
        assert!(occupied(&g, 0) > occupied(&g, 1));
        assert_eq!(occupied(&g, 0), 4, "full level fills the column");
    }

    #[test]
    fn bars_fill_from_the_bottom() {
        let g = render_grid(&[0.3], 1, 4, SpectrumStyle::Bars);
        assert_ne!(g[3][0].glyph, ' ', "bottom row must be filled first");
        assert_eq!(g[0][0].glyph, ' ', "top stays empty for a low level");
    }

    #[test]
    fn mirror_is_symmetric_around_center() {
        let g = render_grid(&[1.0], 1, 5, SpectrumStyle::Mirror);
        let filled: Vec<usize> = (0..5).filter(|&r| g[r][0].glyph != ' ').collect();
        assert!(filled.contains(&0) && filled.contains(&4));
    }

    #[test]
    fn dots_place_single_mark_per_column() {
        let g = render_grid(&[1.0, 0.5], 2, 4, SpectrumStyle::Dots);
        assert_eq!(occupied(&g, 0), 1);
        assert_eq!(occupied(&g, 1), 1);
    }

    #[test]
    fn wave_places_single_mark_per_column() {
        let g = render_grid(&[0.8], 1, 4, SpectrumStyle::Wave);
        assert_eq!(occupied(&g, 0), 1);
    }

    #[test]
    fn grid_dimensions_match_request() {
        let g = render_grid(&[0.5, 0.5, 0.5], 6, 3, SpectrumStyle::Bars);
        assert_eq!(g.len(), 3);
        assert!(g.iter().all(|r| r.len() == 6));
    }
}
