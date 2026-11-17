//! TrueType fonts, as far as sketch text needs them: each character's glyph outline (its contours' points, on the
//! curve or quadratic control points), its box and its advance, from the `cmap`, `hmtx`, `loca` and `glyf` tables.

use std::collections::HashMap;

pub struct Font {
    data: &'static [u8],
    tables: HashMap<[u8; 4], (usize, usize)>,
    pub units_per_em: f64,
    long_loca: bool,
    glyphs: usize,
    metrics: usize,
}

/// A glyph's outline in font units: its contours, each a closed run of points (on the curve or not), and its box.
#[derive(Clone, Debug, Default)]
pub struct Glyph {
    pub contours: Vec<Vec<(f64, f64, bool)>>,
    pub x_max: f64,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn i16_at(d: &[u8], at: usize) -> Option<i16> {
    u16_at(d, at).map(|v| v as i16)
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(at)?, *d.get(at + 1)?, *d.get(at + 2)?, *d.get(at + 3)?]))
}

impl Font {
    pub fn parse(data: &'static [u8]) -> Option<Font> {
        let count = u16_at(data, 4)? as usize;
        let mut tables = HashMap::new();
        for i in 0..count {
            let record = 12 + 16 * i;
            let tag: [u8; 4] = data.get(record..record + 4)?.try_into().ok()?;
            tables.insert(tag, (u32_at(data, record + 8)? as usize, u32_at(data, record + 12)? as usize));
        }
        let head = tables.get(b"head")?.0;
        let maxp = tables.get(b"maxp")?.0;
        let hhea = tables.get(b"hhea")?.0;
        Some(Font {
            data,
            units_per_em: u16_at(data, head + 18)? as f64,
            long_loca: i16_at(data, head + 50)? == 1,
            glyphs: u16_at(data, maxp + 4)? as usize,
            metrics: u16_at(data, hhea + 34)? as usize,
            tables,
        })
    }

    fn table(&self, tag: &[u8; 4]) -> Option<usize> {
        self.tables.get(tag).map(|t| t.0)
    }

    /// The glyph for a character (format 4 or 12 of the Windows Unicode `cmap`), or the missing glyph's.
    pub fn glyph_index(&self, c: char) -> usize {
        self.lookup(c as u32).unwrap_or(0)
    }

    fn lookup(&self, code: u32) -> Option<usize> {
        let d = self.data;
        let cmap = self.table(b"cmap")?;
        let count = u16_at(d, cmap + 2)? as usize;
        let mut best: Option<usize> = None;
        for i in 0..count {
            let (platform, encoding) = (u16_at(d, cmap + 4 + 8 * i)?, u16_at(d, cmap + 6 + 8 * i)?);
            let offset = cmap + u32_at(d, cmap + 8 + 8 * i)? as usize;
            let format = u16_at(d, offset)?;
            if platform == 3 && encoding == 10 && format == 12 {
                best = Some(offset);
                break;
            }
            if platform == 3 && encoding == 1 && format == 4 && best.is_none() {
                best = Some(offset);
            }
        }
        let table = best?;
        match u16_at(d, table)? {
            4 => {
                let segments = u16_at(d, table + 6)? as usize / 2;
                let ends = table + 14;
                let starts = ends + 2 * segments + 2;
                let deltas = starts + 2 * segments;
                let offsets = deltas + 2 * segments;
                for s in 0..segments {
                    let end = u16_at(d, ends + 2 * s)? as u32;
                    let start = u16_at(d, starts + 2 * s)? as u32;
                    if code < start || code > end {
                        continue;
                    }
                    let delta = u16_at(d, deltas + 2 * s)?;
                    let range = u16_at(d, offsets + 2 * s)? as usize;
                    let glyph = if range == 0 {
                        (code as u16).wrapping_add(delta)
                    } else {
                        let at = offsets + 2 * s + range + 2 * (code - start) as usize;
                        match u16_at(d, at)? {
                            0 => 0,
                            g => g.wrapping_add(delta),
                        }
                    };
                    return Some(glyph as usize);
                }
                None
            }
            12 => {
                let groups = u32_at(d, table + 12)? as usize;
                for g in 0..groups {
                    let at = table + 16 + 12 * g;
                    let (start, end, first) = (u32_at(d, at)?, u32_at(d, at + 4)?, u32_at(d, at + 8)?);
                    if (start..=end).contains(&code) {
                        return Some((first + code - start) as usize);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// A glyph's advance width, in font units.
    pub fn advance(&self, glyph: usize) -> f64 {
        let Some(hmtx) = self.table(b"hmtx") else { return 0.0 };
        let i = glyph.min(self.metrics.saturating_sub(1));
        u16_at(self.data, hmtx + 4 * i).unwrap_or(0) as f64
    }

    /// A glyph's outline: composite glyphs resolved into their parts' points, moved by their offsets.
    pub fn glyph(&self, glyph: usize) -> Glyph {
        self.outline(glyph, 0).unwrap_or_default()
    }

    fn outline(&self, glyph: usize, depth: usize) -> Option<Glyph> {
        let d = self.data;
        if glyph >= self.glyphs || depth > 8 {
            return None;
        }
        let loca = self.table(b"loca")?;
        let glyf = self.table(b"glyf")?;
        let (start, end) = if self.long_loca {
            (u32_at(d, loca + 4 * glyph)? as usize, u32_at(d, loca + 4 * glyph + 4)? as usize)
        } else {
            (2 * u16_at(d, loca + 2 * glyph)? as usize, 2 * u16_at(d, loca + 2 * glyph + 2)? as usize)
        };
        if end <= start {
            return Some(Glyph::default());
        }
        let g = glyf + start;
        let contours = i16_at(d, g)?;
        let x_max = i16_at(d, g + 6)? as f64;
        if contours >= 0 {
            let contours = contours as usize;
            let ends: Vec<usize> =
                (0..contours).map(|i| u16_at(d, g + 10 + 2 * i).map(|e| e as usize)).collect::<Option<_>>()?;
            let points = ends.last().map_or(0, |e| e + 1);
            let instructions = u16_at(d, g + 10 + 2 * contours)? as usize;
            let mut at = g + 12 + 2 * contours + instructions;
            let mut flags = Vec::with_capacity(points);
            while flags.len() < points {
                let flag = *d.get(at)?;
                at += 1;
                flags.push(flag);
                if flag & 8 != 0 {
                    let repeat = *d.get(at)?;
                    at += 1;
                    for _ in 0..repeat {
                        flags.push(flag);
                    }
                }
            }
            flags.truncate(points);
            let mut coordinates = |short: u8, same: u8| -> Option<Vec<f64>> {
                let mut values = Vec::with_capacity(points);
                let mut value = 0i32;
                for &flag in &flags {
                    if flag & short != 0 {
                        let delta = *d.get(at)? as i32;
                        at += 1;
                        value += if flag & same != 0 { delta } else { -delta };
                    } else if flag & same == 0 {
                        value += i16_at(d, at)? as i32;
                        at += 2;
                    }
                    values.push(value as f64);
                }
                Some(values)
            };
            let xs = coordinates(2, 16)?;
            let ys = coordinates(4, 32)?;
            let mut out = Vec::new();
            let mut first = 0;
            for &end in &ends {
                out.push((first..=end).map(|i| (xs[i], ys[i], flags[i] & 1 != 0)).collect());
                first = end + 1;
            }
            return Some(Glyph { contours: out, x_max });
        }
        // A composite: its parts' outlines, each moved by its offset (scaled parts aren't needed by the fonts here).
        let mut out = Glyph { contours: Vec::new(), x_max };
        let mut at = g + 10;
        loop {
            let flags = u16_at(d, at)?;
            let part = u16_at(d, at + 2)? as usize;
            at += 4;
            let (dx, dy) = if flags & 1 != 0 {
                let v = (i16_at(d, at)? as f64, i16_at(d, at + 2)? as f64);
                at += 4;
                v
            } else {
                let v = (*d.get(at)? as i8 as f64, *d.get(at + 1)? as i8 as f64);
                at += 2;
                v
            };
            let (mut a, mut b, mut c, mut e) = (1.0, 0.0, 0.0, 1.0);
            let f2dot14 = |at: usize| i16_at(d, at).map(|v| v as f64 / 16384.0);
            if flags & 8 != 0 {
                a = f2dot14(at)?;
                e = a;
                at += 2;
            } else if flags & 0x40 != 0 {
                a = f2dot14(at)?;
                e = f2dot14(at + 2)?;
                at += 4;
            } else if flags & 0x80 != 0 {
                (a, b, c, e) = (f2dot14(at)?, f2dot14(at + 2)?, f2dot14(at + 4)?, f2dot14(at + 6)?);
                at += 8;
            }
            if let Some(inner) = self.outline(part, depth + 1) {
                for contour in inner.contours {
                    out.contours.push(
                        contour.into_iter().map(|(x, y, on)| (a * x + c * y + dx, b * x + e * y + dy, on)).collect(),
                    );
                }
            }
            if flags & 0x20 == 0 {
                break;
            }
        }
        Some(out)
    }
}

/// A contour's pieces: lines between points on the curve, quadratic Béziers through each control point (with the
/// point on the curve TrueType implies midway between two control points), each as its points.
pub fn segments(contour: &[(f64, f64, bool)]) -> Vec<Vec<(f64, f64)>> {
    let Some(first_on) = contour.iter().position(|p| p.2) else {
        // All control points: start midway between the last and the first.
        let n = contour.len();
        if n == 0 {
            return Vec::new();
        }
        let mid = |a: (f64, f64, bool), b: (f64, f64, bool)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, true);
        let mut points = vec![mid(contour[n - 1], contour[0])];
        points.extend_from_slice(contour);
        return segments(&points);
    };
    let mut points: Vec<(f64, f64, bool)> = contour[first_on..].iter().chain(&contour[..first_on]).copied().collect();
    points.push(points[0]);
    let mut out = Vec::new();
    let mut current = (points[0].0, points[0].1);
    let mut control: Option<(f64, f64)> = None;
    for &(x, y, on) in &points[1..] {
        if on {
            match control.take() {
                None => out.push(vec![current, (x, y)]),
                Some(c) => out.push(vec![current, c, (x, y)]),
            }
            current = (x, y);
        } else {
            if let Some(c) = control {
                let mid = ((c.0 + x) / 2.0, (c.1 + y) / 2.0);
                out.push(vec![current, c, mid]);
                current = mid;
            }
            control = Some((x, y));
        }
    }
    out
}

/// The fonts sketch text can use, by the names Onshape gives them (fs-geom/fonts/README.md).
pub fn by_name(name: &str) -> Option<&'static Font> {
    use std::sync::OnceLock;
    macro_rules! fonts {
        ($($file:literal),* $(,)?) => {{
            const FILES: &[(&str, &[u8])] = &[$(($file, include_bytes!(concat!("../fonts/", $file)))),*];
            static PARSED: [OnceLock<Option<Font>>; FILES.len()] = [const { OnceLock::new() }; FILES.len()];
            let k = FILES.iter().position(|(file, _)| *file == name)?;
            PARSED[k].get_or_init(|| Font::parse(FILES[k].1)).as_ref()
        }};
    }
    fonts!(
        "OpenSans-Regular.ttf",
        "OpenSans-Bold.ttf",
        "OpenSans-Italic.ttf",
        "OpenSans-BoldItalic.ttf",
        "Arimo-Regular.ttf",
        "Arimo-Italic.ttf",
        "Arimo-BoldItalic.ttf",
        "Tinos-Regular.ttf",
        "Tinos-Bold.ttf",
        "Tinos-Italic.ttf",
        "Tinos-BoldItalic.ttf",
    )
}

/// Whether Onshape's sketch text has a font of this name (std's `skText` lists them, with their bold and italic
/// forms), whether or not the engine does.
pub fn onshape_has(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".ttf").or_else(|| name.strip_suffix(".otf")) else { return false };
    let family = stem.rsplit_once('-').map_or(stem, |(f, _)| f);
    matches!(
        family,
        "OpenSans" | "AllertaStencil" | "Arimo" | "DroidSansMono" | "NotoSans" | "NotoSansCJKjp" | "NotoSansCJKkr"
            | "NotoSansCJKsc" | "NotoSansCJKtc" | "NotoSerif" | "RobotoSlab" | "Tinos"
    )
}
